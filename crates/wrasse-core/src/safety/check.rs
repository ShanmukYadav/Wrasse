//! Planning a removal: every request is vetted (protected places, Never touch, scan match,
//! ownership, running apps) and re-checked at the last moment.

use super::*;

pub(super) fn identity(md: &fs::Metadata) -> Identity {
    let (dev, ino) = platform::file_identity(md);
    Identity {
        dev,
        ino,
        uid: platform::owner_uid(md),
        is_dir: md.is_dir(),
        is_symlink: md.file_type().is_symlink(),
        len: md.len(),
        mtime: md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs()),
    }
}

pub(super) fn resolve_dir(dir: &Path) -> io::Result<PathBuf> {
    let d = fs::canonicalize(dir)?;
    Ok(d.strip_prefix("/System/Volumes/Data")
        .map(|p| Path::new("/").join(p))
        .unwrap_or(d))
}

/// The path with its parent folders' links resolved (the item itself is not followed, so a link
/// is removed as a link). Firmlinked data-volume paths read as the user sees them.
pub(super) fn resolve(path: &Path) -> Option<PathBuf> {
    let parent = fs::canonicalize(path.parent()?).ok()?;
    let parent = parent
        .strip_prefix("/System/Volumes/Data")
        .map(|p| Path::new("/").join(p))
        .unwrap_or(parent);
    Some(parent.join(path.file_name()?))
}

/// Why a path can't be removed at all, or `None` when only the per-mode rules apply.
pub(super) fn protection_of(ctx: &Context, path: &Path) -> Option<Protection> {
    if path.components().any(|c| {
        platform::protected_name_suffixes()
            .iter()
            .any(|s| c.as_os_str().to_string_lossy().ends_with(s))
    }) {
        return Some(Protection::Hard);
    }
    let mut strongest: Option<Protection> = None;
    for p in &ctx.protected {
        let hit = match p.protection {
            Protection::Hard => path.starts_with(&p.path),
            // The folder itself; its contents are fine (FolderOnly) or hand-pick-only.
            Protection::FolderOnly => path == p.path,
            Protection::HandPickOnly => path.starts_with(&p.path),
        };
        if !hit {
            continue;
        }
        let this = if p.protection == Protection::HandPickOnly && path == p.path {
            Protection::Hard
        } else {
            p.protection
        };
        strongest = Some(match (strongest, this) {
            (Some(Protection::Hard), _) | (_, Protection::Hard) => Protection::Hard,
            (Some(Protection::FolderOnly), _) | (_, Protection::FolderOnly) => {
                Protection::FolderOnly
            }
            _ => Protection::HandPickOnly,
        });
    }
    strongest
}

/// Checks every request and returns what would happen. Touches nothing.
pub fn prepare(ctx: &Context, requests: &[Request], mode: Mode) -> Plan {
    let mut plan = Plan {
        mode,
        items: Vec::new(),
        skipped: Vec::new(),
        bytes: 0,
        undoable: mode == Mode::Quarantine,
    };
    for req in requests {
        match check(ctx, req, mode) {
            Ok(planned) => {
                plan.bytes += planned.bytes;
                plan.items.push(planned);
            }
            Err(reason) => plan.skipped.push(Skipped {
                path: req.path.clone(),
                reason,
            }),
        }
    }
    plan
}

pub(super) fn check(ctx: &Context, req: &Request, mode: Mode) -> Result<Planned, Reason> {
    let (identity, warnings) = vet(ctx, req, mode)?;
    if req.scanned.is_some_and(|s| !matches_scan(&req.path, &s)) {
        return Err(Reason::ChangedSinceScan);
    }
    Ok(Planned {
        request: req.clone(),
        identity,
        bytes: crate::quarantine::size_on_disk(&req.path),
        warnings,
    })
}

/// Every check except sizing; run when planning and again right before acting, so an item that
/// became protected, excluded, open or iCloud-only after the preview is still refused.
pub(super) fn vet(
    ctx: &Context,
    req: &Request,
    mode: Mode,
) -> Result<(Identity, Vec<Warning>), Reason> {
    let md = fs::symlink_metadata(&req.path).map_err(|_| Reason::NotFound)?;
    let path = resolve(&req.path).ok_or(Reason::NotFound)?;
    match protection_of(ctx, &path) {
        Some(Protection::Hard) | Some(Protection::FolderOnly) => return Err(Reason::Protected),
        Some(Protection::HandPickOnly) if !req.hand_picked || mode == Mode::DeleteNow => {
            return Err(Reason::HandPickOnly)
        }
        _ => {}
    }
    if ctx.exclusions.contains(&path) || ctx.exclusions.contains(&req.path) {
        return Err(Reason::Excluded);
    }
    if let Some(root) = &req.root {
        let root = fs::canonicalize(root).map_err(|_| Reason::OutsideRoot)?;
        let root = root
            .strip_prefix("/System/Volumes/Data")
            .map(|p| Path::new("/").join(p))
            .unwrap_or(root);
        if !path.starts_with(&root) || path == root {
            return Err(Reason::OutsideRoot);
        }
    }
    let id = identity(&md);
    if id.dev != ctx.startup_dev {
        return Err(Reason::OtherVolume);
    }
    if ctx.uid.is_some() && id.uid != ctx.uid {
        return Err(Reason::OtherUser);
    }
    if platform::is_dataless(&md) {
        return Err(Reason::InCloudOnly);
    }
    if mode == Mode::DeleteNow && req.safety != Safety::Safe {
        return Err(Reason::NotSafeForDeleteNow);
    }
    if req.safety == Safety::YourCall && !id.is_dir && platform::is_open(&req.path) {
        return Err(Reason::InUse);
    }
    if is_app_bundle(&path) && app_running(&req.path, &path) {
        return Err(Reason::AppRunning);
    }
    let mut warnings = Vec::new();
    if ctx.synced.iter().any(|s| path.starts_with(s)) {
        warnings.push(Warning::SyncedToICloud);
    }
    Ok((id, warnings))
}

/// The live file still has the size and modification time the scan recorded.
pub(super) fn matches_scan(path: &Path, scanned: &Scanned) -> bool {
    let Ok(md) = fs::symlink_metadata(path) else {
        return false;
    };
    // A link is a different thing than the file the scan listed.
    if md.file_type().is_symlink() {
        return false;
    }
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    crate::quarantine::size_on_disk(path) == scanned.bytes && mtime == scanned.mtime
}

/// Same file as planned? Safe cache folders may change size; "your call" items must not change.
pub(super) fn still_same(planned: &Planned) -> bool {
    let Ok(md) = fs::symlink_metadata(&planned.request.path) else {
        return false;
    };
    let now = identity(&md);
    let was = planned.identity;
    let same = now.dev == was.dev
        && now.ino == was.ino
        && now.uid == was.uid
        && now.is_dir == was.is_dir
        && now.is_symlink == was.is_symlink;
    let unchanged = planned.request.safety == Safety::Safe && was.is_dir
        || (now.len == was.len && now.mtime == was.mtime);
    same && unchanged
}

pub(super) fn is_app_bundle(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "app")
}

pub(super) fn app_running(given: &Path, resolved: &Path) -> bool {
    platform::running_executables()
        .iter()
        .any(|e| platform::is_app_process(given, e) || platform::is_app_process(resolved, e))
}
