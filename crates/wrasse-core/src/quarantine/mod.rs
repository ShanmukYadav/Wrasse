//! Quarantine: items moved aside, restorable, purged after a while (PLAN.md §6.11).
//!
//! Layout: `<data>/Quarantine/<batch>/batch.json` (the manifest) and `<batch>/items/<n>-<name>`.
//! Moves are same-volume renames, so an item is never copied or half-moved.
//!
//! Crash safety: every move is written to the manifest as `pending` *before* the rename and marked
//! `stored` *after* it, each manifest write atomic (temp file, sync, rename). `reconcile`, run at
//! launch, settles any entry a crash left `pending` by looking at where the item actually is, so
//! nothing is lost and nothing is orphaned. Only this module renames into and out of quarantine,
//! and only `purge` deletes from it. Platform-neutral.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST: &str = "batch.json";
const ITEMS: &str = "items";
/// Items stay this long before they're purged automatically (PLAN.md §6.11).
pub const DEFAULT_KEEP_DAYS: u64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Written before the move; a crash may leave it here.
    Pending,
    Stored,
    Restored,
    Purged,
    /// The item was neither in quarantine nor back in place (e.g. removed by hand).
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// "<batch>/<n>": stable id for restore and purge.
    pub id: String,
    pub original: PathBuf,
    /// Relative to the batch folder.
    pub stored: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub state: State,
    /// Where it went back to, if restored (may differ when the original spot was taken).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_to: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: String,
    /// Seconds since the Unix epoch.
    pub created_at: u64,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    pub completed: usize,
    pub rolled_back: usize,
    pub missing: usize,
}

pub struct Quarantine {
    root: PathBuf,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Writes the manifest atomically: a crash leaves either the old or the new one, never half.
fn write_manifest(dir: &Path, batch: &Batch) -> io::Result<()> {
    let tmp = dir.join("batch.json.tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(batch).map_err(io::Error::other)?)?;
    f.sync_all()?;
    fs::rename(&tmp, dir.join(MANIFEST))
}

fn read_manifest(dir: &Path) -> Option<Batch> {
    serde_json::from_slice(&fs::read(dir.join(MANIFEST)).ok()?).ok()
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// Bytes on disk under a path (files and folders), without following symlinks.
pub fn size_on_disk(path: &Path) -> u64 {
    let Ok(md) = fs::symlink_metadata(path) else {
        return 0;
    };
    #[cfg(unix)]
    let own = std::os::unix::fs::MetadataExt::blocks(&md) * 512;
    #[cfg(not(unix))]
    let own = md.len();
    if !md.is_dir() {
        return own;
    }
    own + fs::read_dir(path)
        .map(|es| es.flatten().map(|e| size_on_disk(&e.path())).sum::<u64>())
        .unwrap_or(0)
}

impl Quarantine {
    /// Opens (creating) the quarantine folder.
    pub fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Quarantine { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A new batch; items are added with [`BatchWriter::store`].
    pub(crate) fn begin(&self) -> io::Result<BatchWriter> {
        let created_at = now();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let id = format!("{created_at}-{nanos:09}");
        let dir = self.root.join(&id);
        fs::create_dir_all(dir.join(ITEMS))?;
        let batch = Batch {
            id,
            created_at,
            entries: Vec::new(),
        };
        write_manifest(&dir, &batch)?;
        Ok(BatchWriter { dir, batch })
    }

    fn batch_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = fs::read_dir(&self.root)
            .map(|es| {
                es.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.join(MANIFEST).exists())
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        dirs
    }

    /// Every batch, oldest first.
    pub(crate) fn batches(&self) -> Vec<Batch> {
        self.batch_dirs()
            .iter()
            .filter_map(|d| read_manifest(d))
            .collect()
    }

    /// Items currently in quarantine (restorable).
    pub fn stored(&self) -> Vec<(Batch, Entry)> {
        self.batches()
            .into_iter()
            .flat_map(|b| {
                let entries = b.entries.clone();
                entries
                    .into_iter()
                    .filter(|e| e.state == State::Stored)
                    .map(move |e| (b.clone(), e))
            })
            .collect()
    }

    /// Settles entries a crash left unfinished, by where each item actually is. Run at launch.
    pub fn reconcile(&self) -> io::Result<ReconcileReport> {
        let mut report = ReconcileReport::default();
        for dir in self.batch_dirs() {
            let Some(mut batch) = read_manifest(&dir) else {
                continue;
            };
            let mut changed = false;
            for e in &mut batch.entries {
                let stored = dir.join(&e.stored);
                match e.state {
                    State::Pending => {
                        changed = true;
                        if exists(&stored) && !exists(&e.original) {
                            e.state = State::Stored; // moved, but not yet marked
                            report.completed += 1;
                        } else if exists(&e.original) && !exists(&stored) {
                            e.state = State::Restored; // never moved: it's still in place
                            e.restored_to = Some(e.original.clone());
                            report.rolled_back += 1;
                        } else if exists(&stored) {
                            e.state = State::Stored; // both exist (shouldn't happen): keep both, restorable
                            report.completed += 1;
                        } else {
                            e.state = State::Missing;
                            report.missing += 1;
                        }
                    }
                    State::Stored if !exists(&stored) => {
                        changed = true;
                        if exists(&e.original) {
                            e.state = State::Restored; // a restore finished but wasn't marked
                            e.restored_to = Some(e.original.clone());
                        } else {
                            e.state = State::Missing;
                            report.missing += 1;
                        }
                    }
                    _ => {}
                }
            }
            if changed {
                write_manifest(&dir, &batch)?;
            }
            self.drop_if_done(&dir, &batch)?;
        }
        Ok(report)
    }

    /// Puts items back. If the original spot is taken, the item goes beside it with " (restored)".
    pub fn restore(&self, ids: &[String]) -> io::Result<Vec<(String, PathBuf)>> {
        let mut out = Vec::new();
        for dir in self.batch_dirs() {
            let Some(mut batch) = read_manifest(&dir) else {
                continue;
            };
            let mut changed = false;
            for e in &mut batch.entries {
                if e.state != State::Stored || !ids.contains(&e.id) {
                    continue;
                }
                let target = free_spot(&e.original);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(dir.join(&e.stored), &target)?;
                e.state = State::Restored;
                e.restored_to = Some(target.clone());
                changed = true;
                out.push((e.id.clone(), target));
            }
            if changed {
                write_manifest(&dir, &batch)?;
                self.drop_if_done(&dir, &batch)?;
            }
        }
        Ok(out)
    }

    /// Deletes items from quarantine for good. Not undoable.
    pub fn purge(&self, ids: &[String]) -> io::Result<Vec<(String, u64)>> {
        self.purge_where(|_, e| ids.contains(&e.id))
    }

    /// Purges everything stored longer than `days` (run at launch).
    pub fn purge_older_than(&self, days: u64) -> io::Result<Vec<(String, u64)>> {
        let cutoff = now().saturating_sub(days * 86_400);
        self.purge_where(|b, _| b.created_at < cutoff)
    }

    fn purge_where(&self, pick: impl Fn(&Batch, &Entry) -> bool) -> io::Result<Vec<(String, u64)>> {
        let mut out = Vec::new();
        for dir in self.batch_dirs() {
            let Some(mut batch) = read_manifest(&dir) else {
                continue;
            };
            let snapshot = batch.clone();
            let mut changed = false;
            for e in &mut batch.entries {
                if e.state != State::Stored || !pick(&snapshot, e) {
                    continue;
                }
                let stored = dir.join(&e.stored);
                let md = fs::symlink_metadata(&stored)?;
                if md.is_dir() {
                    fs::remove_dir_all(&stored)?;
                } else {
                    fs::remove_file(&stored)?;
                }
                e.state = State::Purged;
                changed = true;
                out.push((e.id.clone(), e.bytes));
            }
            if changed {
                write_manifest(&dir, &batch)?;
                self.drop_if_done(&dir, &batch)?;
            }
        }
        Ok(out)
    }

    /// A batch with nothing left to restore is removed (its manifest goes with it).
    fn drop_if_done(&self, dir: &Path, batch: &Batch) -> io::Result<()> {
        if batch
            .entries
            .iter()
            .all(|e| matches!(e.state, State::Restored | State::Purged | State::Missing))
        {
            let items = dir.join(ITEMS);
            if fs::read_dir(&items)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true)
            {
                fs::remove_dir_all(dir)?;
            }
        }
        Ok(())
    }
}

/// The original path if free, else "name (restored).ext", "name (restored 2).ext", …
fn free_spot(original: &Path) -> PathBuf {
    if !exists(original) {
        return original.to_path_buf();
    }
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = original
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = original.parent().unwrap_or(Path::new("/"));
    for n in 1.. {
        let suffix = if n == 1 {
            " (restored)".to_string()
        } else {
            format!(" (restored {n})")
        };
        let candidate = parent.join(format!("{stem}{suffix}{ext}"));
        if !exists(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// Adds items to one batch, crash-safely.
pub(crate) struct BatchWriter {
    dir: PathBuf,
    batch: Batch,
}

impl BatchWriter {
    /// Moves one item into quarantine: manifest `pending` → rename → manifest `stored`.
    /// The item must be on the same volume as the quarantine (the caller checks).
    pub fn store(
        &mut self,
        original: &Path,
        bytes: u64,
        rule_id: Option<&str>,
    ) -> io::Result<String> {
        let n = self.batch.entries.len();
        let name = original
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "item".into());
        let stored = format!("{ITEMS}/{n}-{name}");
        let id = format!("{}/{n}", self.batch.id);
        self.batch.entries.push(Entry {
            id: id.clone(),
            original: original.to_path_buf(),
            stored: stored.clone(),
            bytes,
            rule_id: rule_id.map(str::to_string),
            state: State::Pending,
            restored_to: None,
        });
        write_manifest(&self.dir, &self.batch)?;
        if let Some(ms) = test_pause() {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        if let Err(e) = fs::rename(original, self.dir.join(&stored)) {
            self.batch.entries.pop();
            write_manifest(&self.dir, &self.batch)?;
            return Err(e);
        }
        if let Some(last) = self.batch.entries.last_mut() {
            last.state = State::Stored;
        }
        write_manifest(&self.dir, &self.batch)?;
        Ok(id)
    }

    /// Ends the batch; an empty one leaves nothing behind.
    pub(crate) fn finish(self) -> io::Result<Batch> {
        if self.batch.entries.is_empty() {
            fs::remove_dir_all(&self.dir)?;
        }
        Ok(self.batch)
    }
}

/// Crash tests only: `WRASSE_TEST_PAUSE_MS` slows each move so a test can kill the process
/// between the manifest write and the rename.
fn test_pause() -> Option<u64> {
    if cfg!(test) || cfg!(debug_assertions) {
        std::env::var("WRASSE_TEST_PAUSE_MS").ok()?.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
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
}
