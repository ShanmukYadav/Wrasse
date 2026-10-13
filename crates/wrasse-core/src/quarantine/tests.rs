use super::*;

fn scratch(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("wrasse-q-{tag}-{nanos}"));
    fs::create_dir_all(&d).unwrap();
    d
}

fn tree_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).unwrap().flatten() {
            let md = e.metadata().unwrap();
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

#[test]
fn store_restore_is_byte_identical_and_survives_reopening() {
    let base = scratch("roundtrip");
    let home = base.join("home");
    fs::create_dir_all(home.join("Caches/app/sub")).unwrap();
    fs::write(home.join("Caches/app/a.bin"), vec![9u8; 50_000]).unwrap();
    fs::write(home.join("Caches/app/sub/b.txt"), b"hello").unwrap();
    fs::write(home.join("old.dmg"), vec![1u8; 10_000]).unwrap();
    let before = tree_bytes(&home);

    let q = Quarantine::open(base.join("Quarantine")).unwrap();
    let mut w = q.begin().unwrap();
    let a = w
        .store(&home.join("Caches/app"), 60_000, Some("test-cache"))
        .unwrap();
    let b = w.store(&home.join("old.dmg"), 10_000, None).unwrap();
    w.finish().unwrap();
    assert!(!home.join("Caches/app").exists() && !home.join("old.dmg").exists());

    // A fresh handle (as after quitting and relaunching) still sees and restores them.
    let q2 = Quarantine::open(base.join("Quarantine")).unwrap();
    assert_eq!(q2.stored().len(), 2);
    let restored = q2.restore(&[a, b]).unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(tree_bytes(&home), before);
    assert!(
        q2.batches().is_empty(),
        "a fully restored batch leaves nothing behind"
    );
    crate::platform::remove_test_dir(&base);
}

#[test]
fn restore_beside_an_occupied_spot_and_purge() {
    let base = scratch("occupied");
    fs::write(base.join("note.txt"), b"one").unwrap();
    fs::write(base.join("junk.log"), b"junk").unwrap();
    let q = Quarantine::open(base.join("Quarantine")).unwrap();
    let mut w = q.begin().unwrap();
    let note = w.store(&base.join("note.txt"), 3, None).unwrap();
    let junk = w.store(&base.join("junk.log"), 4, None).unwrap();
    w.finish().unwrap();
    fs::write(base.join("note.txt"), b"new").unwrap();
    let back = q.restore(&[note]).unwrap();
    assert_eq!(back[0].1, base.join("note (restored).txt"));
    assert_eq!(fs::read(base.join("note.txt")).unwrap(), b"new");
    let purged = q.purge(&[junk]).unwrap();
    assert_eq!(purged.len(), 1);
    assert!(q.stored().is_empty());
    crate::platform::remove_test_dir(&base);
}

#[test]
fn reconcile_settles_entries_a_crash_left_pending() {
    let base = scratch("reconcile");
    fs::write(base.join("moved.txt"), b"m").unwrap();
    fs::write(base.join("unmoved.txt"), b"u").unwrap();
    let q = Quarantine::open(base.join("Quarantine")).unwrap();
    let w = q.begin().unwrap();
    // Simulate a crash: two entries written as pending; one file was renamed, one wasn't.
    let mut batch = w.batch.clone();
    for (n, name) in ["moved.txt", "unmoved.txt"].iter().enumerate() {
        batch.entries.push(Entry {
            id: format!("{}/{n}", batch.id),
            original: base.join(name),
            stored: format!("{ITEMS}/{n}-{name}"),
            bytes: 1,
            rule_id: None,
            state: State::Pending,
            restored_to: None,
        });
    }
    write_manifest(&w.dir, &batch).unwrap();
    fs::rename(
        base.join("moved.txt"),
        w.dir.join(format!("{ITEMS}/0-moved.txt")),
    )
    .unwrap();
    drop(w);

    let report = q.reconcile().unwrap();
    assert_eq!(
        report,
        ReconcileReport {
            completed: 1,
            rolled_back: 1,
            missing: 0
        }
    );
    let stored = q.stored();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].1.original, base.join("moved.txt"));
    assert!(base.join("unmoved.txt").exists());
    crate::platform::remove_test_dir(&base);
}

/// Kills a real process in the middle of a batch, then reconciles: every file is either back
/// in place or restorable from quarantine, and restoring gives the original tree back.
#[test]
fn crash_mid_batch_loses_nothing() {
    if let Ok(dir) = std::env::var("WRASSE_CRASH_CHILD") {
        // Child: quarantine all 40 files slowly, until killed.
        let base = PathBuf::from(dir);
        let q = Quarantine::open(base.join("Quarantine")).unwrap();
        let mut w = q.begin().unwrap();
        for i in 0..40 {
            w.store(&base.join("home").join(format!("f{i:02}.bin")), 1000, None)
                .unwrap();
        }
        w.finish().unwrap();
        return;
    }
    let base = scratch("crash");
    fs::create_dir_all(base.join("home")).unwrap();
    for i in 0..40 {
        fs::write(
            base.join("home").join(format!("f{i:02}.bin")),
            vec![i as u8; 1000],
        )
        .unwrap();
    }
    let before = tree_bytes(&base.join("home"));
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args([
            "--exact",
            "quarantine::tests::crash_mid_batch_loses_nothing",
            "--nocapture",
        ])
        .env("WRASSE_CRASH_CHILD", &base)
        .env("WRASSE_TEST_PAUSE_MS", "25")
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    child.kill().unwrap();
    child.wait().unwrap();

    let q = Quarantine::open(base.join("Quarantine")).unwrap();
    let in_place = fs::read_dir(base.join("home")).unwrap().count();
    let report = q.reconcile().unwrap();
    let stored = q.stored();
    assert!(
        in_place > 0 && !stored.is_empty(),
        "the kill should land mid-batch ({in_place} in place)"
    );
    assert_eq!(
        in_place + stored.len(),
        40,
        "every file is either in place or restorable"
    );
    assert_eq!(report.missing, 0);
    assert!(q
        .batches()
        .iter()
        .all(|b| b.entries.iter().all(|e| e.state != State::Pending)));
    let ids: Vec<String> = stored.iter().map(|(_, e)| e.id.clone()).collect();
    q.restore(&ids).unwrap();
    assert_eq!(tree_bytes(&base.join("home")), before);
    crate::platform::remove_test_dir(&base);
}
