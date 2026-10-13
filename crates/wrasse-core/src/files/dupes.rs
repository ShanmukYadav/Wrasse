//! Duplicate files: size → partial hash → full hash (PLAN.md §6.9, docs/history/phase3-plan.md §5).
//! Hard links (same file) and iCloud-only files are never duplicates; APFS clones are shown but
//! free nothing. The user picks which copy to keep; before anything is removed the kept copy and
//! every other copy are hashed again, and the group is left alone if anything changed.

use crate::safety::{self, Context, Mode, Request, Safety};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Files of one size, with the hash of their sampled bytes.
type Sampled = Vec<(PathBuf, fs::Metadata, String)>;

/// Files smaller than this aren't checked (docs/history/phase3-plan.md decision 6).
pub const MIN_BYTES: u64 = 10_000_000;
/// Bytes read at the start, middle and end for the partial hash.
const SAMPLE: u64 = 64 * 1024;

/// Cancel and progress for a duplicate search.
#[derive(Debug, Default)]
pub struct Control {
    pub cancel: AtomicBool,
    /// Bytes hashed so far, and the total that may need hashing.
    pub hashed: AtomicU64,
    pub to_hash: AtomicU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub path: PathBuf,
    /// Days since it was last changed.
    pub modified_days: u32,
    /// In Documents, Desktop or iCloud Drive: never suggested for removal, picked by hand only.
    pub hand_pick_only: bool,
    /// Shares its blocks with another file (APFS clone): removing it frees (almost) nothing.
    pub clone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Full BLAKE3 hash, hex.
    pub hash: String,
    /// Size of each copy.
    pub bytes: u64,
    pub copies: Vec<Copy>,
    /// What keeping one copy frees: the other copies, counting clones of each other once.
    pub reclaimable: u64,
}

fn hex(h: &blake3::Hash) -> String {
    h.to_hex().to_string()
}

/// Hash of the size and the first, middle and last [`SAMPLE`] bytes.
pub fn partial_hash(path: &Path, size: u64) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    h.update(&size.to_le_bytes());
    let mut buf = vec![0u8; SAMPLE as usize];
    for at in [0, size / 2, size.saturating_sub(SAMPLE)] {
        f.seek(SeekFrom::Start(at))?;
        let n = f.read(&mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Hash of the whole file, read in 1 MB chunks; stops early when cancelled.
pub fn full_hash(path: &Path, control: Option<&Control>) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if control.is_some_and(|c| c.cancel.load(Ordering::Relaxed)) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        if let Some(c) = control {
            c.hashed.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    Ok(hex(&h.finalize()))
}

/// Groups byte-identical files among `candidates` (path, size). `hand_pick_only` marks copies the
/// user must pick by hand. Runs on the scan's low-priority pool.
pub fn find(
    candidates: Vec<(PathBuf, u64)>,
    hand_pick_only: &(dyn Fn(&Path) -> bool + Sync),
    today: u32,
    control: &Control,
) -> Vec<Group> {
    // 1. Same size (and not the same file twice through a hard link; not iCloud-only).
    let mut by_size: HashMap<u64, Vec<(PathBuf, fs::Metadata)>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for (path, size) in candidates {
        if size < MIN_BYTES {
            continue;
        }
        let Ok(md) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !md.is_file()
            || crate::platform::is_dataless(&md)
            || !seen.insert(crate::platform::file_identity(&md))
        {
            continue;
        }
        by_size.entry(md.len()).or_default().push((path, md));
    }
    let same_size: Vec<(u64, Vec<(PathBuf, fs::Metadata)>)> =
        by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    // 2. Partial hash, 3. full hash (only where partial hashes still match).
    let run = || {
        let partial: Vec<(u64, Sampled)> = same_size
            .into_par_iter()
            .map(|(size, files)| {
                let hashed = files
                    .into_iter()
                    .filter_map(|(p, md)| partial_hash(&p, size).ok().map(|h| (p, md, h)))
                    .collect();
                (size, hashed)
            })
            .collect();
        let mut work: Vec<(u64, Vec<(PathBuf, fs::Metadata)>)> = Vec::new();
        for (size, files) in partial {
            let mut groups: HashMap<String, Vec<(PathBuf, fs::Metadata)>> = HashMap::new();
            for (p, md, h) in files {
                groups.entry(h).or_default().push((p, md));
            }
            work.extend(
                groups
                    .into_values()
                    .filter(|g| g.len() > 1)
                    .map(|g| (size, g)),
            );
        }
        control.to_hash.store(
            work.iter().map(|(s, g)| s * g.len() as u64).sum(),
            Ordering::Relaxed,
        );
        let mut out: Vec<Group> = work
            .into_par_iter()
            .flat_map_iter(|(size, files)| {
                let mut groups: HashMap<String, Vec<(PathBuf, fs::Metadata)>> = HashMap::new();
                for (p, md) in files {
                    if let Ok(h) = full_hash(&p, Some(control)) {
                        groups.entry(h).or_default().push((p, md));
                    }
                }
                groups
                    .into_iter()
                    .filter(|(_, g)| g.len() > 1)
                    .map(|(hash, g)| group(hash, size, g, hand_pick_only, today))
                    .collect::<Vec<_>>()
            })
            .collect();
        out.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable).then(a.hash.cmp(&b.hash)));
        out
    };
    match crate::scan::walk::pool() {
        Ok(pool) => pool.install(run),
        Err(_) => run(),
    }
}

fn group(
    hash: String,
    bytes: u64,
    files: Vec<(PathBuf, fs::Metadata)>,
    hand_pick_only: &(dyn Fn(&Path) -> bool + Sync),
    today: u32,
) -> Group {
    let mut copies: Vec<Copy> = files
        .iter()
        .map(|(p, md)| {
            let day = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(today, |d| (d.as_secs() / 86_400) as u32);
            Copy {
                clone: crate::platform::clone_shared_bytes(p, crate::scan::walk::allocated(md))
                    .is_some_and(|s| s > 0),
                path: p.clone(),
                modified_days: today.saturating_sub(day),
                hand_pick_only: hand_pick_only(p),
            }
        })
        .collect();
    copies.sort_by(|a, b| a.path.cmp(&b.path));
    // Clones share their blocks, so together they are one set of blocks; keeping one set frees
    // the others.
    let clones = copies.iter().filter(|c| c.clone).count();
    let sets = copies.len() - clones + usize::from(clones > 0);
    let freeing = sets.max(1) - 1;
    Group {
        reclaimable: bytes * freeing as u64,
        hash,
        bytes,
        copies,
    }
}

/// Removes the copies the user chose to remove, keeping `keep`. Everything is hashed again first:
/// if the kept copy is gone or changed, or any copy no longer matches, nothing is removed. Goes to
/// the tray (picked by hand, so Documents and Desktop copies may go too).
pub fn remove(
    ctx: &Context,
    keep: &Path,
    remove: &[PathBuf],
    dry_run: bool,
) -> io::Result<safety::Report> {
    let refused = |why: &str| io::Error::new(io::ErrorKind::PermissionDenied, why.to_string());
    let keep_md =
        fs::symlink_metadata(keep).map_err(|_| refused("the copy you keep isn't there"))?;
    if !keep_md.is_file() || remove.is_empty() || remove.iter().any(|p| p == keep) {
        return Err(refused("nothing to remove"));
    }
    let want = full_hash(keep, None)?;
    for p in remove {
        let md = fs::symlink_metadata(p).map_err(|_| refused("a copy isn't there any more"))?;
        if !md.is_file()
            || md.len() != keep_md.len()
            || crate::platform::file_identity(&md) == crate::platform::file_identity(&keep_md)
            || full_hash(p, None)? != want
        {
            return Err(refused(
                "the copies aren't identical any more; nothing was removed",
            ));
        }
    }
    let requests: Vec<Request> = remove
        .iter()
        .map(|p| Request {
            path: p.clone(),
            safety: Safety::YourCall,
            rule_id: None,
            root: None,
            hand_picked: true,
            scanned: None,
        })
        .collect();
    let plan = safety::prepare(ctx, &requests, Mode::Quarantine);
    safety::remove(ctx, &plan, dry_run)
}
