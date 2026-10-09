//! Duplicate tests (docs/phase3-plan.md §7). Under `safety/` because they make an APFS clone with
//! `cp -c` and remove copies through the tray.

use crate::files::dupes::{self, Control, MIN_BYTES};
use crate::safety::{self, Context};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Setup {
    base: PathBuf,
    home: PathBuf,
    ctx: Context,
}

impl Drop for Setup {
    fn drop(&mut self) {
        crate::platform::remove_test_dir(&self.base);
    }
}

fn setup(tag: &str) -> Setup {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-dupes-{tag}-{nanos}"));
    fs::create_dir_all(base.join("home/Downloads")).unwrap();
    fs::create_dir_all(base.join("home/Documents")).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    let ctx = Context::new(base.join("home"), base.join("data")).unwrap();
    Setup {
        home: ctx.home.clone(),
        base,
        ctx,
    }
}

/// 12 MB of deterministic, non-repeating bytes.
fn data(seed: u64) -> Vec<u8> {
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..12_000_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

fn today() -> u32 {
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86_400) as u32
}

fn candidates(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let md = fs::symlink_metadata(e.path()).unwrap();
            if md.is_dir() {
                stack.push(e.path());
            } else {
                out.push((e.path(), md.len()));
            }
        }
    }
    out
}

#[test]
fn size_partial_and_full_hash_with_links_clones_and_hand_pick() {
    let s = setup("find");
    let dl = s.home.join("Downloads");
    let original = data(1);
    fs::write(dl.join("a.bin"), &original).unwrap();
    fs::write(dl.join("b.bin"), &original).unwrap();
    fs::write(s.home.join("Documents/kept.bin"), &original).unwrap();
    // Same size, differs at a byte the partial hash doesn't sample: partial matches, full doesn't.
    let mut near = original.clone();
    near[200_000] ^= 0xFF;
    fs::write(dl.join("near.bin"), &near).unwrap();
    assert_eq!(
        dupes::partial_hash(&dl.join("a.bin"), 12_000_000).unwrap(),
        dupes::partial_hash(&dl.join("near.bin"), 12_000_000).unwrap()
    );
    // A hard link is the same file, not a copy.
    fs::hard_link(dl.join("a.bin"), dl.join("a-link.bin")).unwrap();
    // An APFS clone: identical, but removing it frees nothing.
    let ok = std::process::Command::new("/bin/cp")
        .args([
            "-c",
            dl.join("a.bin").to_str().unwrap(),
            dl.join("clone.bin").to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(ok.success());
    // Below the minimum size: never checked.
    fs::write(dl.join("small-1.bin"), vec![9u8; (MIN_BYTES / 2) as usize]).unwrap();
    fs::write(dl.join("small-2.bin"), vec![9u8; (MIN_BYTES / 2) as usize]).unwrap();
    // Another identical pair of a different file.
    let other = data(2);
    fs::write(dl.join("x.bin"), &other).unwrap();
    fs::write(dl.join("y.bin"), &other).unwrap();

    let docs = s.home.join("Documents");
    let hand = move |p: &Path| p.starts_with(&docs);
    let groups = dupes::find(candidates(&s.home), &hand, today(), &Control::default());
    assert_eq!(groups.len(), 2, "{groups:#?}");
    let g = groups
        .iter()
        .find(|g| g.copies.iter().any(|c| c.path.ends_with("b.bin")))
        .unwrap();
    let names: Vec<String> = g
        .copies
        .iter()
        .map(|c| c.path.strip_prefix(&s.home).unwrap().display().to_string())
        .collect();
    assert_eq!(
        names.len(),
        4,
        "a (or its link, once), b, clone, Documents copy: {names:?}"
    );
    assert!(
        names
            .iter()
            .filter(|n| n.starts_with("Downloads/a"))
            .count()
            == 1
    );
    assert!(!names
        .iter()
        .any(|n| n.contains("near") || n.contains("small")));
    let clone = g
        .copies
        .iter()
        .find(|c| c.path.ends_with("clone.bin"))
        .unwrap();
    assert!(clone.clone, "cp -c makes an APFS clone");
    assert!(
        g.copies
            .iter()
            .find(|c| c.path.ends_with("kept.bin"))
            .unwrap()
            .hand_pick_only
    );
    assert_eq!(
        g.reclaimable,
        2 * 12_000_000,
        "four copies, one kept, the clone frees nothing"
    );
    assert_eq!(g.bytes, 12_000_000);
}

#[test]
fn removal_rehashes_and_keeps_the_chosen_copy() {
    let s = setup("remove");
    let dl = s.home.join("Downloads");
    let original = data(3);
    for n in ["keep.bin", "b.bin", "c.bin"] {
        fs::write(dl.join(n), &original).unwrap();
    }
    let keep = dl.join("keep.bin");
    // The kept copy changed after the search: nothing is removed.
    let mut changed = original.clone();
    changed[6_000_123] ^= 1;
    fs::write(&keep, &changed).unwrap();
    assert!(dupes::remove(&s.ctx, &keep, &[dl.join("b.bin")], false).is_err());
    assert!(dl.join("b.bin").exists());
    fs::write(&keep, &original).unwrap();
    // Asking to remove the kept copy itself is refused.
    assert!(dupes::remove(&s.ctx, &keep, std::slice::from_ref(&keep), false).is_err());
    // Dry run touches nothing.
    let dry = dupes::remove(&s.ctx, &keep, &[dl.join("b.bin"), dl.join("c.bin")], true).unwrap();
    assert_eq!(dry.done.len(), 2);
    assert!(dl.join("b.bin").exists());
    let report =
        dupes::remove(&s.ctx, &keep, &[dl.join("b.bin"), dl.join("c.bin")], false).unwrap();
    assert_eq!(report.done.len(), 2);
    assert!(keep.exists() && !dl.join("b.bin").exists() && !dl.join("c.bin").exists());
    assert_eq!(fs::read(&keep).unwrap(), original);
    let ids: Vec<String> = report.done.iter().filter_map(|d| d.entry.clone()).collect();
    safety::restore(&s.ctx, &ids).unwrap();
    assert_eq!(
        fs::read(dl.join("c.bin")).unwrap(),
        original,
        "restored byte for byte"
    );
}
