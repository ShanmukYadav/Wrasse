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

/// Why a scan is paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    User,
    /// On battery and below [`LOW_BATTERY`] percent.
    Battery,
    LowPowerMode,
}

/// Scans pause on battery below this charge (PLAN.md Phase 1).
pub const LOW_BATTERY: u8 = 20;

/// Shared between the scan and whoever drives it: cancel, pause and live progress.
#[derive(Default)]
pub struct ScanControl {
    cancel: AtomicBool,
    paused: AtomicBool,
    /// 0 running, 1 user, 2 battery, 3 Low Power Mode.
    reason: std::sync::atomic::AtomicU8,
    /// The user resumed a power pause: don't pause for power again during this scan.
    power_override: AtomicBool,
    items: AtomicU64,
    bytes: AtomicU64,
    current: Mutex<String>,
    /// Folders left out of this scan (marked unreadable without being opened). Set once,
    /// before the scan starts.
    skip: std::sync::OnceLock<Vec<PathBuf>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub items: u64,
    pub bytes: u64,
    pub current: String,
    pub paused: Option<PauseReason>,
}

impl ScanControl {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Leaves these folders out of the scan: they're marked unreadable without being opened.
    /// Only the first call counts.
    pub fn skip_folders(&self, folders: Vec<PathBuf>) {
        let _ = self.skip.set(folders);
    }

    fn skips(&self, path: &Path) -> bool {
        self.skip.get().is_some_and(|s| {
            let len = path.as_os_str().len();
            s.iter().any(|f| f.as_os_str().len() == len && f == path)
        })
    }
    /// Pause or resume by hand. Resuming also overrides power pauses for the rest of the scan.
    pub fn set_paused(&self, paused: bool) {
        if !paused {
            self.power_override.store(true, Ordering::Relaxed);
        }
        self.reason
            .store(if paused { 1 } else { 0 }, Ordering::Relaxed);
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Pauses on battery below [`LOW_BATTERY`] or in Low Power Mode, and resumes when that
    /// clears, unless the user paused by hand or chose to carry on.
    pub fn apply_power(&self, power: crate::platform::PowerState) {
        if self.power_override.load(Ordering::Relaxed) || self.reason.load(Ordering::Relaxed) == 1 {
            return;
        }
        let reason = if power.low_power {
            3
        } else if power.on_battery && power.percent < LOW_BATTERY {
            2
        } else {
            0
        };
        self.reason.store(reason, Ordering::Relaxed);
        self.paused.store(reason != 0, Ordering::Relaxed);
    }

    pub(crate) fn pause_reason(&self) -> Option<PauseReason> {
        if !self.paused.load(Ordering::Relaxed) {
            return None;
        }
        match self.reason.load(Ordering::Relaxed) {
            2 => Some(PauseReason::Battery),
            3 => Some(PauseReason::LowPowerMode),
            _ => Some(PauseReason::User),
        }
    }
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn progress(&self) -> Progress {
        Progress {
            items: self.items.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
            paused: self.pause_reason(),
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
    /// Modification time, seconds since the Unix epoch.
    mtime: u64,
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
    let pool = pool()?;
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

/// A folder that changed since the last scan (from FSEvents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    /// The OS lost track of details below this folder: re-walk all of it, not just its entries.
    pub recursive: bool,
}

/// Updates a previous scan in place of a full walk: only the changed folders are re-read.
/// Paths outside the tree's root are ignored. Hard links seen in unchanged parts aren't known,
/// so a hard link added inside a changed folder may be counted again until the next full scan.
pub fn rescan(previous: &Tree, changes: &[Change], control: &ScanControl) -> io::Result<Tree> {
    let root_path = PathBuf::from(&previous.root_path);
    let md = fs::symlink_metadata(&root_path)?;
    let pool = pool()?;
    let ctx = Ctx {
        control,
        links: LinkSet::new(),
        dev: dev_of(&md),
        stats: Mutex::new(ScanStats::default()),
    };
    let mut root = unflatten(previous, previous.root());
    // Shallow paths first, so a re-walked folder isn't then refreshed again for a change inside it.
    let mut ordered: Vec<&Change> = changes.iter().collect();
    ordered.sort_by_key(|c| c.path.components().count());
    let mut rewalked: Vec<PathBuf> = Vec::new();
    pool.install(|| {
        for change in ordered {
            if rewalked.iter().any(|r| change.path.starts_with(r)) {
                continue;
            }
            let Ok(rel) = change.path.strip_prefix(&root_path) else {
                continue;
            };
            // The deepest folder of the old tree on the way to the changed path.
            let mut node = &mut root;
            let mut path = root_path.clone();
            let mut exact = true;
            for part in rel.components() {
                let name = part.as_os_str().to_string_lossy();
                match node.dirs.iter().position(|d| *d.name == *name) {
                    Some(i) => {
                        path.push(part);
                        node = &mut node.dirs[i];
                    }
                    None => {
                        exact = false;
                        break;
                    }
                }
            }
            let Ok(own) = fs::symlink_metadata(&path) else {
                continue; // gone; its parent's change event covers the removal
            };
            if !own.is_dir() || dev_of(&own) != ctx.dev {
                continue;
            }
            let old = std::mem::take(node);
            *node = if exact && change.recursive {
                rewalked.push(path.clone());
                walk_dir(path, old.name, &own, &ctx)
            } else {
                refresh_dir(old, path, &own, &ctx)
            };
        }
    });
    total_all(&mut root);
    let mut stats = count(&root);
    stats.hardlinks_deduped = previous.stats.hardlinks_deduped;
    stats.cancelled = control.is_cancelled();
    stats.clone_shared_bytes = root.clone_shared;
    Ok(flatten(root, previous.root_path.clone(), stats))
}

/// The arena back into nested folders, so parts of it can be swapped out.
fn unflatten(tree: &Tree, id: u32) -> DirOut {
    let n = tree.node(id);
    let mut out = DirOut {
        name: tree.name(id).into(),
        size: n.size,
        small_size: n.small_size,
        small_count: n.small_count,
        items: n.items,
        last_used_day: n.last_used_day,
        clone_shared: n.clone_shared,
        flags: n.flags,
        files: Vec::new(),
        dirs: Vec::new(),
    };
    for c in tree.children(id) {
        let cn = tree.node(c);
        match cn.kind {
            Kind::Dir => out.dirs.push(unflatten(tree, c)),
            Kind::File => out.files.push(FileOut {
                name: tree.name(c).into(),
                size: cn.size,
                last_used_day: cn.last_used_day,
                mtime: cn.mtime,
                clone_shared: cn.clone_shared,
                flags: cn.flags,
            }),
        }
    }
    out
}

/// Scan statistics recounted from a tree (after an incremental update).
fn count(d: &DirOut) -> ScanStats {
    let mut s = ScanStats {
        files: d.files.len() as u64 + u64::from(d.small_count),
        dirs: d.dirs.len() as u64,
        unreadable_dirs: u64::from(d.flags & flag::UNREADABLE != 0),
        dataless_items: d
            .files
            .iter()
            .filter(|f| f.flags & flag::DATALESS != 0)
            .count() as u64
            + u64::from(d.flags & flag::DATALESS != 0),
        ..Default::default()
    };
    for c in &d.dirs {
        let cs = count(c);
        s.files += cs.files;
        s.dirs += cs.dirs;
        s.unreadable_dirs += cs.unreadable_dirs;
        s.dataless_items += cs.dataless_items;
    }
    s
}

pub(crate) fn pool() -> io::Result<rayon::ThreadPool> {
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .saturating_sub(1)
        .max(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("wrasse-scan-{i}"))
        .start_handler(|_| platform::lower_thread_priority())
        .build()
        .map_err(io::Error::other)
}

fn walk_dir(path: PathBuf, name: Box<str>, own: &fs::Metadata, ctx: &Ctx) -> DirOut {
    let Some((mut out, subdirs)) = read_level(&path, name, own, ctx) else {
        return DirOut::default();
    };
    let children: Vec<DirOut> = subdirs
        .into_par_iter()
        .map(|(p, n, md)| walk_dir(p, n, &md, ctx))
        .collect();
    out.dirs.extend(children);
    total(&mut out);
    out
}

/// Re-reads one folder's own entries and keeps the subtrees of subfolders that are still there
/// (FSEvents reports changes per folder). New subfolders are walked in full.
fn refresh_dir(old: DirOut, path: PathBuf, own: &fs::Metadata, ctx: &Ctx) -> DirOut {
    let Some((mut out, subdirs)) = read_level(&path, old.name.clone(), own, ctx) else {
        return old;
    };
    let mut previous: std::collections::HashMap<Box<str>, DirOut> =
        old.dirs.into_iter().map(|d| (d.name.clone(), d)).collect();
    let (kept, fresh): (Vec<_>, Vec<_>) = subdirs.into_iter().partition(|(_, n, _)| {
        previous
            .get(n)
            .is_some_and(|d| d.flags & (flag::UNREADABLE | flag::MOUNT | flag::DATALESS) == 0)
    });
    let walked: Vec<DirOut> = fresh
        .into_par_iter()
        .map(|(p, n, md)| walk_dir(p, n, &md, ctx))
        .collect();
    for (_, n, _) in kept {
        if let Some(d) = previous.remove(&n) {
            out.dirs.push(d);
        }
    }
    out.dirs.extend(walked);
    total(&mut out);
    out
}

/// One folder's direct entries: large files listed, small files aggregated, special folders
/// (other volumes, iCloud dataless) recorded, and the subfolders still to visit. `None` when the
/// scan was cancelled.
#[allow(clippy::type_complexity)]
fn read_level(
    path: &Path,
    name: Box<str>,
    own: &fs::Metadata,
    ctx: &Ctx,
) -> Option<(DirOut, Vec<(PathBuf, Box<str>, fs::Metadata)>)> {
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
        return None;
    }
    if let Ok(mut current) = control.current.try_lock() {
        current.clear();
        current.push_str(&path.to_string_lossy());
    }

    let read = if control.skips(path) {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    } else {
        fs::read_dir(path)
    };
    let entries = match read {
        Ok(e) => e,
        Err(_) => {
            out.flags |= flag::UNREADABLE;
            bump(ctx, |s| s.unreadable_dirs += 1);
            return Some((out, Vec::new()));
        }
    };

    let mut subdirs: Vec<(PathBuf, Box<str>, fs::Metadata)> = Vec::new();
    let (mut files, mut dirs, mut dataless, mut deduped) = (0u64, 0u64, 0u64, 0u64);
    for entry in entries.flatten() {
        let Ok(md) = entry.metadata() else { continue }; // lstat: symlinks are not followed
        let name: Box<str> = entry.file_name().to_string_lossy().into();
        let used = last_used_day(&md);
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
        if size >= SMALL_FILE {
            let mut clone_shared = 0;
            if md.is_file() {
                if let Some(shared) = platform::clone_shared_bytes(&entry.path(), size) {
                    flags |= flag::CLONE;
                    clone_shared = shared;
                }
            }
            out.files.push(FileOut {
                name,
                size,
                last_used_day: used,
                mtime: mtime_secs(&md),
                clone_shared,
                flags,
            });
        } else {
            out.small_size += size;
            out.small_count += 1;
            out.last_used_day = out.last_used_day.max(used);
        }
    }
    bump(ctx, |s| {
        s.files += files;
        s.dirs += dirs;
        s.dataless_items += dataless;
        s.hardlinks_deduped += deduped;
    });
    control.items.fetch_add(dirs, Ordering::Relaxed);
    Some((out, subdirs))
}

/// A folder's totals from its direct entries and its (already totalled) subfolders.
fn total(d: &mut DirOut) {
    let direct = d.files.len() as u64 + u64::from(d.small_count) + d.dirs.len() as u64;
    d.size = d.small_size
        + d.files.iter().map(|f| f.size).sum::<u64>()
        + d.dirs.iter().map(|c| c.size).sum::<u64>();
    d.items = direct + d.dirs.iter().map(|c| c.items).sum::<u64>();
    d.clone_shared = d.files.iter().map(|f| f.clone_shared).sum::<u64>()
        + d.dirs.iter().map(|c| c.clone_shared).sum::<u64>();
    let newest = d
        .files
        .iter()
        .map(|f| f.last_used_day)
        .chain(d.dirs.iter().map(|c| c.last_used_day))
        .max()
        .unwrap_or(0);
    d.last_used_day = d.last_used_day.max(newest);
}

/// Totals for a whole subtree, bottom-up (after folders inside it were refreshed).
fn total_all(d: &mut DirOut) {
    for c in &mut d.dirs {
        total_all(c);
    }
    total(d);
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
        mtime: 0,
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
                    mtime: f.mtime,
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

fn mtime_secs(md: &fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
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
pub(crate) fn allocated(md: &fs::Metadata) -> u64 {
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
pub(crate) fn allocated(md: &fs::Metadata) -> u64 {
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
mod tests;
