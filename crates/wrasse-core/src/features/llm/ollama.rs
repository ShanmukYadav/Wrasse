//! An Ollama models directory: `manifests/` plus `blobs/`. Read-only except through
//! [`exclusive_paths`], which only lists what a later quarantine may move.

use super::{blob_content_hash, file_day, quant_of};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One model, or an orphaned blob no manifest names.
#[derive(Debug, Clone)]
pub struct Listed {
    pub name: String,
    pub manifest: Option<PathBuf>,
    pub quantization: Option<String>,
    /// Sum of the blobs this manifest names, including ones shared with other models.
    pub logical: u64,
    /// Bytes this model adds on disk. Shared blobs belong to the largest model.
    pub exclusive: u64,
    pub shares_weights: bool,
    pub shares_with: Option<String>,
    pub weight: Option<PathBuf>,
    pub content_hash: Option<String>,
    pub orphan: bool,
    pub last_day: u32,
    digests: Vec<String>,
}

/// A parsed models directory.
#[derive(Debug, Clone)]
pub struct Store {
    pub dir: PathBuf,
    pub models: Vec<Listed>,
}

impl Store {
    /// `root` may be `~/.ollama` or the models directory itself (`blobs/` and `manifests/`
    /// as children). `None` when neither layout is there, so the caller lists the folder as-is.
    pub fn open(root: &Path) -> Option<Self> {
        let dir = models_dir(root)?;
        Some(Self {
            models: read_store(&dir),
            dir,
        })
    }
}

/// Manifest plus the blobs only this model uses. Re-read at removal time. Shared blobs stay.
/// Every path is canonical and stays inside the models directory.
pub fn exclusive_paths(models_dir: &Path, name: &str) -> Vec<PathBuf> {
    let Some(store) = Store::open(models_dir) else {
        return Vec::new();
    };
    let Some(model) = store.models.iter().find(|m| m.name == name) else {
        return Vec::new();
    };
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for m in store.models.iter().filter(|m| !m.orphan) {
        for d in &m.digests {
            *counts.entry(d).or_insert(0) += 1;
        }
    }
    let mut out = Vec::new();
    if let Some(manifest) = &model.manifest {
        if let Some(p) = inside(&store.dir, manifest) {
            out.push(p);
        }
    }
    if model.orphan {
        if let Some(weight) = &model.weight {
            if let Some(p) = inside(&store.dir, weight) {
                out.push(p);
            }
        }
        return out;
    }
    for digest in &model.digests {
        if counts.get(digest.as_str()).copied().unwrap_or(0) != 1 {
            continue;
        }
        let blob = store.dir.join("blobs").join(format!("sha256-{digest}"));
        if let Some(p) = inside(&store.dir, &blob) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Model names `ollama rm` accepts: a short token, no traversal and no quotes.
pub fn valid_name(name: &str) -> bool {
    crate::platform::valid_ollama_name(name)
}

fn models_dir(root: &Path) -> Option<PathBuf> {
    let layout = |p: &Path| p.join("blobs").is_dir() || p.join("manifests").is_dir();
    if layout(root) {
        Some(root.to_path_buf())
    } else if layout(&root.join("models")) {
        Some(root.join("models"))
    } else {
        None
    }
}

fn inside(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = fs::canonicalize(root).ok()?;
    let canon = fs::canonicalize(path).ok()?;
    canon.starts_with(&root).then_some(canon)
}

fn digest_hex(raw: &str) -> Option<String> {
    let hex = raw
        .strip_prefix("sha256:")
        .or_else(|| raw.strip_prefix("sha256-"))?;
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

fn allocated(path: &Path, root: &Path) -> u64 {
    let Ok(md) = fs::symlink_metadata(path) else {
        return 0;
    };
    if crate::platform::is_dataless(&md) {
        return 0;
    }
    if md.file_type().is_symlink() && inside(root, path).is_none() {
        return 0;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::MetadataExt::blocks(&md) * 512
    }
    #[cfg(not(unix))]
    {
        md.len()
    }
}

struct Layer {
    hex: String,
    model: bool,
    json_size: u64,
}

fn read_store(dir: &Path) -> Vec<Listed> {
    let mut manifests = Vec::new();
    walk_files(&dir.join("manifests"), &mut manifests, 0);
    let mut parsed: Vec<Draft> = Vec::new();
    for path in manifests {
        if let Some(draft) = parse_manifest(dir, &path) {
            parsed.push(draft);
        }
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for draft in &parsed {
        for d in &draft.digests {
            *counts.entry(d.clone()).or_insert(0) += 1;
        }
    }
    let mut owner: HashMap<String, usize> = HashMap::new();
    for hex in counts.keys() {
        if counts[hex] <= 1 {
            continue;
        }
        let mut best: Option<usize> = None;
        for (i, draft) in parsed.iter().enumerate() {
            if !draft.digests.iter().any(|d| d == hex) {
                continue;
            }
            best = Some(match best {
                None => i,
                Some(b) => {
                    let larger = draft.logical > parsed[b].logical;
                    let tie = draft.logical == parsed[b].logical && draft.name < parsed[b].name;
                    if larger || tie {
                        i
                    } else {
                        b
                    }
                }
            });
        }
        if let Some(b) = best {
            owner.insert(hex.clone(), b);
        }
    }
    let mut out = Vec::new();
    for (i, draft) in parsed.iter().enumerate() {
        let mut exclusive = 0u64;
        let mut shared: Option<(u64, String)> = None;
        for hex in &draft.digests {
            let bytes = draft.alloc.get(hex).copied().unwrap_or(0);
            let rc = counts.get(hex).copied().unwrap_or(0);
            if rc <= 1 || owner.get(hex) == Some(&i) {
                exclusive += bytes;
            } else if let Some(&o) = owner.get(hex) {
                let bigger = shared.as_ref().map(|(s, _)| bytes > *s).unwrap_or(true);
                if bigger {
                    shared = Some((bytes, parsed[o].name.clone()));
                }
            }
        }
        let logical = draft.logical;
        out.push(Listed {
            name: draft.name.clone(),
            manifest: Some(draft.manifest.clone()),
            quantization: draft.quantization.clone(),
            logical,
            exclusive,
            shares_weights: exclusive == 0 && logical > 0,
            shares_with: shared.map(|(_, n)| n),
            weight: draft.weight.clone(),
            content_hash: draft.content_hash.clone(),
            orphan: false,
            last_day: draft.last_day,
            digests: draft.digests.clone(),
        });
    }
    let referenced: std::collections::HashSet<String> =
        counts.keys().map(|h| format!("sha256-{h}")).collect();
    if let Ok(entries) = fs::read_dir(dir.join("blobs")) {
        let mut blobs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        blobs.sort();
        for blob in blobs {
            let Some(name) = blob.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if referenced.contains(&name.to_ascii_lowercase()) {
                continue;
            }
            let Ok(md) = fs::symlink_metadata(&blob) else {
                continue;
            };
            if md.is_dir() || crate::platform::is_dataless(&md) {
                continue;
            }
            if md.file_type().is_symlink() {
                let Some(canon) = inside(dir, &blob) else {
                    continue;
                };
                if fs::metadata(&canon).is_ok_and(|m| m.is_dir()) {
                    continue;
                }
            }
            let bytes = allocated(&blob, dir);
            out.push(Listed {
                name: format!("Orphaned blob · {name}"),
                manifest: None,
                quantization: quant_of(name, Some(&blob)),
                logical: bytes,
                exclusive: bytes,
                shares_weights: false,
                shares_with: None,
                weight: Some(blob.clone()),
                content_hash: blob_content_hash(&blob),
                orphan: true,
                last_day: file_day(&blob),
                digests: Vec::new(),
            });
        }
    }
    out
}

struct Draft {
    name: String,
    manifest: PathBuf,
    quantization: Option<String>,
    logical: u64,
    digests: Vec<String>,
    alloc: HashMap<String, u64>,
    weight: Option<PathBuf>,
    content_hash: Option<String>,
    last_day: u32,
}

fn parse_manifest(dir: &Path, path: &Path) -> Option<Draft> {
    let rel = path.strip_prefix(dir.join("manifests")).ok()?;
    let parts: Vec<&str> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let name = display_name(&parts)?;
    let text = fs::read_to_string(path).ok()?;
    if text.len() > 2_000_000 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut layers = Vec::new();
    let mut digests = Vec::new();
    if let Some(raw) = value
        .get("config")
        .and_then(|c| c.get("digest"))
        .and_then(|d| d.as_str())
    {
        if let Some(hex) = digest_hex(raw) {
            push_digest(&mut digests, hex);
        }
    }
    if let Some(list) = value.get("layers").and_then(|l| l.as_array()) {
        for layer in list {
            let Some(raw) = layer.get("digest").and_then(|d| d.as_str()) else {
                continue;
            };
            let Some(hex) = digest_hex(raw) else {
                continue;
            };
            let media = layer
                .get("mediaType")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let json_size = layer.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
            push_digest(&mut digests, hex.clone());
            layers.push(Layer {
                hex,
                model: media.to_ascii_lowercase().contains("model"),
                json_size,
            });
        }
    }
    let primary = layers
        .iter()
        .filter(|l| l.model)
        .max_by_key(|l| l.json_size)
        .or_else(|| layers.iter().max_by_key(|l| l.json_size));
    let mut alloc = HashMap::new();
    let mut logical = 0u64;
    for hex in &digests {
        let blob = dir.join("blobs").join(format!("sha256-{hex}"));
        let bytes = allocated(&blob, dir);
        logical += bytes;
        alloc.insert(hex.clone(), bytes);
    }
    let weight = primary.map(|l| dir.join("blobs").join(format!("sha256-{}", l.hex)));
    let weight = weight.filter(|p| inside(dir, p).is_some());
    Some(Draft {
        quantization: quant_of(&name, weight.as_deref()),
        content_hash: weight.as_ref().and_then(|p| blob_content_hash(p)),
        name,
        manifest: path.to_path_buf(),
        logical,
        digests,
        alloc,
        weight,
        last_day: file_day(path),
    })
}

fn push_digest(out: &mut Vec<String>, hex: String) {
    if !out.contains(&hex) {
        out.push(hex);
    }
}

/// `registry.ollama.ai/library/llama3/latest` → `llama3:latest`.
fn display_name(parts: &[&str]) -> Option<String> {
    if parts.len() < 2 {
        return None;
    }
    let tag = parts[parts.len() - 1];
    let model = parts[parts.len() - 2];
    let namespace = if parts.len() >= 3 {
        parts[parts.len() - 3]
    } else {
        "library"
    };
    let host = if parts.len() >= 4 {
        parts[..parts.len() - 3].join("/")
    } else {
        "registry.ollama.ai".to_string()
    };
    let default_host = host == "registry.ollama.ai";
    Some(if default_host && namespace == "library" {
        format!("{model}:{tag}")
    } else if default_host {
        format!("{namespace}/{model}:{tag}")
    } else {
        format!("{host}/{namespace}/{model}:{tag}")
    })
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(md) = fs::symlink_metadata(&path) else {
            continue;
        };
        if md.is_dir() {
            walk_files(&path, out, depth + 1);
        } else if md.is_file() {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-ollama-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn blob(dir: &Path, hex: &str, bytes: usize) -> PathBuf {
        let path = dir.join("blobs").join(format!("sha256-{hex}"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&vec![1u8; bytes]).unwrap();
        path
    }

    fn manifest(dir: &Path, rel: &str, body: &str) {
        let path = dir.join("manifests").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn names_stay_inside_a_small_safe_set() {
        assert!(valid_name("llama3:latest"));
        assert!(valid_name("library/llama3:latest"));
        assert!(valid_name("wrasse-fixture:test"));
        assert!(!valid_name(""));
        assert!(!valid_name("../etc"));
        assert!(!valid_name("a\\b"));
        assert!(!valid_name("say \"hi\""));
        assert!(!valid_name(&"a".repeat(257)));
    }

    #[test]
    fn shared_blobs_count_once_and_orphans_stay_listed() {
        let root = scratch("store");
        let dir = root.join(".ollama");
        let hex_shared = "a".repeat(64);
        let hex_own = "b".repeat(64);
        blob(&dir.join("models"), &hex_shared, 10_000_000);
        blob(&dir.join("models"), &hex_own, 10_000_000);
        blob(&dir.join("models"), "orphan", 10_000_000);
        let big = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_shared}","size":10}},{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_own}","size":20}}]}}"#
        );
        let tiny = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_shared}","size":10}}]}}"#
        );
        manifest(
            &dir.join("models"),
            "registry.ollama.ai/library/llama-Q4_K_M/latest",
            &big,
        );
        manifest(
            &dir.join("models"),
            "registry.ollama.ai/library/tiny/latest",
            &tiny,
        );
        // Not JSON: ignored, and it does not hide the blobs it would have named.
        manifest(
            &dir.join("models"),
            "registry.ollama.ai/library/broken/latest",
            "nope",
        );
        let store = Store::open(&dir).expect("models layout");
        let big = store
            .models
            .iter()
            .find(|m| m.name == "llama-Q4_K_M:latest")
            .unwrap();
        let tiny = store
            .models
            .iter()
            .find(|m| m.name == "tiny:latest")
            .unwrap();
        let orphan = store.models.iter().find(|m| m.orphan).unwrap();
        assert_eq!(big.quantization.as_deref(), Some("Q4_K_M"));
        assert!(
            big.exclusive >= 20_000_000,
            "shared blob goes to the larger model"
        );
        assert!(!big.shares_weights);
        assert!(tiny.shares_weights);
        assert_eq!(tiny.exclusive, 0);
        assert!(tiny.logical >= 10_000_000);
        assert_eq!(tiny.shares_with.as_deref(), Some("llama-Q4_K_M:latest"));
        assert!(orphan.name.contains("sha256-orphan"));
        assert!(orphan.exclusive >= 10_000_000);
        let accounted: u64 = store.models.iter().map(|m| m.exclusive).sum();
        assert!(accounted >= 30_000_000);
        assert!(
            accounted < 40_000_000,
            "the shared blob is not counted twice"
        );
        let paths = exclusive_paths(&store.dir, "llama-Q4_K_M:latest");
        assert!(paths.iter().any(|p| p.ends_with("llama-Q4_K_M/latest")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with(format!("sha256-{hex_own}"))));
        assert!(!paths
            .iter()
            .any(|p| p.ends_with(format!("sha256-{hex_shared}"))));
        crate::platform::remove_test_dir(&root);
    }
}
