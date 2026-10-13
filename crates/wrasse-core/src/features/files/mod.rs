//! Large, old and duplicate files (PLAN.md §6.9, docs/history/phase3-plan.md §5). Read-only: lists and
//! groups; removal of duplicates goes through `safety` after the kept copy is checked again.

pub mod dupes;

use crate::scan::tree::{flag, Kind, Tree};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Video,
    Archive,
    DiskImage,
    Model,
    Audio,
    Image,
    Document,
    Other,
}

pub(crate) fn kind_of(name: &str) -> FileKind {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mov" | "mp4" | "m4v" | "mkv" | "avi" | "webm" | "mts" | "braw" | "r3d" => FileKind::Video,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "xip" => {
            FileKind::Archive
        }
        "dmg" | "iso" | "img" | "pkg" | "vmdk" | "vdi" | "qcow2" | "sparseimage"
        | "sparsebundle" => FileKind::DiskImage,
        "gguf" | "safetensors" | "bin" | "pt" | "pth" | "ckpt" | "onnx" | "mlmodel" | "npz" => {
            FileKind::Model
        }
        "wav" | "aiff" | "aif" | "flac" | "mp3" | "m4a" | "caf" => FileKind::Audio,
        "jpg" | "jpeg" | "png" | "heic" | "tif" | "tiff" | "psd" | "raw" | "dng" | "cr3"
        | "nef" | "arw" => FileKind::Image,
        "pdf" | "doc" | "docx" | "key" | "pages" | "numbers" | "ppt" | "pptx" | "xls" | "xlsx" => {
            FileKind::Document
        }
        _ => FileKind::Other,
    }
}

/// Folder names that are one thing to the user (an app, a library): never looked inside.
pub(crate) fn is_package(name: &str) -> bool {
    const PACKAGES: [&str; 8] = [
        ".app",
        ".framework",
        ".bundle",
        ".plugin",
        ".kext",
        ".xcarchive",
        ".sparsebundle",
        ".photoslibrary",
    ];
    PACKAGES.iter().any(|s| name.ends_with(s))
        || crate::platform::protected_name_suffixes()
            .iter()
            .any(|s| name.ends_with(s))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigFile {
    pub path: PathBuf,
    pub bytes: u64,
    /// Days since last opened (Spotlight) or, when Spotlight doesn't know, since last read or
    /// changed (`approx`).
    pub last_used_days: u32,
    pub approx: bool,
    pub kind: FileKind,
    /// Modification time from the scan, seconds since the Unix epoch. Sent back with a move so
    /// a file that changed after the scan is skipped.
    pub mtime: u64,
}

/// Folders Large & Old and Duplicates never look into: the Library (apps, caches and Wrasse's own
/// data have their own screens), hidden folders, and stores whose files are only meaningful
/// together (model blobs, package caches, environments): removing one file would break the rest.
fn skipped_dir(name: &str, is_library: bool) -> bool {
    const STORES: [&str; 8] = [
        "node_modules",
        "site-packages",
        "blobs",
        "uv_cache",
        "DerivedData",
        "__pycache__",
        "venv",
        "Pods",
    ];
    is_library || name.starts_with('.') || STORES.contains(&name) || is_package(name)
}

/// Files of at least `min_bytes` under `within` (the home folder) in a saved scan tree, as
/// (path, bytes), biggest first. Skips iCloud-only files, second hard links, other mounts,
/// package contents and the folders in [`skipped_dir`]. `display_prefix` is the scan root's prefix
/// (`/System/Volumes/Data`).
pub fn files_over(
    tree: &Tree,
    display_prefix: &str,
    within: &Path,
    min_bytes: u64,
) -> Vec<(PathBuf, u64)> {
    let prefix = display_prefix.trim_end_matches('/');
    let Some(start) = tree.find(&format!("{prefix}{}", within.display())) else {
        return Vec::new();
    };
    let mut found: Vec<(u32, u64)> = Vec::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        for c in tree.children(id) {
            let n = tree.node(c);
            if n.size < min_bytes || n.flags & (flag::DATALESS | flag::MOUNT) != 0 {
                continue;
            }
            match n.kind {
                Kind::Dir
                    if !skipped_dir(tree.name(c), id == start && tree.name(c) == "Library") =>
                {
                    stack.push(c)
                }
                Kind::File if n.flags & flag::HARDLINK_SEEN == 0 => found.push((c, n.size)),
                _ => {}
            }
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.1));
    found
        .into_iter()
        .map(|(id, bytes)| {
            let arena = tree.path(id);
            (
                PathBuf::from(arena.strip_prefix(prefix).unwrap_or(&arena)),
                bytes,
            )
        })
        .collect()
}

/// [`kind_of`] for a real path. A name that looks like a model stays [`FileKind::Other`] unless
/// the file has a model header or a sibling `config.json` with `model_type` or `architectures`.
pub(crate) fn file_kind(path: &Path) -> FileKind {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kind = kind_of(&name);
    if kind == FileKind::Model && !crate::scan::weights::is_model_weights(path) {
        FileKind::Other
    } else {
        kind
    }
}

/// The biggest files (at most `limit`) of at least `min_bytes` under `within`, with when each was
/// last used: Spotlight's date (`last_used`, Unix seconds) or, failing that, the newest of last
/// read and last changed from the scan (`approx`).
pub fn large_files(
    tree: &Tree,
    display_prefix: &str,
    within: &Path,
    min_bytes: u64,
    limit: usize,
    today: u32,
    last_used: &dyn Fn(&Path) -> Option<i64>,
) -> Vec<BigFile> {
    let prefix = display_prefix.trim_end_matches('/');
    let mut files = files_over(tree, display_prefix, within, min_bytes);
    files.truncate(limit);
    files
        .into_iter()
        .map(|(path, bytes)| {
            let spot = last_used(&path).map(|s| today.saturating_sub((s / 86_400) as u32));
            let node = tree
                .find(&format!("{prefix}{}", path.display()))
                .map(|id| tree.node(id));
            let scanned = node.map_or(today, |n| n.last_used_day);
            BigFile {
                kind: file_kind(&path),
                last_used_days: spot.unwrap_or_else(|| today.saturating_sub(scanned)),
                approx: spot.is_none(),
                mtime: node.map_or(0, |n| n.mtime),
                bytes,
                path,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn large_files_skip_packages_icloud_and_small_ones() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-large-{nanos}"));
        let home = root.join("Users/me");
        let put = |p: &Path, n: usize| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, vec![5u8; n]).unwrap();
        };
        put(&home.join("Movies/trip.mov"), 30_000_000);
        put(&home.join("Downloads/ubuntu.iso"), 20_000_000);
        put(&home.join("Downloads/notes.txt"), 2_000_000);
        put(
            &home.join("Applications/Big.app/Contents/Resources/huge.bin"),
            40_000_000,
        );
        put(&root.join("Users/other/movie.mov"), 30_000_000);
        put(
            &home.join("Library/Application Support/x/big.bin"),
            30_000_000,
        );
        put(&home.join(".cache/huggingface/hub/blobs/ab/cd"), 30_000_000);
        put(
            &home.join("Projects/app/node_modules/big/dist.bin"),
            30_000_000,
        );
        put(&home.join("Projects/env/hub/blobs/5c/5ccf"), 30_000_000);
        put(
            &home.join("Projects/env/venv/lib/libtorch.dylib"),
            30_000_000,
        );
        let tree = crate::scan::scan(&root, &crate::scan::ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let found = large_files(
            &tree,
            &prefix,
            Path::new("/Users/me"),
            10_000_000,
            100,
            20_000,
            &|p| {
                p.ends_with("trip.mov")
                    .then_some(20_000 * 86_400 - 400 * 86_400)
            },
        );
        let names: Vec<(String, FileKind, bool)> = found
            .iter()
            .map(|f| (f.path.display().to_string(), f.kind, f.approx))
            .collect();
        assert_eq!(
            names,
            [
                (
                    "/Users/me/Movies/trip.mov".to_string(),
                    FileKind::Video,
                    false
                ),
                (
                    "/Users/me/Downloads/ubuntu.iso".to_string(),
                    FileKind::DiskImage,
                    true
                ),
            ],
            "not inside an app, other users', the Library, hidden folders, model blobs or environments"
        );
        assert_eq!(found[0].last_used_days, 400, "from Spotlight");
        assert!(found[1].approx, "no Spotlight date: the age is approximate");
        assert!(
            found[1].mtime > 0,
            "the scan's modification time travels with the file"
        );
        crate::platform::remove_test_dir(&root);
    }

    #[test]
    fn zero_bin_is_not_shown_as_a_model() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-kind-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("old-big.bin");
        fs::write(&bin, [0u8; 64]).unwrap();
        assert_eq!(file_kind(&bin), FileKind::Other);
        let gguf = dir.join("mistral.gguf");
        fs::write(&gguf, b"GGUF\x03\x00\x00\x00").unwrap();
        assert_eq!(file_kind(&gguf), FileKind::Model);
        let safe = dir.join("adapter.safetensors");
        let json = br#"{"format":"pt"}"#;
        let mut bytes = (json.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(json);
        fs::write(&safe, bytes).unwrap();
        assert_eq!(file_kind(&safe), FileKind::Model);
        crate::platform::remove_test_dir(&dir);
    }
}
