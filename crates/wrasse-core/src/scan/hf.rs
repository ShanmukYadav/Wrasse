//! Hugging Face hub caches (`…/hub/models--org--name/…`). A model's files are symlinks from
//! `snapshots/` into shared blob storage, so a model's real size is the size of the files its
//! snapshot links point to. Read-only: links are read and followed, nothing is opened for writing.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfModel {
    /// "org/name", e.g. "mlx-community/Mistral-7B-Instruct-v0.3-4bit".
    pub repo: String,
    /// The `models--org--name` folder.
    pub dir: PathBuf,
    /// Bytes on disk of the files its snapshots point to (shared blobs counted once, first model wins).
    pub bytes: u64,
}

/// The models in a hub folder, largest first.
pub fn hf_models(hub: &Path) -> Vec<HfModel> {
    let Ok(entries) = fs::read_dir(hub) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("models--"))
        })
        .collect();
    dirs.sort();
    for dir in dirs {
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let repo = name.trim_start_matches("models--").replacen("--", "/", 1);
        let mut bytes = 0;
        sum_snapshot(&dir.join("snapshots"), &mut seen, &mut bytes, 0);
        out.push(HfModel { repo, dir, bytes });
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.bytes));
    out
}

fn sum_snapshot(dir: &Path, seen: &mut HashSet<PathBuf>, bytes: &mut u64, depth: u32) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(md) = e.metadata() else { continue }; // lstat
        if md.is_dir() {
            sum_snapshot(&e.path(), seen, bytes, depth + 1);
            continue;
        }
        // Follow the link (possibly a chain) to the real blob.
        let Ok(target) = fs::canonicalize(e.path()) else {
            continue;
        };
        if !seen.insert(target.clone()) {
            continue;
        }
        if let Ok(t) = fs::metadata(&target) {
            *bytes += allocated(&t);
        }
    }
}

#[cfg(unix)]
fn allocated(md: &fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::blocks(md) * 512
}
#[cfg(not(unix))]
fn allocated(md: &fs::Metadata) -> u64 {
    md.len()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sizes_follow_snapshot_links_and_count_shared_blobs_once() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let hub = std::env::temp_dir().join(format!("wrasse-hf-{nanos}/hub"));
        fs::create_dir_all(hub.join("blobs")).unwrap();
        let mut f = fs::File::create(hub.join("blobs/weights")).unwrap();
        f.write_all(&vec![1u8; 2_000_000]).unwrap();
        f.sync_all().unwrap();
        for m in ["models--org--a", "models--org--b"] {
            let snap = hub.join(m).join("snapshots/rev");
            fs::create_dir_all(&snap).unwrap();
            std::os::unix::fs::symlink("../../../blobs/weights", snap.join("model.safetensors"))
                .unwrap();
        }
        let models = hf_models(&hub);
        assert_eq!(models.len(), 2);
        assert!(models[0].bytes >= 2_000_000);
        assert_eq!(models[1].bytes, 0, "a shared blob counts once");
        assert_eq!(models[0].repo, "org/a");
        crate::platform::remove_test_dir(hub.parent().unwrap());
    }
}
