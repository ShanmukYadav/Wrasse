//! The last scan, saved outside the project (PLAN.md §5.4): `~/Library/Application Support/
//! com.shanmukyadav.wrasse/` on macOS. Used to show the map instantly at launch and for
//! "what grew". Writing it is the only file Wrasse creates in this phase.

use super::breakdown::UiDisk;
use super::tree::{Kind, Node, ScanStats, Tree};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Caches above this size are not written (PLAN.md §5.4 budget).
const MAX_BYTES: usize = 200_000_000;
const FILE: &str = "scan-cache.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheFile {
    version: u32,
    /// Seconds since the Unix epoch.
    scanned_at: u64,
    disk: UiDisk,
}

/// The app's data folder for this OS.
pub fn data_dir(identifier: &str) -> Option<PathBuf> {
    // For measurements (scripts/check-memory.sh --full-scan): a throwaway data folder, so the
    // app starts with no saved scan and no tray, and the user's real data folder is untouched.
    if let Some(dir) = std::env::var_os("WRASSE_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = crate::platform::home_dir()?;
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support").join(identifier))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(identifier))
    } else {
        Some(home.join(".local/share").join(identifier))
    }
}

pub fn save(dir: &Path, disk: &UiDisk, scanned_at: u64) -> io::Result<()> {
    let json = serde_json::to_vec(&CacheFile {
        version: VERSION,
        scanned_at,
        disk: disk.clone(),
    })
    .map_err(io::Error::other)?;
    if json.len() > MAX_BYTES {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    fs::write(dir.join(FILE), json)
}

/// The cached disk and when it was scanned, if a readable cache of this version exists.
pub fn load(dir: &Path) -> Option<(UiDisk, u64)> {
    let bytes = fs::read(dir.join(FILE)).ok()?;
    let file: CacheFile = serde_json::from_slice(&bytes).ok()?;
    (file.version == VERSION).then_some((file.disk, file.scanned_at))
}

// ---- The full tree, for incremental rescans --------------------------------------------------
//
// A compact binary file (`scan-tree.bin`): the arena as fixed-size records plus the names, and the
// FSEvents event ID taken just before the walk started, so the next scan can ask "what changed
// since then?" and re-read only those folders.

const TREE_FILE: &str = "scan-tree.bin";
/// `WRSTREE3` adds each file's modification time (seconds). Older caches are ignored until the
/// next scan, so Large & Old can compare a file to the scan that listed it.
const TREE_MAGIC: &[u8; 8] = b"WRSTREE3";

pub struct SavedTree {
    pub tree: Tree,
    /// FSEvents event ID from just before this tree was walked (0 where there's no event log).
    pub event_id: u64,
    /// Seconds since the Unix epoch.
    pub scanned_at: u64,
    /// Identity of the volume's change log when `event_id` was taken (FSEvents UUID).
    pub log_id: Option<String>,
}

fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes());
}

pub fn save_tree(
    dir: &Path,
    tree: &Tree,
    event_id: u64,
    log_id: Option<&str>,
    scanned_at: u64,
) -> io::Result<()> {
    let mut b = Vec::with_capacity(tree.nodes.len() * 56 + 1024);
    b.extend_from_slice(TREE_MAGIC);
    put_u64(&mut b, event_id);
    put_u64(&mut b, scanned_at);
    let log = log_id.unwrap_or("");
    put_u32(&mut b, log.len() as u32);
    b.extend_from_slice(log.as_bytes());
    put_u32(&mut b, tree.root_path.len() as u32);
    b.extend_from_slice(tree.root_path.as_bytes());
    let s = tree.stats;
    for v in [
        s.files,
        s.dirs,
        s.unreadable_dirs,
        s.dataless_items,
        s.hardlinks_deduped,
        s.clone_shared_bytes,
    ] {
        put_u64(&mut b, v);
    }
    put_u32(&mut b, tree.nodes.len() as u32);
    for n in &tree.nodes {
        for v in [
            n.parent,
            n.first_child,
            n.next_sibling,
            n.name,
            n.small_count,
            n.last_used_day,
        ] {
            put_u32(&mut b, v);
        }
        for v in [n.size, n.small_size, n.items, n.clone_shared, n.mtime] {
            put_u64(&mut b, v);
        }
        b.push(n.kind as u8);
        b.push(n.flags);
    }
    put_u32(&mut b, tree.names.len() as u32);
    for name in &tree.names {
        put_u32(&mut b, name.len() as u32);
        b.extend_from_slice(name.as_bytes());
    }
    if b.len() > MAX_BYTES {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    fs::write(dir.join(TREE_FILE), b)
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
}

pub fn load_tree(dir: &Path) -> Option<SavedTree> {
    let bytes = fs::read(dir.join(TREE_FILE)).ok()?;
    let mut r = Reader { b: &bytes, at: 0 };
    if r.take(8)? != TREE_MAGIC {
        return None;
    }
    let event_id = r.u64()?;
    let scanned_at = r.u64()?;
    let log_len = r.u32()? as usize;
    let log = String::from_utf8(r.take(log_len)?.to_vec()).ok()?;
    let log_id = (!log.is_empty()).then_some(log);
    let len = r.u32()? as usize;
    let root_path = String::from_utf8(r.take(len)?.to_vec()).ok()?;
    let stats = ScanStats {
        files: r.u64()?,
        dirs: r.u64()?,
        unreadable_dirs: r.u64()?,
        dataless_items: r.u64()?,
        hardlinks_deduped: r.u64()?,
        clone_shared_bytes: r.u64()?,
        cancelled: false,
    };
    let count = r.u32()? as usize;
    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let (parent, first_child, next_sibling, name, small_count, last_used_day) =
            (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let (size, small_size, items, clone_shared, mtime) =
            (r.u64()?, r.u64()?, r.u64()?, r.u64()?, r.u64()?);
        let kind = if r.u8()? == Kind::File as u8 {
            Kind::File
        } else {
            Kind::Dir
        };
        let flags = r.u8()?;
        nodes.push(Node {
            parent,
            first_child,
            next_sibling,
            name,
            size,
            small_size,
            small_count,
            items,
            last_used_day,
            mtime,
            clone_shared,
            kind,
            flags,
        });
    }
    let name_count = r.u32()? as usize;
    let mut names = Vec::with_capacity(name_count);
    for _ in 0..name_count {
        let len = r.u32()? as usize;
        names.push(
            String::from_utf8_lossy(r.take(len)?)
                .into_owned()
                .into_boxed_str(),
        );
    }
    if nodes.is_empty() || nodes.iter().any(|n| n.name as usize >= names.len()) {
        return None;
    }
    Some(SavedTree {
        tree: Tree {
            nodes,
            names,
            root_path,
            stats,
        },
        event_id,
        scanned_at,
        log_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{scan, ScanControl};

    #[test]
    fn tree_round_trips() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-cache-{nanos}"));
        fs::create_dir_all(dir.join("data/sub")).unwrap();
        fs::write(dir.join("data/sub/a.bin"), vec![1u8; 2_000_000]).unwrap();
        let tree = scan(&dir.join("data"), &ScanControl::default()).unwrap();
        save_tree(&dir, &tree, 42, Some("uuid"), 7).unwrap();
        let back = load_tree(&dir).unwrap();
        assert_eq!(back.event_id, 42);
        assert_eq!(back.log_id.as_deref(), Some("uuid"));
        assert_eq!(back.tree.nodes.len(), tree.nodes.len());
        assert_eq!(back.tree.node(0).size, tree.node(0).size);
        let file = back
            .tree
            .find(&format!("{}/data/sub/a.bin", dir.display()))
            .unwrap();
        assert_eq!(back.tree.node(file).mtime, tree.node(file).mtime);
        assert!(tree.node(file).mtime > 0);
        assert_eq!(back.tree.root_path, tree.root_path);
        assert!(back
            .tree
            .find(&format!("{}/data/sub/a.bin", dir.display()))
            .is_some());
        crate::platform::remove_test_dir(&dir);
    }
}
