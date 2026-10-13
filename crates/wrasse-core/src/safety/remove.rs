//! Carrying out a plan (to the tray, or deleted for good), restoring and purging the tray.

use super::*;

/// Acts on a prepared plan: re-checks each item, then quarantines or deletes it (or, in dry-run,
/// reports what it would do). One operation at a time; the machine stays awake meanwhile.
pub fn remove(ctx: &Context, plan: &Plan, dry_run: bool) -> io::Result<Report> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    let _awake = platform::keep_awake("Wrasse is cleaning up");
    let mut report = Report {
        dry_run,
        skipped: plan.skipped.clone(),
        ..Default::default()
    };
    let mut batch = if dry_run || plan.mode == Mode::DeleteNow {
        None
    } else {
        Some(ctx.quarantine.begin()?)
    };
    let action = match plan.mode {
        Mode::Quarantine => "quarantine",
        Mode::DeleteNow => "delete",
    };
    for item in &plan.items {
        let req = &item.request;
        let target = req.path.to_string_lossy();
        let refused = match vet(ctx, req, plan.mode) {
            Err(reason) => Some(reason),
            Ok(_) if req.scanned.is_some_and(|s| !matches_scan(&req.path, &s)) => {
                Some(Reason::ChangedSinceScan)
            }
            Ok(_) if !still_same(item) => Some(Reason::Changed),
            Ok(_) => None,
        };
        if let Some(reason) = refused {
            report.skipped.push(Skipped {
                path: req.path.clone(),
                reason,
            });
            log(
                ctx,
                "skip",
                &target,
                item.bytes,
                req.rule_id.as_deref(),
                dry_run,
                if reason == Reason::Changed {
                    "changed since preview"
                } else if reason == Reason::ChangedSinceScan {
                    "changed since the scan"
                } else {
                    "refused at the last moment"
                },
            );
            continue;
        }
        if dry_run {
            report.done.push(Done {
                path: req.path.clone(),
                bytes: item.bytes,
                entry: None,
            });
            report.bytes += item.bytes;
            log(
                ctx,
                action,
                &target,
                item.bytes,
                req.rule_id.as_deref(),
                true,
                "ok",
            );
            continue;
        }
        let result = match (&mut batch, plan.mode) {
            (Some(w), Mode::Quarantine) => w
                .store(&req.path, item.bytes, req.rule_id.as_deref())
                .map(Some),
            _ => delete(&req.path, item.identity.is_dir && !item.identity.is_symlink).map(|_| None),
        };
        match result {
            Ok(entry) => {
                report.done.push(Done {
                    path: req.path.clone(),
                    bytes: item.bytes,
                    entry,
                });
                report.bytes += item.bytes;
                log(
                    ctx,
                    action,
                    &target,
                    item.bytes,
                    req.rule_id.as_deref(),
                    false,
                    "ok",
                );
            }
            Err(e) => {
                report.skipped.push(Skipped {
                    path: req.path.clone(),
                    reason: Reason::Failed,
                });
                log(
                    ctx,
                    action,
                    &target,
                    item.bytes,
                    req.rule_id.as_deref(),
                    false,
                    &e.to_string(),
                );
            }
        }
    }
    if let Some(w) = batch {
        w.finish()?;
    }
    Ok(report)
}

pub(super) fn delete(path: &Path, is_dir: bool) -> io::Result<()> {
    if is_dir {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub(super) fn log(
    ctx: &Context,
    action: &str,
    target: &str,
    bytes: u64,
    rule_id: Option<&str>,
    dry_run: bool,
    result: &str,
) {
    // An audit line that can't be written must not stop the cleanup; it's reported in the result.
    let _ = ctx.audit.write(Record {
        at: 0,
        action,
        target,
        bytes,
        rule_id,
        dry_run,
        result,
    });
}

/// Restores quarantined items (by entry id).
pub fn restore(ctx: &Context, ids: &[String]) -> io::Result<Vec<(String, PathBuf)>> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    let restored = ctx.quarantine.restore(ids)?;
    for (_, path) in &restored {
        log(
            ctx,
            "restore",
            &path.to_string_lossy(),
            0,
            None,
            false,
            "ok",
        );
    }
    Ok(restored)
}

/// Empties quarantined items for good (the tray's "Empty now"). Not undoable.
pub fn purge(ctx: &Context, ids: &[String], dry_run: bool) -> io::Result<u64> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    if dry_run {
        let bytes = ctx
            .quarantine
            .stored()
            .iter()
            .filter(|(_, e)| ids.contains(&e.id))
            .map(|(_, e)| e.bytes)
            .sum();
        log(
            ctx,
            "purge",
            &format!("{} quarantined items", ids.len()),
            bytes,
            None,
            true,
            "ok",
        );
        return Ok(bytes);
    }
    let purged = ctx.quarantine.purge(ids)?;
    let bytes = purged.iter().map(|(_, b)| b).sum();
    log(
        ctx,
        "purge",
        &format!("{} quarantined items", purged.len()),
        bytes,
        None,
        false,
        "ok",
    );
    Ok(bytes)
}
