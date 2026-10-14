use super::*;
use std::io::Write;
use std::time::SystemTime;

fn setup(tag: &str) -> (PathBuf, Context) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-safety-{tag}-{nanos}"));
    let home = base.join("home");
    for d in [
        "Documents",
        "Desktop",
        "Downloads",
        "Library/Caches/app",
        "Library/Keychains",
    ] {
        fs::create_dir_all(home.join(d)).unwrap();
    }
    fs::write(home.join("Library/Caches/app/blob"), vec![1u8; 4000]).unwrap();
    fs::write(home.join("Documents/report.pdf"), b"report").unwrap();
    fs::write(home.join("Downloads/old.dmg"), vec![2u8; 3000]).unwrap();
    fs::write(home.join("Library/Keychains/login.keychain-db"), b"secret").unwrap();
    let ctx = Context::new(home, base.join("data")).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    (base, ctx)
}

fn req(path: PathBuf, safety: Safety) -> Request {
    Request {
        path,
        safety,
        rule_id: None,
        root: None,
        hand_picked: false,
        scanned: None,
    }
}

#[test]
fn protected_and_hand_pick_rules() {
    let (base, ctx) = setup("protect");
    let h = &ctx.home;
    let plan = prepare(
        &ctx,
        &[
            req(
                h.join("Library/Keychains/login.keychain-db"),
                Safety::YourCall,
            ),
            req(h.join("Documents"), Safety::YourCall),
            req(h.join("Documents/report.pdf"), Safety::YourCall),
            req(h.join("Downloads"), Safety::YourCall),
            req(h.join("Downloads/old.dmg"), Safety::YourCall),
            req(h.join("Library/Caches/app"), Safety::Safe),
            req(h.clone(), Safety::YourCall),
            req(ctx.data_dir.join("Quarantine"), Safety::YourCall),
        ],
        Mode::Quarantine,
    );
    let skipped: Vec<_> = plan
        .skipped
        .iter()
        .map(|s| (s.path.strip_prefix(&base).unwrap().to_path_buf(), s.reason))
        .collect();
    assert!(skipped.contains(&(
        PathBuf::from("home/Library/Keychains/login.keychain-db"),
        Reason::Protected
    )));
    assert!(skipped.contains(&(PathBuf::from("home/Documents"), Reason::Protected)));
    assert!(skipped.contains(&(
        PathBuf::from("home/Documents/report.pdf"),
        Reason::HandPickOnly
    )));
    assert!(skipped.contains(&(PathBuf::from("home/Downloads"), Reason::Protected)));
    assert!(skipped.contains(&(PathBuf::from("home"), Reason::Protected)));
    assert!(skipped.contains(&(PathBuf::from("data/Quarantine"), Reason::Protected)));
    assert_eq!(plan.items.len(), 2, "old.dmg and the cache folder");

    // Picked by hand, a Documents item may go to quarantine, never Delete now.
    let mut picked = req(h.join("Documents/report.pdf"), Safety::YourCall);
    picked.hand_picked = true;
    assert_eq!(
        prepare(&ctx, std::slice::from_ref(&picked), Mode::Quarantine)
            .items
            .len(),
        1
    );
    assert_eq!(
        prepare(&ctx, &[picked], Mode::DeleteNow).skipped[0].reason,
        Reason::HandPickOnly
    );
    crate::platform::remove_test_dir(&base);
}

#[test]
fn delete_now_only_for_safe_and_dry_run_touches_nothing() {
    let (base, ctx) = setup("modes");
    let h = &ctx.home;
    let your_call = prepare(
        &ctx,
        &[req(h.join("Downloads/old.dmg"), Safety::YourCall)],
        Mode::DeleteNow,
    );
    assert_eq!(your_call.skipped[0].reason, Reason::NotSafeForDeleteNow);
    assert!(!your_call.undoable);

    let plan = prepare(
        &ctx,
        &[req(h.join("Library/Caches/app"), Safety::Safe)],
        Mode::DeleteNow,
    );
    let dry = remove(&ctx, &plan, true).unwrap();
    assert_eq!(dry.done.len(), 1);
    assert!(
        h.join("Library/Caches/app/blob").exists(),
        "dry-run changes nothing"
    );
    let real = remove(&ctx, &plan, false).unwrap();
    assert_eq!(real.done.len(), 1);
    assert!(!h.join("Library/Caches/app").exists());
    assert!(ctx.audit.recent(10).len() >= 2);
    crate::platform::remove_test_dir(&base);
}

#[test]
fn changed_items_are_skipped_at_the_last_moment() {
    let (base, ctx) = setup("changed");
    let file = ctx.home.join("Downloads/old.dmg");
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    // The file is replaced between preview and action (a different file, same name).
    crate::platform::remove_test_file(&file);
    fs::write(&file, b"something else").unwrap();
    let report = remove(&ctx, &plan, false).unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::Changed);
    assert_eq!(fs::read(&file).unwrap(), b"something else");
    crate::platform::remove_test_dir(&base);
}

/// Large & Old sends the scan's size and mtime. A file that no longer matches is skipped
/// and stays where it is; one that still matches is moved.
#[test]
fn a_file_changed_since_the_scan_is_skipped_and_reported() {
    let (base, ctx) = setup("since-scan");
    let shot = |p: &Path| {
        let md = fs::symlink_metadata(p).unwrap();
        Scanned {
            bytes: crate::quarantine::size_on_disk(p),
            mtime: md
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    };
    let file = ctx.home.join("Downloads/old.dmg");
    let original = shot(&file);
    let mut changed = req(file.clone(), Safety::YourCall);
    changed.hand_picked = true;
    changed.scanned = Some(original);
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(original.mtime - 86_400),
        )
        .unwrap();
    let plan = prepare(&ctx, &[changed], Mode::Quarantine);
    assert!(plan.items.is_empty());
    assert_eq!(plan.skipped[0].reason, Reason::ChangedSinceScan);
    let report = remove(&ctx, &plan, false).unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::ChangedSinceScan);
    assert!(file.exists(), "a changed file stays put");

    let grown = ctx.home.join("Downloads/grown.bin");
    fs::write(&grown, vec![3u8; 1000]).unwrap();
    let before = shot(&grown);
    fs::OpenOptions::new()
        .append(true)
        .open(&grown)
        .unwrap()
        .write_all(&vec![9u8; 1_000_000])
        .unwrap();
    fs::File::options()
        .write(true)
        .open(&grown)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(before.mtime))
        .unwrap();
    let mut sized = req(grown.clone(), Safety::YourCall);
    sized.hand_picked = true;
    sized.scanned = Some(before);
    let plan = prepare(&ctx, &[sized.clone()], Mode::Quarantine);
    assert_eq!(plan.skipped[0].reason, Reason::ChangedSinceScan);
    assert!(grown.exists());

    sized.scanned = Some(shot(&grown));
    let plan = prepare(&ctx, &[sized], Mode::Quarantine);
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    let report = remove(&ctx, &plan, false).unwrap();
    assert_eq!(report.done.len(), 1);
    assert!(!grown.exists());
    crate::platform::remove_test_dir(&base);
}

fn skip_logged(ctx: &Context, path: &Path, result: &str) -> bool {
    ctx.audit.recent(50).iter().any(|r| {
        r["action"] == "skip"
            && r["target"] == path.to_string_lossy().as_ref()
            && r["result"] == result
    })
}

#[test]
fn edited_items_are_skipped_and_reported() {
    let (base, ctx) = setup("edited");
    let file = ctx.home.join("Downloads/old.dmg");
    // Same file (same inode), new contents.
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(b"more")
        .unwrap();
    let report = remove(&ctx, &plan, false).unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::Changed);
    assert!(skip_logged(&ctx, &file, "changed since preview"));
    // Same size, only the modified date moved: still "changed".
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(SystemTime::now() - std::time::Duration::from_secs(86_400 * 3))
        .unwrap();
    let report = remove(&ctx, &plan, false).unwrap();
    assert_eq!(report.skipped[0].reason, Reason::Changed);
    assert_eq!(
        fs::metadata(&file).unwrap().len(),
        3004,
        "left exactly as it was"
    );
    crate::platform::remove_test_dir(&base);
}

#[test]
fn items_swapped_for_a_link_are_skipped_and_reported() {
    let (base, ctx) = setup("swapped");
    let h = ctx.home.clone();
    // A "your call" file becomes a link to a private file.
    let file = h.join("Downloads/old.dmg");
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    crate::platform::remove_test_file(&file);
    std::os::unix::fs::symlink(h.join("Documents/report.pdf"), &file).unwrap();
    let report = remove(&ctx, &plan, false).unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::Changed);
    assert!(skip_logged(&ctx, &file, "changed since preview"));
    assert!(fs::symlink_metadata(&file)
        .unwrap()
        .file_type()
        .is_symlink());
    // A safe cache folder becomes a link to the Keychains folder.
    let cache = h.join("Library/Caches/app");
    let mut r = req(cache.clone(), Safety::Safe);
    r.root = Some(h.join("Library/Caches"));
    let plan = prepare(&ctx, &[r], Mode::DeleteNow);
    assert_eq!(plan.items.len(), 1);
    crate::platform::remove_test_file(&cache.join("blob"));
    fs::remove_dir(&cache).unwrap();
    std::os::unix::fs::symlink(h.join("Library/Keychains"), &cache).unwrap();
    let report = remove(&ctx, &plan, false).unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::Changed);
    assert_eq!(
        fs::read(h.join("Library/Keychains/login.keychain-db")).unwrap(),
        b"secret"
    );
    assert_eq!(fs::read(h.join("Documents/report.pdf")).unwrap(), b"report");
    crate::platform::remove_test_dir(&base);
}

#[test]
fn never_touch_is_refused_by_remove_and_delegate() {
    let (base, mut ctx) = setup("nevertouch");
    let h = ctx.home.clone();
    // Added to the list after the preview: remove still refuses it.
    let file = h.join("Downloads/old.dmg");
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    assert_eq!(plan.items.len(), 1);
    ctx.exclusions.add(h.join("Downloads")).unwrap();
    for dry_run in [true, false] {
        let report = remove(&ctx, &plan, dry_run).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::Excluded);
    }
    assert!(skip_logged(&ctx, &file, "refused at the last moment"));
    assert!(file.exists());
    // The list survives a relaunch.
    let reopened = Context::new(h.clone(), ctx.data_dir.clone()).unwrap();
    assert_eq!(
        prepare(
            &reopened,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine
        )
        .skipped[0]
            .reason,
        Reason::Excluded
    );
    // Delegated: emptying the Trash is refused while the Trash is on the list, before any
    // OS action runs (tests never reach the real one either; see `platform::run_delegated`).
    fs::create_dir_all(h.join(".Trash")).unwrap();
    fs::write(h.join(".Trash/kept.txt"), b"kept").unwrap();
    ctx.exclusions.add(h.join(".Trash")).unwrap();
    for dry_run in [true, false] {
        let err = delegate(&ctx, Delegated::EmptyTrash, dry_run).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
    assert!(h.join(".Trash/kept.txt").exists());
    // Off the list, dry-run reports the size.
    ctx.exclusions.remove(&h.join(".Trash")).unwrap();
    assert!(delegate(&ctx, Delegated::EmptyTrash, true).unwrap() > 0);
    assert_ne!(
        delegate(&ctx, Delegated::EmptyTrash, false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied,
        "past the list check, the test build stops before the OS"
    );
    crate::platform::remove_test_dir(&base);
}

/// Snapshot thinning, brew, Docker and simulators: dry run reports them and does not run them.
/// A request of 0 bytes is refused before any program starts.
#[test]
fn delegated_cleanups_dry_run_without_running() {
    let (base, ctx) = setup("delegate-preview");
    let err = delegate(&ctx, Delegated::ThinLocalSnapshots { bytes: 0 }, true).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(err.to_string().contains("no space held by snapshots"));
    assert_eq!(
        delegate(&ctx, Delegated::ThinLocalSnapshots { bytes: 1000 }, true).unwrap(),
        1000
    );
    assert!(delegate(&ctx, Delegated::BrewCleanup, true).is_ok());
    assert!(delegate(&ctx, Delegated::DockerPrune, true).is_ok());
    assert!(delegate(&ctx, Delegated::SimctlDeleteUnavailable, true).is_ok());
    assert_eq!(
        Delegated::ThinLocalSnapshots { bytes: 1000 }.action_name(),
        "thinSnapshots"
    );
    assert_eq!(Delegated::BrewCleanup.action_name(), "brewCleanup");
    assert_eq!(Delegated::DockerPrune.action_name(), "dockerPrune");
    assert_eq!(
        Delegated::SimctlDeleteUnavailable.action_name(),
        "deleteUnavailableSimulators"
    );
    assert_eq!(Delegated::EmptyTrash.action_name(), "emptyTrash");
    crate::platform::remove_test_dir(&base);
}

/// A "your call" file another process has open is skipped, at preview and at the last moment.
#[test]
fn open_files_are_skipped() {
    let (base, ctx) = setup("openfile");
    let file = ctx.home.join("Downloads/old.dmg");
    let hold = || {
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .stdin(fs::File::open(&file).unwrap())
            .spawn()
            .unwrap()
    };
    // Open before the preview.
    let mut child = hold();
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    assert_eq!(plan.skipped[0].reason, Reason::InUse);
    let _ = child.kill();
    let _ = child.wait();
    // Closed at preview, opened before the action.
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    assert_eq!(plan.items.len(), 1);
    let mut child = hold();
    let report = remove(&ctx, &plan, false).unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped[0].reason, Reason::InUse);
    assert!(file.exists());
    // Closed again: it goes.
    let plan = prepare(
        &ctx,
        &[req(file.clone(), Safety::YourCall)],
        Mode::Quarantine,
    );
    assert_eq!(remove(&ctx, &plan, false).unwrap().done.len(), 1);
    crate::platform::remove_test_dir(&base);
}

#[test]
fn symlinks_cant_escape_a_root_or_reach_protected_places() {
    let (base, ctx) = setup("links");
    let h = &ctx.home;
    // A cache root containing a link to the Keychains folder.
    std::os::unix::fs::symlink(
        h.join("Library/Keychains"),
        h.join("Library/Caches/app/escape"),
    )
    .unwrap();
    let mut through_link = req(
        h.join("Library/Caches/app/escape/login.keychain-db"),
        Safety::Safe,
    );
    through_link.root = Some(h.join("Library/Caches/app"));
    let plan = prepare(&ctx, &[through_link], Mode::DeleteNow);
    assert_eq!(plan.skipped[0].reason, Reason::Protected);
    // The link itself can go: it's removed as a link, the folder it points to stays.
    let mut link = req(h.join("Library/Caches/app/escape"), Safety::Safe);
    link.root = Some(h.join("Library/Caches/app"));
    let plan = prepare(&ctx, &[link], Mode::DeleteNow);
    remove(&ctx, &plan, false).unwrap();
    assert!(h.join("Library/Keychains/login.keychain-db").exists());
    // An item outside its rule's root is refused.
    let mut outside = req(h.join("Downloads/old.dmg"), Safety::Safe);
    outside.root = Some(h.join("Library/Caches"));
    assert_eq!(
        prepare(&ctx, &[outside], Mode::DeleteNow).skipped[0].reason,
        Reason::OutsideRoot
    );
    crate::platform::remove_test_dir(&base);
}

#[test]
fn exclusions_are_enforced() {
    let (base, mut ctx) = setup("exclude");
    ctx.exclusions.add(ctx.home.join("Downloads")).unwrap();
    let plan = prepare(
        &ctx,
        &[req(ctx.home.join("Downloads/old.dmg"), Safety::YourCall)],
        Mode::Quarantine,
    );
    assert_eq!(plan.skipped[0].reason, Reason::Excluded);
    crate::platform::remove_test_dir(&base);
}

/// PLAN.md Phase 2 checkpoint: clean a fixture home with every bundled rule (all to quarantine),
/// check protected and personal files are untouched, restore everything, compare byte for byte.
#[test]
fn fixture_home_clean_and_restore_is_byte_identical() {
    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(p) = stack.pop() {
            for e in fs::read_dir(&p).unwrap().flatten() {
                let md = fs::symlink_metadata(e.path()).unwrap();
                if md.is_dir() {
                    stack.push(e.path());
                } else {
                    out.push((
                        e.path().strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(e.path()).unwrap(),
                    ));
                }
            }
        }
        out.sort();
        out
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-safety-e2e-{nanos}"));
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    wrasse_fixtures::mac_home(&home).unwrap();
    let ctx = Context::new(home, base.join("data")).unwrap();
    let h = ctx.home.clone();
    let before = snapshot(&h);

    let (rules, errors) =
        crate::rules::load(crate::rules::bundled_text(), false, &h, &ctx.protected);
    assert!(errors.is_empty(), "{errors:?}");
    let (found, _) = crate::rules::find(&rules, &[], &|p| crate::quarantine::size_on_disk(p));
    let requests: Vec<Request> = found
        .iter()
        .filter(|m| m.delegate.is_none())
        .map(|m| Request {
            path: m.path.clone(),
            safety: m.safety,
            rule_id: Some(m.rule_id.clone()),
            root: Some(m.root.clone()),
            hand_picked: false,
            scanned: None,
        })
        .collect();
    assert_eq!(requests.len(), 9, "{found:#?}");
    let plan = prepare(&ctx, &requests, Mode::Quarantine);
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    let report = remove(&ctx, &plan, false).unwrap();
    assert_eq!(report.done.len(), 9);

    // Personal and protected files are all still there; the junk is gone.
    for keep in [
        "Documents/thesis.docx",
        "Desktop/todo.txt",
        "Library/Keychains/login.keychain-db",
        "Library/Mail/V10/mailbox",
        "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
        "Downloads/Recent.dmg",
        "Downloads/notes.txt",
        ".Trash/deleted-file.txt",
    ] {
        assert!(h.join(keep).exists(), "{keep} must not be touched");
    }
    assert!(!h.join("Downloads/OldInstaller.dmg").exists());
    assert!(!h.join("Library/Caches/com.spotify.client").exists());

    // After "relaunching", everything restores byte for byte.
    let reopened = Context::new(h.clone(), ctx.data_dir.clone()).unwrap();
    let ids: Vec<String> = report.done.iter().filter_map(|d| d.entry.clone()).collect();
    let back = restore(&reopened, &ids).unwrap();
    assert_eq!(back.len(), 9);
    assert_eq!(snapshot(&h), before);
    crate::platform::remove_test_dir(&base);
}

#[test]
fn remove_own_data_empties_the_data_folder_only() {
    let (base, ctx) = setup("owndata");
    // Something in the tray, a saved scan, settings and a Never touch entry.
    let plan = prepare(
        &ctx,
        &[req(ctx.home.join("Downloads/old.dmg"), Safety::YourCall)],
        Mode::Quarantine,
    );
    remove(&ctx, &plan, false).unwrap();
    fs::write(ctx.data_dir.join("scan-cache.json"), b"{}").unwrap();
    crate::settings::save(&ctx.data_dir, crate::settings::Settings { keep_days: 3 }).unwrap();

    let preview = remove_own_data(&ctx, true).unwrap();
    assert_eq!(preview.tray_items, 1);
    assert!(preview.tray_bytes >= 3000 && preview.bytes >= preview.tray_bytes);
    assert!(
        ctx.data_dir.join("scan-cache.json").exists(),
        "dry run removes nothing"
    );

    let done = remove_own_data(&ctx, false).unwrap();
    assert!(done.bytes > 0);
    assert_eq!(
        fs::read_dir(&ctx.data_dir).unwrap().count(),
        0,
        "data folder empty"
    );
    // The home folder is untouched (the tray item is gone for good, as the sheet says).
    assert!(ctx.home.join("Documents/report.pdf").exists());
    assert!(!ctx.home.join("Downloads/old.dmg").exists());
    crate::platform::remove_test_dir(&base);
}

#[test]
fn remove_own_data_refuses_a_folder_that_isnt_wrasses() {
    let (base, mut ctx) = setup("notours");
    ctx.data_dir = ctx.home.join("Downloads");
    let err = remove_own_data(&ctx, false).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(ctx.home.join("Downloads/old.dmg").exists());
    crate::platform::remove_test_dir(&base);
}
