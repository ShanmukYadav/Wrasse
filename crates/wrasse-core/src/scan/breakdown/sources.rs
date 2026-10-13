//! Where things live on this Mac: AI tool and developer folders, cache and library names, weights
//! files, project places. Pure helpers for the builder.

use super::*;

/// Model folders (PLAN.md §6.6), honoring `OLLAMA_MODELS`, `HF_HUB_CACHE` and `HF_HOME`.
pub(super) fn ai_sources(home: &str, env: &HashMap<String, String>) -> Vec<(String, String)> {
    let ollama = env
        .get("OLLAMA_MODELS")
        .cloned()
        .unwrap_or_else(|| format!("{home}/.ollama"));
    let hf = env
        .get("HF_HUB_CACHE")
        .cloned()
        .or_else(|| env.get("HF_HOME").map(|h| format!("{h}/hub")))
        .unwrap_or_else(|| format!("{home}/.cache/huggingface"));
    vec![
        ("Ollama".into(), ollama),
        ("LM Studio".into(), format!("{home}/.lmstudio")),
        (
            "LM Studio (legacy)".into(),
            format!("{home}/.cache/lm-studio"),
        ),
        ("Hugging Face".into(), hf),
        ("Jan".into(), jan_support(home)),
        ("Jan".into(), format!("{home}/jan")),
        ("GPT4All".into(), gpt4all_support(home)),
    ]
}

pub(super) fn jan_support(home: &str) -> String {
    if cfg!(windows) {
        format!("{home}/AppData/Roaming/Jan")
    } else {
        format!("{home}/Library/Application Support/Jan")
    }
}

pub(super) fn gpt4all_support(home: &str) -> String {
    if cfg!(windows) {
        format!("{home}/AppData/Local/nomic.ai/GPT4All")
    } else {
        format!("{home}/Library/Application Support/nomic.ai/GPT4All")
    }
}

pub(super) fn strip_prefix(prefix: &str, path: &Path) -> String {
    let prefix = prefix.trim_end_matches('/');
    let shown = |rest: &str| {
        if rest.is_empty() {
            "/".to_string()
        } else if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        }
    };
    let prefix_path = Path::new(prefix);
    if let (Ok(root), Ok(canon)) = (
        std::fs::canonicalize(prefix_path),
        std::fs::canonicalize(path),
    ) {
        if let Ok(rest) = canon.strip_prefix(&root) {
            return shown(&rest.to_string_lossy());
        }
    }
    let text = path.to_string_lossy();
    text.strip_prefix(prefix)
        .map(shown)
        .unwrap_or_else(|| text.into_owned())
}

pub(super) fn overlaps(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

pub(super) fn tool_of(label: &str) -> crate::llm::ModelTool {
    match label {
        "Ollama" | "Ollama models" => crate::llm::ModelTool::Ollama,
        "LM Studio" | "LM Studio (legacy)" => crate::llm::ModelTool::LmStudio,
        "Hugging Face" | "Hugging Face cache" | "Hugging Face files" => {
            crate::llm::ModelTool::HuggingFace
        }
        "Jan" => crate::llm::ModelTool::Jan,
        "GPT4All" => crate::llm::ModelTool::Gpt4all,
        _ => crate::llm::ModelTool::Loose,
    }
}

/// A snapshot file to hash when looking for the same weights in another tool.
pub(super) fn first_weight(dir: &Path) -> Option<PathBuf> {
    fn walk(dir: &Path, depth: u32) -> Option<PathBuf> {
        if depth > 6 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if crate::platform::is_dataless(&md) {
                continue;
            }
            if md.is_dir() {
                if let Some(found) = walk(&path, depth + 1) {
                    return Some(found);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "gguf" | "safetensors" | "bin" | "pt" | "pth"))
            {
                return Some(path);
            }
        }
        None
    }
    walk(&dir.join("snapshots"), 0).or_else(|| walk(dir, 0))
}

pub(super) fn removal_of(tool: crate::llm::ModelTool) -> crate::llm::ModelRemoval {
    match tool {
        crate::llm::ModelTool::Ollama => crate::llm::ModelRemoval::Ollama,
        crate::llm::ModelTool::HuggingFace => crate::llm::ModelRemoval::HuggingFace,
        _ => crate::llm::ModelRemoval::Quarantine,
    }
}

/// True when `dir` has an entry named `name`. Does not open the file.
pub(super) fn has_dir_entry(dir: &str, name: &str) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .is_some_and(|rd| rd.flatten().any(|e| e.file_name() == name))
}

/// Known developer folders: toolchains, package caches, build products, simulators.
pub(super) fn dev_sources(home: &str) -> Vec<(String, String)> {
    let h = |p: &str| format!("{home}/{p}");
    vec![
        ("Xcode & simulators".into(), h("Library/Developer")),
        ("Android SDK".into(), h("Library/Android")),
        ("Docker".into(), h("Library/Containers/com.docker.docker")),
        ("Homebrew".into(), "/opt/homebrew".into()),
        ("Homebrew".into(), "/usr/local/Homebrew".into()),
        ("npm cache".into(), h(".npm")),
        ("pnpm store".into(), h("Library/pnpm")),
        ("pnpm store".into(), h(".local/share/pnpm/store")),
        ("Yarn cache".into(), h("Library/Caches/Yarn")),
        ("Bun".into(), h(".bun")),
        ("Cargo".into(), h(".cargo")),
        ("Rust toolchains".into(), h(".rustup")),
        ("Go".into(), h("go")),
        ("Gradle".into(), h(".gradle")),
        ("Maven".into(), h(".m2")),
        ("CocoaPods".into(), h(".cocoapods")),
        ("CocoaPods cache".into(), h("Library/Caches/CocoaPods")),
        ("NuGet".into(), h(".nuget")),
        ("pip cache".into(), h("Library/Caches/pip")),
        ("pyenv".into(), h(".pyenv")),
        ("Conda".into(), h(".conda")),
        ("Miniconda".into(), h("miniconda3")),
        ("Miniconda".into(), h("Miniconda3")),
        ("Anaconda".into(), h("anaconda3")),
        ("Miniforge".into(), h("miniforge3")),
        ("Mambaforge".into(), h("mambaforge")),
        ("Docker CLI".into(), h(".docker")),
        ("Android".into(), h(".android")),
        ("Gems".into(), h(".gem")),
        ("VS Code extensions".into(), h(".vscode/extensions")),
    ]
}

pub(super) fn is_rebuildable_dev(label: &str) -> bool {
    matches!(
        label,
        "npm cache" | "pnpm store" | "Yarn cache" | "CocoaPods cache" | "pip cache"
    )
}

/// Folders whose names say "this is a cache".
pub(super) fn is_cache_name(name: &str) -> bool {
    matches!(
        name,
        "Caches"
            | "Cache"
            | "caches"
            | "cache"
            | "DerivedData"
            | "_cacache"
            | "node_modules"
            | ".pnpm-store"
            | "Code Cache"
            | "GPUCache"
    )
}

/// Library folders that hold data the user can't get back (mail, messages, keychains, iCloud).
pub(super) fn is_protected_library(name: &str) -> bool {
    matches!(
        name,
        "Keychains"
            | "Mobile Documents"
            | "CloudStorage"
            | "Mail"
            | "Messages"
            | "Photos"
            | "Accounts"
            | "Cookies"
            | "Calendars"
            | "Reminders"
            | "Safari"
            | "Group Containers"
    )
}

/// App-managed libraries: shown for size, never opened up or cleaned.
pub(super) fn is_read_only_library(name: &str) -> bool {
    name.ends_with(".photoslibrary")
        || name.ends_with(".musiclibrary")
        || name.ends_with(".tvlibrary")
        || name.ends_with(".fcpbundle")
        || name.ends_with(".logicx")
}

/// Model weights worth listing on their own. The extension and size only decide whether to look;
/// [`crate::scan::weights::is_model_weights`] has to see a real header or a sibling `config.json`
/// with `model_type` or `architectures`.
pub(super) fn is_weights_file(path: &str, name: &str, size: u64) -> bool {
    let n = name.to_ascii_lowercase();
    let weights = [
        ".gguf",
        ".safetensors",
        ".ckpt",
        ".pt",
        ".pth",
        ".onnx",
        ".mlmodel",
        ".tflite",
        ".h5",
    ];
    let sized = if weights.iter().any(|e| n.ends_with(e)) {
        size >= 50_000_000
    } else {
        n.ends_with(".bin")
            && (size >= MODEL_FILE
                || ((n.starts_with("pytorch_model") || n.starts_with("model"))
                    && size >= 50_000_000))
    };
    sized && crate::scan::weights::is_model_weights(Path::new(path))
}

/// Documents, Desktop, and iCloud Drive. Models here are project files, shown together.
pub(super) fn project_place(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    parts.iter().any(|s| matches!(*s, "Documents" | "Desktop"))
        || p.contains("/Library/Mobile Documents/")
        || p.contains("/com~apple~CloudDocs/")
}

/// A Hugging Face cache that lives in a project (for example `Documents/vyakta/env/hf_cache`),
/// not the account-wide cache under `~/.cache`. It is project data.
pub(crate) fn project_hf_cache(path: &str) -> bool {
    if !project_place(path) {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    lower.contains("/hf_cache/")
        || lower.ends_with("/hf_cache")
        || lower.contains("/huggingface/")
        || lower.ends_with("/huggingface")
        || lower.contains("/hub/models--")
}

/// Groups found models. Anything under Documents, Desktop, or iCloud Drive is "Project models".
/// Other finds use the first folder under home that isn't a generic container.
/// True when Spotlight's last-used instant is the item's creation instant.
pub(super) fn spotlight_matches_birth(path: &Path, secs: i64) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(created) = md.created() else {
        return false;
    };
    let Ok(birth) = created.duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    secs.abs_diff(birth.as_secs() as i64) <= 5
}

pub(super) fn model_group(display: &str, home: &str) -> (String, String) {
    // Containers, not projects: a model in ~/Library/Mobile Documents/com~apple~CloudDocs/Backup/…
    // groups under "Backup", one in ~/Library/Application Support/SomeApp/… under "SomeApp".
    const GENERIC: [&str; 20] = [
        "Documents",
        "Desktop",
        "Downloads",
        "Developer",
        "Projects",
        "projects",
        "Code",
        "code",
        "src",
        "repos",
        "workspace",
        "dev",
        "git",
        "GitHub",
        "Library",
        "Mobile Documents",
        "com~apple~CloudDocs",
        "Application Support",
        "Containers",
        "Caches",
    ];
    let Some(rel) = (!home.is_empty())
        .then(|| display.strip_prefix(home))
        .flatten()
    else {
        return ("Elsewhere on this Mac".into(), "/".into());
    };
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    if project_place(display) {
        let mut path = home.to_string();
        for p in &parts {
            path.push('/');
            path.push_str(p);
            if matches!(*p, "Documents" | "Desktop" | "Mobile Documents") {
                return ("Project models".into(), path);
            }
        }
        return ("Project models".into(), home.to_string());
    }
    let mut path = home.to_string();
    for (i, p) in parts.iter().enumerate() {
        path.push('/');
        path.push_str(p);
        if i + 1 < parts.len() && !GENERIC.contains(p) {
            return ((*p).to_string(), path);
        }
    }
    ("Your folders".into(), home.to_string())
}
