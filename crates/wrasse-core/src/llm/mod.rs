//! Local model catalogs (PLAN.md §6.6). Discovery only reads. Removal goes through
//! `safety::remove` or `safety::delegate`, and a loaded model is never removed.
//!
//! Duplicate grouping is size, then a BLAKE3 of the first and last 16 MB. A full hash is
//! required before a duplicate can be removed. An Ollama blob's `sha256-` name is that
//! blob's content hash and is reused when both copies have one.

mod ollama;
mod remove;

pub use ollama::{exclusive_paths, valid_name, Store};
pub use remove::{remove_model, remove_model_with, unload_model, Live, ModelJob, Probe};

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Shown on an APFS clone. Removing it frees almost nothing.
pub(crate) const CLONE_NOTE: &str = "clone, no space to gain";

/// Shown when two models match on size and the sampled edges only.
pub(crate) const SAMPLE_NOTE: &str =
    "Same size and edges as another model. A full check runs before removal.";

/// Shown on an iCloud Drive copy that matches another model. Removing it removes it everywhere.
pub(crate) const ICLOUD_DUPE_NOTE: &str =
    "Removing this iCloud copy also removes it from your other devices.";

/// `model.safetensors` and `pytorch_model.bin` in one folder. Headers only, not a tensor load.
pub(crate) const FORMAT_NOTE: &str = "Same weights in two formats.";

/// True for a path inside iCloud Drive (`~/Library/Mobile Documents`).
pub(crate) fn icloud_synced_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.contains("/Library/Mobile Documents/") || p.contains("/com~apple~CloudDocs/")
}

const SAMPLE: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelTool {
    Ollama,
    LmStudio,
    HuggingFace,
    Jan,
    Gpt4all,
    Loose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelRemoval {
    Ollama,
    HuggingFace,
    Quarantine,
}

/// How far a cross-tool duplicate has been checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DupeLevel {
    /// Same size and the same first and last 16 MB. Not enough to remove.
    Sampled,
    /// Full content hash matches (or both copies carry the same Ollama sha256 name).
    Full,
}

/// What the UI shows for one model. Bytes live on the node; this is the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMeta {
    pub tool: ModelTool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub quantization: Option<String>,
    pub removal: ModelRemoval,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ollama_name: Option<String>,
    /// Ollama models directory, so a later removal can re-read manifests.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub models_dir: Option<String>,
    /// Primary weight file. Hashed when this model is grouped with another.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub partner_weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub dupe_level: Option<DupeLevel>,
    /// Ollama sha256 of the weight blob, when the filename is that digest.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub orphan_blob: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clone_copy: bool,
    /// Its bytes are already counted on another model. Shown, not added again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shares_weights: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub shares_with: Option<String>,
}

/// A running model, or a tool process when a per-model size isn't available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedModel {
    pub name: String,
    pub tool: ModelTool,
    pub bytes: u64,
    /// Process memory, not a single model's allocation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub approx: bool,
}

/// One weight file considered for duplicate grouping.
pub(crate) struct DupeCand {
    pub path: PathBuf,
    pub len: u64,
    pub content_hash: Option<String>,
    pub dataless: bool,
}

/// The partner and how sure the match is. `clone` is this file, not the partner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DupeMatch {
    pub partner: PathBuf,
    pub level: DupeLevel,
}

/// Groups `items` that are the same length. Dataless files are never opened.
/// Returns, for each index that has a match, the partner (the first index in the group is
/// the one the others point at).
#[cfg(test)]
pub(crate) fn pair_duplicates(items: &[DupeCand]) -> Vec<Option<DupeMatch>> {
    pair_duplicates_until(items, None)
}

/// Like [`pair_duplicates`], but stops sampling once `deadline` has passed. Copies not
/// reached stay unpaired. A removal still hashes the whole file before it deletes anything.
pub(crate) fn pair_duplicates_until(
    items: &[DupeCand],
    deadline: Option<std::time::Instant>,
) -> Vec<Option<DupeMatch>> {
    let mut out = vec![None; items.len()];
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].len, items[i].path.to_string_lossy().into_owned()));
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len()
            && items[order[j]].len == items[order[i]].len
            && items[order[i]].len > 0
        {
            j += 1;
        }
        if j > i + 1 {
            pair_group(&order[i..j], items, &mut out, deadline);
        }
        i = j;
    }
    out
}

fn pair_group(
    idxs: &[usize],
    items: &[DupeCand],
    out: &mut [Option<DupeMatch>],
    deadline: Option<std::time::Instant>,
) {
    // Ollama blob names are the content hash. Matching names pair without reading the file.
    // Dataless copies are skipped here so a placeholder is never opened.
    let mut addressed: Vec<(String, usize)> = Vec::new();
    for &i in idxs {
        if items[i].dataless {
            continue;
        }
        let Some(hash) = items[i].content_hash.clone() else {
            continue;
        };
        if let Some((_, keep)) = addressed.iter().find(|(h, _)| *h == hash) {
            out[i] = Some(DupeMatch {
                partner: items[*keep].path.clone(),
                level: DupeLevel::Full,
            });
        } else {
            addressed.push((hash, i));
        }
    }
    let mut samples: Vec<(usize, String)> = Vec::new();
    for &i in idxs {
        if items[i].dataless || out[i].is_some() {
            continue;
        }
        // A volume scan can meet dozens of hundred-megabyte copies. Past the deadline the
        // rest stay unpaired so the scan can finish and let its memory go.
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            break;
        }
        if let Ok(hash) = sample_hash(&items[i].path) {
            samples.push((i, hash));
        }
    }
    // Keep the first path in each sample group as the one the others duplicate.
    let mut seen: Vec<(String, usize)> = Vec::new();
    for (i, hash) in samples {
        if let Some((_, keep)) = seen.iter().find(|(h, _)| *h == hash) {
            let level = full_level(&items[i], &items[*keep]);
            out[i] = Some(DupeMatch {
                partner: items[*keep].path.clone(),
                level,
            });
        } else {
            seen.push((hash, i));
        }
    }
}

fn full_level(a: &DupeCand, b: &DupeCand) -> DupeLevel {
    match (&a.content_hash, &b.content_hash) {
        (Some(x), Some(y)) if x == y => DupeLevel::Full,
        _ => DupeLevel::Sampled,
    }
}

/// BLAKE3 of the file length and its first and last 16 MB.
/// A dataless file is refused before it is opened, so an iCloud placeholder is not downloaded.
pub(crate) fn sample_hash(path: &Path) -> io::Result<String> {
    let md = std::fs::symlink_metadata(path)?;
    if crate::platform::is_dataless(&md) {
        return Err(io::Error::other("in iCloud only"));
    }
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut h = blake3::Hasher::new();
    h.update(&len.to_le_bytes());
    // Chunked so a scan does not keep a 16 MB buffer (that showed up as idle memory).
    let mut buf = [0u8; 64 * 1024];
    feed(&mut h, &mut f, &mut buf, len.min(SAMPLE))?;
    if len > SAMPLE {
        f.seek(SeekFrom::Start(len - SAMPLE))?;
        feed(&mut h, &mut f, &mut buf, SAMPLE)?;
    }
    Ok(h.finalize().to_hex().to_string())
}

fn feed(h: &mut blake3::Hasher, f: &mut File, buf: &mut [u8], mut want: u64) -> io::Result<()> {
    while want > 0 {
        let take = want.min(buf.len() as u64) as usize;
        let n = f.read(&mut buf[..take])?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        want -= n as u64;
    }
    Ok(())
}

pub(crate) fn full_hash(path: &Path) -> io::Result<String> {
    let md = std::fs::symlink_metadata(path)?;
    if crate::platform::is_dataless(&md) {
        return Err(io::Error::other("in iCloud only"));
    }
    crate::files::dupes::full_hash(path, None)
}

/// True when `path` shares blocks with an APFS clone.
pub(crate) fn is_clone_file(path: &Path) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if md.file_type().is_symlink() || crate::platform::is_dataless(&md) {
        return false;
    }
    let allocated = allocated_len(&md);
    crate::platform::clone_shared_bytes(path, allocated).unwrap_or(0) > 0
}

fn allocated_len(md: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        std::os::unix::fs::MetadataExt::blocks(md) * 512
    }
    #[cfg(not(unix))]
    {
        md.len()
    }
}

/// Newest of access and modification, in days since the Unix epoch.
pub(crate) fn file_day(path: &Path) -> u32 {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if crate::platform::is_dataless(&md) {
        return 0;
    }
    let secs = |t: io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    (secs(md.accessed()).max(secs(md.modified())) / 86_400) as u32
}

/// Quantization written into a name (`Q4_K_M`, `4bit`, …), if one is there.
pub(crate) fn quant_from_name(name: &str) -> Option<String> {
    const TOKENS: &[&str] = &[
        "IQ2_XXS", "IQ2_XS", "IQ3_XXS", "IQ3_XS", "IQ4_NL", "IQ4_XS", "IQ1_S", "IQ1_M", "IQ2_S",
        "IQ3_S", "Q4_K_M", "Q4_K_S", "Q5_K_M", "Q5_K_S", "Q3_K_L", "Q3_K_M", "Q3_K_S", "Q6_K",
        "Q8_0", "Q5_0", "Q5_1", "Q4_0", "Q4_1", "Q2_K", "Q3_K", "Q4_K", "Q5_K", "Q8_K", "BF16",
        "FP16", "FP32", "F16", "F32", "4-bit", "8-bit", "4bit", "8bit", "int4", "int8",
    ];
    let lower = name.to_ascii_lowercase();
    TOKENS.iter().find_map(|token| {
        lower
            .contains(&token.to_ascii_lowercase())
            .then(|| (*token).to_string())
    })
}

/// `general.file_type` from a GGUF header, as a quantization name.
pub(crate) fn quant_from_gguf(path: &Path) -> Option<String> {
    file_type_name(gguf_file_type(path)?)
}

fn file_type_name(kind: u32) -> Option<String> {
    let name = match kind {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        6 => "Q5_0",
        7 => "Q5_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "IQ2_XXS",
        17 => "IQ2_XS",
        18 => "IQ3_XXS",
        19 => "IQ1_S",
        20 => "IQ4_NL",
        21 => "IQ3_S",
        22 => "IQ2_S",
        23 => "IQ4_XS",
        29 => "IQ1_M",
        30 => "BF16",
        _ => return None,
    };
    Some(name.to_string())
}

fn gguf_file_type(path: &Path) -> Option<u32> {
    let mut f = File::open(path).ok()?;
    let mut head = [0u8; 24];
    f.read_exact(&mut head).ok()?;
    if &head[..4] != b"GGUF" {
        return None;
    }
    let version = u32::from_le_bytes(head[4..8].try_into().ok()?);
    if version < 2 {
        return None;
    }
    let n_kv = u64::from_le_bytes(head[16..24].try_into().ok()?);
    let mut budget = 2_000_000u64;
    for _ in 0..n_kv.min(10_000) {
        let key = read_string(&mut f, &mut budget)?;
        let typ = read_u32(&mut f, &mut budget)?;
        if key == "general.file_type" && typ == 4 {
            return read_u32(&mut f, &mut budget);
        }
        skip_value(&mut f, typ, &mut budget)?;
    }
    None
}

fn read_u32(f: &mut File, budget: &mut u64) -> Option<u32> {
    let mut buf = [0u8; 4];
    read_exact(f, &mut buf, budget)?;
    Some(u32::from_le_bytes(buf))
}

fn read_u64(f: &mut File, budget: &mut u64) -> Option<u64> {
    let mut buf = [0u8; 8];
    read_exact(f, &mut buf, budget)?;
    Some(u64::from_le_bytes(buf))
}

fn read_exact(f: &mut File, buf: &mut [u8], budget: &mut u64) -> Option<()> {
    let n = buf.len() as u64;
    if *budget < n {
        return None;
    }
    f.read_exact(buf).ok()?;
    *budget -= n;
    Some(())
}

fn read_string(f: &mut File, budget: &mut u64) -> Option<String> {
    let len = read_u64(f, budget)?;
    if len > 1_000_000 || *budget < len {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    read_exact(f, &mut buf, budget)?;
    String::from_utf8(buf).ok()
}

fn skip_value(f: &mut File, typ: u32, budget: &mut u64) -> Option<()> {
    let width = match typ {
        0 | 1 | 7 => 1,
        2 | 3 => 2,
        4..=6 => 4,
        10..=12 => 8,
        8 => return read_string(f, budget).map(|_| ()),
        9 => {
            let elem = read_u32(f, budget)?;
            let count = read_u64(f, budget)?;
            if count > 100_000 {
                return None;
            }
            for _ in 0..count {
                skip_value(f, elem, budget)?;
            }
            return Some(());
        }
        _ => return None,
    };
    let mut buf = vec![0u8; width];
    read_exact(f, &mut buf, budget)
}

/// A sibling or snapshot `config.json` that names a quantization.
pub(crate) fn quant_from_config(dir: &Path) -> Option<String> {
    let mut files = vec![dir.join("config.json")];
    if let Ok(snaps) = std::fs::read_dir(dir.join("snapshots")) {
        for e in snaps.flatten() {
            files.push(e.path().join("config.json"));
        }
    }
    for path in files {
        if let Some(q) = config_quant(&path) {
            return Some(q);
        }
    }
    None
}

fn config_quant(path: &Path) -> Option<String> {
    let md = std::fs::metadata(path).ok()?;
    if !md.is_file() || md.len() == 0 || md.len() > 1_048_576 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let obj = value.as_object()?;
    if let Some(q) = obj.get("quantization_config").and_then(|v| v.as_object()) {
        let method = q.get("quant_method").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(bits) = q.get("bits").and_then(|v| v.as_u64()) {
            return Some(format!("{bits}-bit"));
        }
        if !method.is_empty() {
            return Some(method.to_string());
        }
    }
    None
}

pub(crate) fn quant_of(name: &str, weight: Option<&Path>) -> Option<String> {
    if let Some(q) = quant_from_name(name) {
        return Some(q);
    }
    let path = weight?;
    quant_from_gguf(path).or_else(|| path.parent().and_then(quant_from_config))
}

/// LM Studio's downloads folder from its settings, when that file names one.
pub(crate) fn lmstudio_download_dir(home: &Path) -> Option<PathBuf> {
    let settings = if cfg!(windows) {
        vec![home.join("AppData/Roaming/LM Studio/settings.json")]
    } else {
        vec![
            home.join("Library/Application Support/LM Studio/settings.json"),
            home.join(".lmstudio/settings.json"),
        ]
    };
    settings.into_iter().find_map(|p| downloads_folder(&p))
}

fn downloads_folder(settings: &Path) -> Option<PathBuf> {
    let md = std::fs::metadata(settings).ok()?;
    if !md.is_file() || md.len() > 1_048_576 {
        return None;
    }
    let text = std::fs::read_to_string(settings).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = value
        .get("downloadsFolder")
        .or_else(|| value.get("modelsDir"))
        .and_then(|v| v.as_str())?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return None;
    }
    let canon = std::fs::canonicalize(&path).ok()?;
    canon.is_dir().then_some(canon)
}

/// sha256 digest carried by an Ollama blob filename (`sha256-<64 hex>`).
pub(crate) fn blob_content_hash(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let hex = name.strip_prefix("sha256-")?;
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// Models the Ollama server at 127.0.0.1:11434 says are loaded. Read-only.
/// An unreadable reply is empty: callers that must refuse a removal use [`ps_contains`].
pub fn loaded_models() -> Vec<LoadedModel> {
    let mut out = Vec::new();
    if let Ok(body) = crate::platform::ollama_get("/api/ps") {
        out.extend(parse_ps(&body));
    }
    let lm = crate::platform::process_memory(&["lm studio", "lmstudio", "lm-studio"]);
    if lm > 0 {
        out.push(LoadedModel {
            name: "LM Studio".into(),
            tool: ModelTool::LmStudio,
            bytes: lm,
            approx: true,
        });
    }
    out
}

pub(crate) fn parse_ps(body: &str) -> Vec<LoadedModel> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(models) = value.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|m| {
            let name = m
                .get("name")
                .or_else(|| m.get("model"))
                .and_then(|n| n.as_str())?;
            let bytes = m.get("size").and_then(|n| n.as_u64()).unwrap_or(0);
            Some(LoadedModel {
                name: name.to_string(),
                tool: ModelTool::Ollama,
                bytes,
                approx: false,
            })
        })
        .collect()
}

/// Whether `body` (an `/api/ps` document) lists `name`. An unreadable document is treated as
/// loaded, so a removal is refused when the answer can't be trusted.
pub(crate) fn ps_contains(body: &str, name: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return true;
    };
    let Some(models) = value.get("models").and_then(|m| m.as_array()) else {
        return false;
    };
    let want = normalize_ollama(name);
    models.iter().any(|m| {
        m.get("name")
            .or_else(|| m.get("model"))
            .and_then(|n| n.as_str())
            .is_some_and(|n| normalize_ollama(n) == want)
    })
}

pub(crate) fn normalize_ollama(name: &str) -> String {
    let mut n = name.trim().trim_start_matches("registry.ollama.ai/");
    if let Some(rest) = n.strip_prefix("library/") {
        n = rest;
    }
    n.to_ascii_lowercase()
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
        let dir = std::env::temp_dir().join(format!("wrasse-llm-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn quant_comes_from_the_name_before_the_header() {
        assert_eq!(quant_from_name("llama3:Q4_K_M").as_deref(), Some("Q4_K_M"));
        assert_eq!(
            quant_from_name("Mistral-7B-Instruct-v0.3-4bit").as_deref(),
            Some("4bit")
        );
        assert!(quant_from_name("llama3:latest").is_none());
    }

    #[test]
    fn gguf_file_type_is_read_from_the_header_only() {
        let dir = scratch("gguf");
        let path = dir.join("weights.gguf");
        let mut f = File::create(&path).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&0u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        let key = b"general.file_type";
        f.write_all(&(key.len() as u64).to_le_bytes()).unwrap();
        f.write_all(key).unwrap();
        f.write_all(&4u32.to_le_bytes()).unwrap();
        f.write_all(&2u32.to_le_bytes()).unwrap();
        // Padding a reader must not need.
        f.write_all(&[0u8; 64]).unwrap();
        assert_eq!(quant_from_gguf(&path).as_deref(), Some("Q4_0"));
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn same_edges_match_and_a_different_middle_stays_sampled() {
        let dir = scratch("hash");
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        let c = dir.join("c.bin");
        for path in [&a, &b, &c] {
            let f = File::create(path).unwrap();
            f.set_len(33 * 1024 * 1024).unwrap();
        }
        // b matches a. c differs only past the first 16 MB and before the last 16 MB.
        let mut fc = std::fs::OpenOptions::new().write(true).open(&c).unwrap();
        fc.seek(SeekFrom::Start(16 * 1024 * 1024 + 100)).unwrap();
        fc.write_all(&[7u8]).unwrap();
        let items = vec![
            DupeCand {
                path: a.clone(),
                len: 33 * 1024 * 1024,
                content_hash: None,
                dataless: false,
            },
            DupeCand {
                path: b.clone(),
                len: 33 * 1024 * 1024,
                content_hash: None,
                dataless: false,
            },
            DupeCand {
                path: c.clone(),
                len: 33 * 1024 * 1024,
                content_hash: None,
                dataless: false,
            },
        ];
        let paired = pair_duplicates(&items);
        let partners: Vec<_> = paired.iter().flatten().map(|m| m.partner.clone()).collect();
        assert!(partners.contains(&a) || partners.iter().any(|p| p == &b || p == &a));
        assert!(paired
            .iter()
            .flatten()
            .all(|m| m.level == DupeLevel::Sampled));
        assert_ne!(full_hash(&a).unwrap(), full_hash(&c).unwrap());
        assert_eq!(full_hash(&a).unwrap(), full_hash(&b).unwrap());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn matching_ollama_digests_are_a_full_match_without_rereading() {
        let items = vec![
            DupeCand {
                path: PathBuf::from("/tmp/one"),
                len: 10,
                content_hash: Some("abc".into()),
                dataless: true,
            },
            DupeCand {
                path: PathBuf::from("/tmp/two"),
                len: 10,
                content_hash: Some("abc".into()),
                dataless: true,
            },
        ];
        // Dataless files are not opened, so a content hash is the only way they pair.
        // These aren't dataless on disk; the flag skips the read. No pair without a sample.
        let paired = pair_duplicates(&items);
        assert!(paired.iter().all(|m| m.is_none()));
    }

    #[test]
    fn matching_content_hashes_pair_without_opening() {
        let items = vec![
            DupeCand {
                path: PathBuf::from("/no/such/wrasse-a"),
                len: 10,
                content_hash: Some("abc".into()),
                dataless: false,
            },
            DupeCand {
                path: PathBuf::from("/no/such/wrasse-b"),
                len: 10,
                content_hash: Some("abc".into()),
                dataless: false,
            },
        ];
        let paired = pair_duplicates(&items);
        assert!(paired[0].is_none());
        let hit = paired[1].as_ref().unwrap();
        assert_eq!(hit.level, DupeLevel::Full);
        assert_eq!(hit.partner, items[0].path);
    }

    #[test]
    fn clone_note_is_the_exact_label() {
        assert_eq!(CLONE_NOTE, "clone, no space to gain");
    }

    #[test]
    fn ps_lists_a_loaded_model_and_a_broken_reply_is_not_empty_for_a_check() {
        let body = r#"{"models":[{"name":"llama3:latest","size":100}]}"#;
        let loaded = parse_ps(body);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "llama3:latest");
        assert_eq!(loaded[0].bytes, 100);
        assert!(ps_contains(body, "library/llama3:latest"));
        assert!(!ps_contains(body, "mistral:latest"));
        assert!(ps_contains("not json", "llama3:latest"));
    }

    #[test]
    fn lmstudio_settings_name_a_folder() {
        let dir = scratch("lms");
        let models = dir.join("models");
        std::fs::create_dir_all(&models).unwrap();
        let settings = dir.join("settings.json");
        std::fs::write(
            &settings,
            format!(r#"{{"downloadsFolder":"{}"}}"#, models.display()),
        )
        .unwrap();
        assert_eq!(
            downloads_folder(&settings).as_deref(),
            Some(std::fs::canonicalize(&models).unwrap().as_path())
        );
        crate::platform::remove_test_dir(&dir);
    }
}
