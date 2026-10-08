//! Parallel, read-only walk of one volume (PLAN.md §5.1–5.4).
//!
//! - Size on disk is `st_blocks * 512`, so compressed and sparse files count what they store.
//! - Hard links are counted once per `(st_dev, st_ino)`.
//! - iCloud dataless files and folders count 0 bytes and are never opened, so nothing downloads.
//! - Other volumes mounted inside the tree are not entered.
//! - Folders that can't be read are marked unreadable, not counted as empty.
//! - Runs on its own low-priority pool; cancel and pause are checked at every folder.

use super::tree::{flag, Kind, Node, ScanStats, Tree, NONE, SMALL_FILE};
use crate::platform;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Shared between the scan and whoever drives it: cancel, pause and live progress.
#[derive(Default)]
pub struct ScanControl {
    cancel: AtomicBool,
    paused: AtomicBool,
    items: AtomicU64,
    bytes: AtomicU64,
    current: Mutex<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub items: u64,
    pub bytes: u64,
    pub current: String,
}

impl ScanControl {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn progress(&self) -> Progress {
        Progress {
            items: self.items.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }
}

/// Hard links seen so far, sharded so parallel workers rarely contend.
struct LinkSet(Vec<Mutex<HashSet<(u64, u64)>>>);

impl LinkSet {
    fn new() -> Self {
        LinkSet((0..64).map(|_| Mutex::new(HashSet::new())).collect())
    }
    /// True the first time a `(dev, ino)` is seen.
    fn first(&self, dev: u64, ino: u64) -> bool {
        let shard = &self.0[(ino % 64) as usize];
        shard
            .lock()
            .map(|mut s| s.insert((dev, ino)))
            .unwrap_or(true)
    }
}

struct Ctx<'a> {
    control: &'a ScanControl,
    links: LinkSet,
    dev: u64,
    stats: Mutex<ScanStats>,
}

struct FileOut {
    name: Box<str>,
    size: u64,
    last_used_day: u32,
    clone_shared: u64,
    flags: u8,
}

#[derive(Default)]
struct DirOut {
    name: Box<str>,
    size: u64,
    small_size: u64,
    small_count: u32,
    items: u64,
    last_used_day: u32,
    clone_shared: u64,
    flags: u8,
    files: Vec<FileOut>,
    dirs: Vec<DirOut>,
}

/// Scans `root` (not crossing into other volumes) and returns the tree. A cancelled scan
/// returns what was read so far with `stats.cancelled` set.
pub fn scan(root: &Path, control: &ScanControl) -> io::Result<Tree> {
    let md = fs::symlink_metadata(root)?;
    if !md.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scan root is not a folder",
        ));
    }
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .saturating_sub(1)
        .max(1);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("wrasse-scan-{i}"))
        .start_handler(|_| platform::lower_thread_priority())
        .build()
        .map_err(io::Error::other)?;

    let ctx = Ctx {
        control,
        links: LinkSet::new(),
        dev: dev_of(&md),
        stats: Mutex::new(ScanStats::default()),
    };
    let out = pool.install(|| walk_dir(root.to_path_buf(), "".into(), &md, &ctx));
    let mut stats = ctx.stats.into_inner().unwrap_or_default();
    stats.cancelled = control.is_cancelled();
    stats.clone_shared_bytes = out.clone_shared;
    Ok(flatten(out, root.to_string_lossy().into_owned(), stats))
}

fn walk_dir(path: PathBuf, name: Box<str>, own: &fs::Metadata, ctx: &Ctx) -> DirOut {
    let mut out = DirOut {
        name,
        last_used_day: last_used_day(own),
        ..Default::default()
    };
    let control = ctx.control;
    while control.paused.load(Ordering::Relaxed) && !control.is_cancelled() {
        std::thread::sleep(Duration::from_millis(50));
    }
    if control.is_cancelled() {
        return out;
    }
    if let Ok(mut current) = control.current.try_lock() {
        current.clear();
        current.push_str(&path.to_string_lossy());
    }

    let entries = match fs::read_dir(&path) {
        Ok(e) => e,
        Err(_) => {
            out.flags |= flag::UNREADABLE;
            bump(ctx, |s| s.unreadable_dirs += 1);
            return out;
        }
    };

    let mut subdirs: Vec<(PathBuf, Box<str>, fs::Metadata)> = Vec::new();
    let (mut files, mut dirs, mut dataless, mut deduped) = (0u64, 0u64, 0u64, 0u64);
    for entry in entries.flatten() {
        let Ok(md) = entry.metadata() else { continue }; // lstat: symlinks are not followed
        let name: Box<str> = entry.file_name().to_string_lossy().into();
        let used = last_used_day(&md);
        out.items += 1;
        if md.is_dir() {
            dirs += 1;
            if dev_of(&md) != ctx.dev {
                out.dirs.push(DirOut {
                    name,
                    flags: flag::MOUNT,
                    last_used_day: used,
                    ..Default::default()
                });
            } else if platform::is_dataless(&md) {
                // Reading a dataless folder would download it; count it as 0 bytes local.
                dataless += 1;
                out.dirs.push(DirOut {
                    name,
                    flags: flag::DATALESS,
                    last_used_day: used,
                    ..Default::default()
                });
            } else {
                subdirs.push((entry.path(), name, md));
            }
            continue;
        }
        files += 1;
        let mut flags = 0;
        let mut size = allocated(&md);
        if platform::is_dataless(&md) {
            dataless += 1;
            flags |= flag::DATALESS;
            size = 0;
        } else if md.is_file() && nlink(&md) > 1 && !ctx.links.first(dev_of(&md), ino(&md)) {
            deduped += 1;
            flags |= flag::HARDLINK_SEEN;
            size = 0;
        }
        control.items.fetch_add(1, Ordering::Relaxed);
        control.bytes.fetch_add(size, Ordering::Relaxed);
        out.last_used_day = out.last_used_day.max(used);
        if size >= SMALL_FILE {
            let mut clone_shared = 0;
            if md.is_file() {
                if let Some(shared) = platform::clone_shared_bytes(&entry.path(), size) {
                    flags |= flag::CLONE;
                    clone_shared = shared;
                }
            }
            out.size += size;
            out.clone_shared += clone_shared;
            out.files.push(FileOut {
                name,
                size,
                last_used_day: used,
                clone_shared,
                flags,
            });
        } else {
            out.size += size;
            out.small_size += size;
            out.small_count += 1;
        }
    }
    bump(ctx, |s| {
        s.files += files;
        s.dirs += dirs;
        s.dataless_items += dataless;
        s.hardlinks_deduped += deduped;
    });
    control.items.fetch_add(dirs, Ordering::Relaxed);

    let children: Vec<DirOut> = subdirs
        .into_par_iter()
        .map(|(p, n, md)| walk_dir(p, n, &md, ctx))
        .collect();
    for d in &children {
        out.size += d.size;
        out.items += d.items;
        out.clone_shared += d.clone_shared;
        out.last_used_day = out.last_used_day.max(d.last_used_day);
    }
    out.dirs.extend(children);
    out
}

fn bump(ctx: &Ctx, f: impl FnOnce(&mut ScanStats)) {
    if let Ok(mut s) = ctx.stats.lock() {
        f(&mut s);
    }
}

/// Turns the nested walk result into the flat arena, children sorted largest first.
fn flatten(root: DirOut, root_path: String, stats: ScanStats) -> Tree {
    let mut tree = Tree {
        root_path,
        stats,
        ..Default::default()
    };
    push_dir(&mut tree, root, NONE);
    tree
}

fn push_dir(tree: &mut Tree, mut d: DirOut, parent: u32) -> u32 {
    let id = tree.nodes.len() as u32;
    tree.names.push(d.name);
    tree.nodes.push(Node {
        parent,
        first_child: NONE,
        next_sibling: NONE,
        name: (tree.names.len() - 1) as u32,
        size: d.size,
        small_size: d.small_size,
        small_count: d.small_count,
        items: d.items,
        last_used_day: d.last_used_day,
        clone_shared: d.clone_shared,
        kind: Kind::Dir,
        flags: d.flags,
    });
    // Children in descending size; linked in reverse so the list reads largest first.
    enum Child {
        Dir(DirOut),
        File(FileOut),
    }
    let mut kids: Vec<(u64, Child)> = d.dirs.drain(..).map(|c| (c.size, Child::Dir(c))).collect();
    kids.extend(d.files.drain(..).map(|f| (f.size, Child::File(f))));
    kids.sort_by_key(|k| k.0);
    for (_, child) in kids {
        let cid = match child {
            Child::Dir(c) => push_dir(tree, c, id),
            Child::File(f) => {
                let fid = tree.nodes.len() as u32;
                tree.names.push(f.name);
                tree.nodes.push(Node {
                    parent: id,
                    first_child: NONE,
                    next_sibling: NONE,
                    name: (tree.names.len() - 1) as u32,
                    size: f.size,
                    small_size: 0,
                    small_count: 0,
                    items: 1,
                    last_used_day: f.last_used_day,
                    clone_shared: f.clone_shared,
                    kind: Kind::File,
                    flags: f.flags,
                });
                fid
            }
        };
        tree.nodes[cid as usize].next_sibling = tree.nodes[id as usize].first_child;
        tree.nodes[id as usize].first_child = cid;
    }
    id
}

fn last_used_day(md: &fs::Metadata) -> u32 {
    let t = |r: io::Result<SystemTime>| {
        r.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| (d.as_secs() / 86_400) as u32)
    };
    t(md.accessed()).max(t(md.modified()))
}

#[cfg(unix)]
fn allocated(md: &fs::Metadata) -> u64 {
    md.blocks() * 512
}
#[cfg(unix)]
fn dev_of(md: &fs::Metadata) -> u64 {
    md.dev()
}
#[cfg(unix)]
fn ino(md: &fs::Metadata) -> u64 {
    md.ino()
}
#[cfg(unix)]
fn nlink(md: &fs::Metadata) -> u64 {
    md.nlink()
}

// Windows parity (allocation size, file IDs) arrives with the Windows checkpoint.
#[cfg(not(unix))]
fn allocated(md: &fs::Metadata) -> u64 {
    md.len()
}
#[cfg(not(unix))]
fn dev_of(_: &fs::Metadata) -> u64 {
    0
}
#[cfg(not(unix))]
fn ino(_: &fs::Metadata) -> u64 {
    0
}
#[cfg(not(unix))]
fn nlink(_: &fs::Metadata) -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Test folders live under the OS temp dir and are left for the OS to clean
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
    }
}
