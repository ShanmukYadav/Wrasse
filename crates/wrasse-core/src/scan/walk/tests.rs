use super::*;
use std::io::Write;

// Test folders live under the OS temp dir; platform::remove_test_dir cleans them up
// (deleting is only allowed in safety/, quarantine/ and platform/).
fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("wrasse-walk-{tag}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, bytes: usize) {
    let mut f = fs::File::create(path).unwrap();
    f.write_all(&vec![7u8; bytes]).unwrap();
    f.sync_all().unwrap();
}

/// Every folder's size is its children plus its aggregated small files.
fn assert_consistent(tree: &Tree, id: u32) {
    let n = tree.node(id);
    if n.kind == Kind::Dir {
        let kids: u64 = tree.children(id).map(|c| tree.node(c).size).sum();
        assert_eq!(n.size, kids + n.small_size, "{}", tree.path(id));
        for c in tree.children(id) {
            assert_consistent(tree, c);
        }
    }
}

#[test]
fn sizes_hard_links_and_aggregation() {
    let root = temp_dir("sizes");
    fs::create_dir_all(root.join("a/b")).unwrap();
    write(&root.join("a/big.bin"), 3_000_000);
    write(&root.join("a/b/small.txt"), 10);
    write(&root.join("a/b/small2.txt"), 5_000);
    fs::hard_link(root.join("a/big.bin"), root.join("a/b/link.bin")).unwrap();

    let tree = scan(&root, &ScanControl::default()).unwrap();
    assert_consistent(&tree, 0);
    // The 3 MB file has two names but is counted once.
    assert_eq!(tree.stats.hardlinks_deduped, 1);
    let total = tree.node(0).size;
    assert!((3_000_000..3_200_000).contains(&total), "total {total}");
    // Exactly one of the two names is stored as a large file.
    let large = ["a/big.bin", "a/b/link.bin"]
        .iter()
        .filter(|p| tree.find(&format!("{}/{p}", root.display())).is_some())
        .count();
    assert_eq!(large, 1);
    let b = tree.find(&format!("{}/a/b", root.display())).unwrap();
    assert!(tree.node(b).small_count >= 2);
    assert_eq!(tree.stats.files, 4);
    crate::platform::remove_test_dir(&root);
}

#[test]
fn unreadable_folders_are_marked() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp_dir("unreadable");
    fs::create_dir_all(root.join("locked")).unwrap();
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let tree = scan(&root, &ScanControl::default()).unwrap();
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    let locked = tree.find(&format!("{}/locked", root.display())).unwrap();
    assert_ne!(tree.node(locked).flags & flag::UNREADABLE, 0);
    assert_eq!(tree.stats.unreadable_dirs, 1);
    crate::platform::remove_test_dir(&root);
}

#[test]
fn skipped_folders_are_not_opened() {
    let root = temp_dir("skip");
    fs::create_dir_all(root.join("guarded/inner")).unwrap();
    write(&root.join("guarded/inner/big.bin"), 3_000_000);
    write(&root.join("open.bin"), 2_000_000);
    let control = ScanControl::default();
    control.skip_folders(vec![root.join("guarded")]);
    let tree = scan(&root, &control).unwrap();
    let guarded = tree.find(&format!("{}/guarded", root.display())).unwrap();
    assert_ne!(tree.node(guarded).flags & flag::UNREADABLE, 0);
    assert_eq!(tree.node(guarded).size, 0);
    assert!(tree
        .find(&format!("{}/guarded/inner", root.display()))
        .is_none());
    assert!(tree.find(&format!("{}/open.bin", root.display())).is_some());
    crate::platform::remove_test_dir(&root);
}

#[test]
fn cancel_stops_early() {
    let root = temp_dir("cancel");
    fs::create_dir_all(root.join("x")).unwrap();
    let control = ScanControl::default();
    control.cancel();
    let tree = scan(&root, &control).unwrap();
    assert!(tree.stats.cancelled);
    assert_eq!(tree.nodes.len(), 1);
    crate::platform::remove_test_dir(&root);
}

#[test]
fn rescan_picks_up_changes_in_changed_folders_only() {
    let root = temp_dir("rescan");
    fs::create_dir_all(root.join("a/deep")).unwrap();
    fs::create_dir_all(root.join("b")).unwrap();
    write(&root.join("a/deep/one.bin"), 2_000_000);
    write(&root.join("b/two.bin"), 3_000_000);
    let first = scan(&root, &ScanControl::default()).unwrap();

    // Change a/ (new file, new folder) and remove b/two.bin.
    write(&root.join("a/new.bin"), 4_000_000);
    fs::create_dir_all(root.join("a/fresh")).unwrap();
    write(&root.join("a/fresh/x.bin"), 1_500_000);
    crate::platform::remove_test_file(&root.join("b/two.bin"));
    let changes = [
        Change {
            path: root.join("a"),
            recursive: false,
        },
        Change {
            path: root.join("b"),
            recursive: false,
        },
    ];
    let updated = rescan(&first, &changes, &ScanControl::default()).unwrap();
    let full = scan(&root, &ScanControl::default()).unwrap();
    assert_eq!(updated.node(0).size, full.node(0).size);
    assert!(updated
        .find(&format!("{}/a/fresh/x.bin", root.display()))
        .is_some());
    assert!(updated
        .find(&format!("{}/a/deep/one.bin", root.display()))
        .is_some());
    assert!(updated
        .find(&format!("{}/b/two.bin", root.display()))
        .is_none());
    assert_consistent(&updated, 0);
    crate::platform::remove_test_dir(&root);
}

#[test]
fn power_pauses_and_resumes_unless_overridden() {
    use crate::platform::PowerState;
    let c = ScanControl::default();
    let low = PowerState {
        on_battery: true,
        percent: 15,
        low_power: false,
    };
    let fine = PowerState {
        on_battery: true,
        percent: 60,
        low_power: false,
    };
    let lpm = PowerState {
        on_battery: false,
        percent: 100,
        low_power: true,
    };
    c.apply_power(low);
    assert_eq!(c.pause_reason(), Some(PauseReason::Battery));
    c.apply_power(fine);
    assert_eq!(c.pause_reason(), None);
    c.apply_power(lpm);
    assert_eq!(c.pause_reason(), Some(PauseReason::LowPowerMode));
    // The user resumes: power no longer pauses this scan.
    c.set_paused(false);
    c.apply_power(low);
    assert_eq!(c.pause_reason(), None);
    // A hand pause isn't undone by power changes.
    let d = ScanControl::default();
    d.set_paused(true);
    d.apply_power(fine);
    assert_eq!(d.pause_reason(), Some(PauseReason::User));
}
