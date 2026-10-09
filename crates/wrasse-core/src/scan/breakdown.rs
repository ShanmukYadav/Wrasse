//! Turns a raw scan into what the UI shows: a volume split into categories (Your Files,
//! AI Models, Developer, Apps, Library & caches, System, Other), each a tree of folders and
//! large items. This is the `Disk` shape in `app/src/api/types.ts`.
//!
//! The tree sent to the UI is pruned: under each folder, items smaller than [`MIN_ITEM`] (and the
//! folder's small files) are folded into one "Smaller items" entry, and at most [`MAX_CHILDREN`]
//! items are listed. The full tree stays in the core.

use super::tree::{flag, Kind, Tree};
use crate::platform::Volumes;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Items smaller than this fold into "Smaller items".
pub const MIN_ITEM: u64 = 10_000_000;
/// At most this many items are listed under one folder.
pub const MAX_CHILDREN: usize = 60;
/// Loose model files at least this large count as models (PLAN.md §6.6).
const MODEL_FILE: u64 = 500_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Files,
    AiModels,
    Developer,
    Apps,
    System,
    Junk,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Volume,
    Dir,
    File,
    App,
    Model,
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    Safe,
    YourCall,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Leftover {
    Caches,
    Support,
    Containers,
    Logs,
    Preferences,
    SavedState,
}

/// Lens bits, as `LENS_BIT` in `app/src/api/types.ts`.
pub mod lens {
    pub const JUNK: u32 = 1;
    pub const APPS: u32 = 2;
    pub const AI_MODELS: u32 = 4;
    pub const DEV: u32 = 8;
    pub const LARGE_OLD: u32 = 16;
    pub const GREW: u32 = 32;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchBytes {
    #[serde(skip_serializing_if = "is_zero", default)]
    pub junk: u64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub apps: u64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub ai_models: u64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub dev: u64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub large_old: u64,
    #[serde(skip_serializing_if = "is_zero", default)]
    pub grew: u64,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl MatchBytes {
    fn add(&mut self, o: &MatchBytes) {
        self.junk += o.junk;
        self.apps += o.apps;
        self.ai_models += o.ai_models;
        self.dev += o.dev;
        self.large_old += o.large_old;
        self.grew += o.grew;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiNode {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub parent: i64,
    pub children: Vec<u32>,
    pub depth: u32,
    pub category: Category,
    pub kind: NodeKind,
    pub last_opened_days: u32,
    pub safety: Safety,
    pub why: String,
    pub lenses: u32,
    pub deep: u32,
    pub match_bytes: MatchBytes,
    pub grew_by: i64,
    pub shade: u8,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duplicate_of: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub read_only: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub rebuildable: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub running: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub app_bundle: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub leftover: Option<Leftover>,
    /// Labels beyond the UI's type: iCloud-only, unreadable, shares blocks with clones.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDisk {
    pub nodes: Vec<UiNode>,
    pub root: u32,
    pub capacity: u64,
    /// The "held by snapshots" node; -1 until Space Diagnostics (Phase 4).
    pub ghost_id: i64,
    pub leaf_count: u32,
    pub summary: Summary,
}

/// Totals for the checkpoint and the System data explanation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub capacity: u64,
    pub available: u64,
    pub os_used: u64,
    pub scanned: u64,
    pub unaccounted: u64,
    pub home: u64,
    pub files: u64,
    pub dirs: u64,
    pub unreadable_dirs: u64,
    pub dataless_items: u64,
    pub hardlinks_deduped: u64,
    pub clone_shared_bytes: u64,
    pub cancelled: bool,
    /// This scan only re-read the folders FSEvents reported as changed.
    #[serde(default)]
    pub incremental: bool,
    #[serde(default)]
    pub changed_folders: u64,
}

/// What the breakdown needs besides the tree.
pub struct Input<'a> {
    pub tree: &'a Tree,
    /// Prefix of the scan root to strip from paths (`/System/Volumes/Data` on macOS).
    pub display_prefix: &'a str,
    pub home: Option<PathBuf>,
    pub volumes: Volumes,
    pub volume_name: String,
    /// Folder sizes from the previous scan, by display path ("what grew").
    pub previous: Option<HashMap<String, u64>>,
    /// Paths of running executables, to mark running apps.
    pub running: Vec<PathBuf>,
    /// Today, in days since the Unix epoch.
    pub today: u32,
    /// Environment overrides for model folders (`OLLAMA_MODELS`, `HF_HOME`, `HF_HUB_CACHE`).
    pub env: HashMap<String, String>,
    /// Reads an app's bundle identifier (Info.plist); injected so tests need no real apps.
    pub bundle_id: &'a dyn Fn(&Path) -> Option<String>,
    /// Lists the models in a Hugging Face hub folder with their real sizes (scan/hf.rs).
    pub hf_models: &'a dyn Fn(&Path) -> Vec<super::hf::HfModel>,
}

/// AI model data found outside the known tool folders, grouped by the project it sits in.
struct Found {
    group: String,
    group_path: String,
    item: FoundItem,
}

enum FoundItem {
    /// A Hugging Face hub cache (has `models--org--name` folders).
    Hub(u32),
    /// An Ollama model store (`models/` with `blobs/` and `manifests/`).
    Ollama(u32),
    /// A model weights file (.gguf, .safetensors, …).
    Weights(u32),
}

#[derive(Clone, Copy)]
struct Ctx {
    category: Category,
    /// Safety for ordinary items in this subtree.
    safety: Safety,
    rebuildable: bool,
    leftover: Option<Leftover>,
}

struct Builder<'a> {
    input: &'a Input<'a>,
    tree: &'a Tree,
    nodes: Vec<UiNode>,
    /// Arena nodes shown somewhere else (moved under a category or an app).
    carved: HashSet<u32>,
    /// Bytes carved out from under each arena node.
    carved_below: HashMap<u32, u64>,
}

pub fn build(input: &Input) -> UiDisk {
    let tree = input.tree;
    let mut b = Builder {
        input,
        tree,
        nodes: Vec::new(),
        carved: HashSet::new(),
        carved_below: HashMap::new(),
    };
    let used = input.volumes.used();
    let root = b.virtual_node(
        &input.volume_name,
        "/",
        None,
        Category::Other,
        NodeKind::Volume,
        Safety::Keep,
        "",
    );

    let home = input
        .home
        .as_ref()
        .map(|h| h.to_string_lossy().into_owned());
    let home_display = home.clone().unwrap_or_default();

    // Claim special folders first, so each byte shows up in exactly one place.
    let ai = b.claim_all(&ai_sources(&home_display, &input.env));
    // Models anywhere else on the Mac (a project's own cache, downloaded weights).
    let found = b.discover_models(&home_display);
    let dev = b.claim_all(&dev_sources(&home_display));
    let apps = b.claim_apps(&home_display);
    let trash = b.claim(&format!("{home_display}/.Trash"));
    let library = b.claim(&format!("{home_display}/Library"));
    let home_id = b.claim(&home_display);

    // Your Files: the home folder, minus Library and everything claimed above.
    let files = b.virtual_node("Your Files", &home_display, Some(root), Category::Files, NodeKind::Dir, Safety::YourCall, "Your documents, downloads, photos and projects. Wrasse never removes these without asking.");
    if let Some(h) = home_id {
        let ctx = Ctx {
            category: Category::Files,
            safety: Safety::YourCall,
            rebuildable: false,
            leftover: None,
        };
        b.emit_children(h, files, ctx);
    }

    // AI Models.
    let ai_node = b.virtual_node(
        "AI Models",
        &format!("{home_display}/.ollama"),
        Some(root),
        Category::AiModels,
        NodeKind::Dir,
        Safety::YourCall,
        "Local AI models from Ollama, LM Studio, Hugging Face and other tools.",
    );
    for (label, id) in ai {
        let ctx = Ctx {
            category: Category::AiModels,
            safety: Safety::YourCall,
            rebuildable: false,
            leftover: None,
        };
        match b.find_hub(id) {
            Some(hub) => {
                b.emit_hf(&label, id, hub, ai_node);
            }
            None => {
                let n = b.emit(id, ai_node, ctx);
                b.nodes[n as usize].name = label;
            }
        }
    }
    let ai_ctx = Ctx {
        category: Category::AiModels,
        safety: Safety::YourCall,
        rebuildable: false,
        leftover: None,
    };
    let mut groups: Vec<(String, u32)> = Vec::new();
    for f in found {
        let group = match groups.iter().find(|(g, _)| *g == f.group) {
            Some(&(_, id)) => id,
            None => {
                let id = b.virtual_node(
                    &f.group,
                    &f.group_path,
                    Some(ai_node),
                    Category::AiModels,
                    NodeKind::Dir,
                    Safety::YourCall,
                    "AI models found in this folder.",
                );
                groups.push((f.group.clone(), id));
                id
            }
        };
        match f.item {
            FoundItem::Hub(id) => {
                b.emit_hf("Hugging Face cache", id, id, group);
            }
            FoundItem::Ollama(id) => {
                let n = b.emit(id, group, ai_ctx);
                b.nodes[n as usize].name = "Ollama models".into();
            }
            FoundItem::Weights(id) => {
                let n = b.emit(id, group, ai_ctx);
                let parent = b.tree.name(b.tree.node(id).parent).to_string();
                let file = b.tree.name(id).to_string();
                b.nodes[n as usize].name = format!("{parent} · {file}");
                b.nodes[n as usize].kind = NodeKind::Model;
            }
        }
    }

    // Developer.
    let dev_node = b.virtual_node(
        "Developer",
        &format!("{home_display}/Library/Developer"),
        Some(root),
        Category::Developer,
        NodeKind::Dir,
        Safety::YourCall,
        "Toolchains, package caches, build products and simulators.",
    );
    for (label, id) in dev {
        let rebuildable = is_rebuildable_dev(&label);
        let safety = if rebuildable {
            Safety::Safe
        } else {
            Safety::YourCall
        };
        let ctx = Ctx {
            category: Category::Developer,
            safety,
            rebuildable,
            leftover: None,
        };
        let n = b.emit(id, dev_node, ctx);
        b.nodes[n as usize].name = label;
    }

    // Apps.
    let apps_node = b.virtual_node(
        "Apps",
        "/Applications",
        Some(root),
        Category::Apps,
        NodeKind::Dir,
        Safety::YourCall,
        "Apps in your Applications folders, with the caches and data they keep in your Library.",
    );
    for app in apps {
        b.emit_app(app, apps_node);
    }

    // Library & caches (the Junk tile): ~/Library and the Trash.
    let junk = b.virtual_node(
        "Library & caches",
        &format!("{home_display}/Library"),
        Some(root),
        Category::Junk,
        NodeKind::Dir,
        Safety::YourCall,
        "Caches, logs and app data in your Library, and the Trash.",
    );
    if let Some(t) = trash {
        let ctx = Ctx {
            category: Category::Junk,
            safety: Safety::Safe,
            rebuildable: false,
            leftover: None,
        };
        let n = b.emit(t, junk, ctx);
        b.nodes[n as usize].name = "Trash".into();
        b.nodes[n as usize].why = "Files you already moved to the Trash.".into();
    }
    if let Some(l) = library {
        let ctx = Ctx {
            category: Category::Junk,
            safety: Safety::YourCall,
            rebuildable: false,
            leftover: None,
        };
        b.emit_children(l, junk, ctx);
    }

    // System: the sealed OS volume, shared system folders, and space no folder accounts for.
    let system = b.virtual_node(
        "System",
        "/System",
        Some(root),
        Category::System,
        NodeKind::Dir,
        Safety::Keep,
        "macOS and the files it needs. Protected.",
    );
    let os = b.virtual_node(
        "macOS",
        "/System",
        Some(system),
        Category::System,
        NodeKind::File,
        Safety::Keep,
        "The read-only system volume. macOS manages it; it can't be cleaned.",
    );
    b.nodes[os as usize].size = input.volumes.os_used;
    let users = tree.find(&format!(
        "{}/Users",
        input.display_prefix.trim_end_matches('/')
    ));
    let applications = tree.find(&format!(
        "{}/Applications",
        input.display_prefix.trim_end_matches('/')
    ));
    let volumes_dir = tree.find(&format!(
        "{}/Volumes",
        input.display_prefix.trim_end_matches('/')
    ));
    let sys_ctx = Ctx {
        category: Category::System,
        safety: Safety::Keep,
        rebuildable: false,
        leftover: None,
    };
    let top: Vec<u32> = tree.children(tree.root()).collect();
    for &c in &top {
        if Some(c) == users
            || Some(c) == applications
            || Some(c) == volumes_dir
            || b.carved.contains(&c)
            || b.size(c) == 0
        {
            continue;
        }
        b.emit(c, system, sys_ctx);
    }

    // Other: other people's homes, Shared, leftovers in /Applications and /Users.
    let other = b.virtual_node(
        "Other",
        "/Users",
        Some(root),
        Category::Other,
        NodeKind::Dir,
        Safety::YourCall,
        "Other users' folders and items that fit no other category.",
    );
    let other_ctx = Ctx {
        category: Category::Other,
        safety: Safety::YourCall,
        rebuildable: false,
        leftover: None,
    };
    for parent in [users, applications, volumes_dir].into_iter().flatten() {
        let kids: Vec<u32> = tree.children(parent).collect();
        // What's left once the home folder and apps are claimed (sizes exclude claimed parts).
        for c in kids {
            if !b.carved.contains(&c) && b.size(c) > 0 {
                b.emit(c, other, other_ctx);
            }
        }
        if b.tree.node(parent).small_size > 0 {
            b.smaller_items(
                other,
                b.tree.node(parent).small_size,
                b.tree.node(parent).small_count as u64,
                other_ctx,
                b.tree.node(parent).last_used_day,
            );
        }
    }

    // Sizes up the virtual levels, then "System data": used space nothing above accounts for.
    let scanned = tree.node(tree.root()).size;
    let unaccounted = used
        .saturating_sub(input.volumes.os_used)
        .saturating_sub(scanned);
    let unreadable = tree.stats.unreadable_dirs;
    let why = format!(
        "Space in use that no folder accounts for: local Time Machine snapshots, swap and sleep images, other system volumes, and {} folder{} Wrasse couldn't read.",
        unreadable,
        if unreadable == 1 { "" } else { "s" }
    );
    let sd = b.virtual_node(
        "System data",
        "/",
        Some(system),
        Category::System,
        NodeKind::File,
        Safety::Keep,
        &why,
    );
    b.nodes[sd as usize].size = unaccounted;

    b.finish(root);
    // Drop empty or near-empty categories so the home map only shows what matters.
    for cat in [files, ai_node, dev_node, apps_node, junk, other] {
        if b.nodes[cat as usize].size < MIN_ITEM {
            let size = b.nodes[cat as usize].size;
            b.nodes[root as usize].size -= size;
            b.nodes[system as usize].size += size;
            b.nodes[sd as usize].size += size;
            b.nodes[root as usize].children.retain(|&c| c != cat);
            // Detached and emptied, so nothing (a lens, a search) can open it.
            let mut stack = vec![cat];
            while let Some(n) = stack.pop() {
                b.nodes[n as usize].size = 0;
                stack.extend(b.nodes[n as usize].children.iter().copied());
            }
        }
    }
    let home_size = home_id.map_or(0, |h| tree.node(h).size);
    // Why this total can differ from Finder: iCloud-only items and APFS clones (PLAN.md Phase 1).
    let s = tree.stats;
    let mut why = String::from("Your documents, downloads, photos and projects. Wrasse never removes these without asking.");
    if s.dataless_items > 0 {
        why.push_str(&format!(
            " {} item{} are in iCloud only and take no space on this Mac, so they count as 0.",
            s.dataless_items,
            if s.dataless_items == 1 { "" } else { "s" }
        ));
    }
    let home_clones = home_id.map_or(0, |h| tree.node(h).clone_shared);
    if home_clones >= 100_000_000 {
        why.push_str(&format!(
            " {:.1} GB of these files share space with copies (APFS clones); like `du`, Wrasse counts each copy, so Finder may show less.",
            home_clones as f64 / 1e9
        ));
    }
    b.nodes[files as usize].why = why;

    let stats = tree.stats;
    let leaf_count = b.nodes.iter().filter(|n| n.children.is_empty()).count() as u32;
    UiDisk {
        root,
        capacity: input.volumes.capacity,
        ghost_id: -1,
        leaf_count,
        summary: Summary {
            capacity: input.volumes.capacity,
            available: input.volumes.available,
            os_used: input.volumes.os_used,
            scanned,
            unaccounted,
            home: home_size,
            files: stats.files,
            dirs: stats.dirs,
            unreadable_dirs: stats.unreadable_dirs,
            dataless_items: stats.dataless_items,
            hardlinks_deduped: stats.hardlinks_deduped,
            clone_shared_bytes: stats.clone_shared_bytes,
            cancelled: stats.cancelled,
            incremental: false,
            changed_folders: 0,
        },
        nodes: b.nodes,
    }
}

/// Model folders (PLAN.md §6.6), honoring `OLLAMA_MODELS`, `HF_HUB_CACHE` and `HF_HOME`.
fn ai_sources(home: &str, env: &HashMap<String, String>) -> Vec<(String, String)> {
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
        (
            "Jan".into(),
            format!("{home}/Library/Application Support/Jan"),
        ),
        ("Jan".into(), format!("{home}/jan")),
        (
            "GPT4All".into(),
            format!("{home}/Library/Application Support/nomic.ai/GPT4All"),
        ),
    ]
}

/// Known developer folders: toolchains, package caches, build products, simulators.
fn dev_sources(home: &str) -> Vec<(String, String)> {
    let h = |p: &str| format!("{home}/{p}");
    vec![
        ("Xcode & simulators".into(), h("Library/Developer")),
        ("Android SDK".into(), h("Library/Android")),
        ("Docker".into(), h("Library/Containers/com.docker.docker")),
        ("Homebrew".into(), "/opt/homebrew".into()),
        ("Homebrew".into(), "/usr/local/Homebrew".into()),
        ("npm cache".into(), h(".npm")),
        ("pnpm store".into(), h("Library/pnpm")),
        ("Yarn cache".into(), h("Library/Caches/Yarn")),
        ("Bun".into(), h(".bun")),
        ("Cargo".into(), h(".cargo")),
        ("Rust toolchains".into(), h(".rustup")),
        ("Go".into(), h("go")),
        ("Gradle".into(), h(".gradle")),
        ("Maven".into(), h(".m2")),
        ("CocoaPods".into(), h(".cocoapods")),
        ("CocoaPods cache".into(), h("Library/Caches/CocoaPods")),
        ("pip cache".into(), h("Library/Caches/pip")),
        ("pyenv".into(), h(".pyenv")),
        ("Conda".into(), h(".conda")),
        ("Miniconda".into(), h("miniconda3")),
        ("Anaconda".into(), h("anaconda3")),
        ("Docker CLI".into(), h(".docker")),
        ("Android".into(), h(".android")),
        ("Gems".into(), h(".gem")),
        ("VS Code extensions".into(), h(".vscode/extensions")),
    ]
}

fn is_rebuildable_dev(label: &str) -> bool {
    matches!(
        label,
        "npm cache" | "pnpm store" | "Yarn cache" | "CocoaPods cache" | "pip cache" | "Gradle"
    )
}

/// Folders whose names say "this is a cache".
fn is_cache_name(name: &str) -> bool {
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
fn is_protected_library(name: &str) -> bool {
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
fn is_read_only_library(name: &str) -> bool {
    name.ends_with(".photoslibrary")
        || name.ends_with(".musiclibrary")
        || name.ends_with(".tvlibrary")
        || name.ends_with(".fcpbundle")
        || name.ends_with(".logicx")
}

/// Model weights worth listing on their own: common weight formats from 50 MB, and generic
/// `.bin` files only when they're named like weights or are large.
fn is_weights_file(name: &str, size: u64) -> bool {
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
    if weights.iter().any(|e| n.ends_with(e)) {
        return size >= 50_000_000;
    }
    n.ends_with(".bin")
        && (size >= MODEL_FILE
            || ((n.starts_with("pytorch_model") || n.starts_with("model")) && size >= 50_000_000))
}

/// Groups found models by project: the first folder under home that isn't a generic container
/// (Documents, Desktop, …), e.g. `~/Documents/vyakta/env/hf_cache/hub` → "vyakta".
fn model_group(display: &str, home: &str) -> (String, String) {
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

fn is_model_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".gguf")
        || n.ends_with(".safetensors")
        || n.ends_with(".bin")
        || n.ends_with(".mlpackage")
}

struct App {
    /// The `.app` bundle in the arena.
    bundle: u32,
    name: String,
    leftovers: Vec<(Leftover, &'static str, u32)>,
    /// A plain folder of apps (e.g. "Adobe Creative Cloud"): (folder, apps inside).
    folder: Option<(u32, Vec<App>)>,
}

impl Builder<'_> {
    fn display(&self, path: &str) -> String {
        let p = path
            .strip_prefix(self.input.display_prefix.trim_end_matches('/'))
            .unwrap_or(path);
        if p.is_empty() {
            "/".into()
        } else {
            p.to_string()
        }
    }

    fn arena_path(&self, display: &str) -> String {
        format!(
            "{}{}",
            self.input.display_prefix.trim_end_matches('/'),
            display
        )
    }

    /// Size of an arena node minus anything carved out from under it.
    fn size(&self, id: u32) -> u64 {
        self.tree
            .node(id)
            .size
            .saturating_sub(self.carved_below.get(&id).copied().unwrap_or(0))
    }

    /// Takes an arena node out of its normal place so it can be shown elsewhere.
    fn claim(&mut self, display: &str) -> Option<u32> {
        if display.is_empty() {
            return None;
        }
        let id = self.tree.find(&self.arena_path(display))?;
        self.claim_id(id)
    }

    fn claim_id(&mut self, id: u32) -> Option<u32> {
        if self.carved.contains(&id) || self.inside_carved(id) {
            return None;
        }
        let size = self.size(id);
        self.carved.insert(id);
        let mut p = self.tree.node(id).parent;
        while p != super::tree::NONE {
            *self.carved_below.entry(p).or_insert(0) += size;
            p = self.tree.node(p).parent;
        }
        Some(id)
    }

    fn inside_carved(&self, id: u32) -> bool {
        let mut p = self.tree.node(id).parent;
        while p != super::tree::NONE {
            if self.carved.contains(&p) {
                return true;
            }
            p = self.tree.node(p).parent;
        }
        false
    }

    /// A Hugging Face hub: a folder holding `models--org--name` folders.
    fn is_hf_hub(&self, id: u32) -> bool {
        self.tree
            .children(id)
            .any(|c| self.tree.name(c).starts_with("models--"))
    }

    /// The hub inside a folder: the folder itself, or its `hub/` child (`~/.cache/huggingface`).
    fn find_hub(&self, id: u32) -> Option<u32> {
        if self.is_hf_hub(id) {
            return Some(id);
        }
        self.tree
            .children(id)
            .find(|&c| self.tree.name(c) == "hub" && self.is_hf_hub(c))
    }

    /// Walks the scan for model data outside the known tool folders and claims it for AI Models.
    fn discover_models(&mut self, home: &str) -> Vec<Found> {
        let mut hits: Vec<(u32, FoundItem)> = Vec::new();
        let mut stack = vec![self.tree.root()];
        while let Some(id) = stack.pop() {
            if self.carved.contains(&id) {
                continue;
            }
            let node = self.tree.node(id);
            let name = self.tree.name(id);
            if node.kind == Kind::File {
                if is_weights_file(name, node.size) {
                    hits.push((id, FoundItem::Weights(id)));
                }
                continue;
            }
            if name.ends_with(".app") {
                continue;
            }
            if self.is_hf_hub(id) {
                hits.push((id, FoundItem::Hub(id)));
                continue;
            }
            let has = |n: &str| self.tree.children(id).any(|c| self.tree.name(c) == n);
            if name == "models" && has("blobs") && has("manifests") {
                hits.push((id, FoundItem::Ollama(id)));
                continue;
            }
            stack.extend(self.tree.children(id));
        }
        let mut out = Vec::new();
        for (id, item) in hits {
            let display = self.display(&self.tree.path(id));
            if self.claim_id(id).is_none() {
                continue;
            }
            let (group, group_path) = model_group(&display, home);
            out.push(Found {
                group,
                group_path,
                item,
            });
        }
        out.sort_by(|a, b| a.group.cmp(&b.group));
        out
    }

    /// A Hugging Face cache as one tile per model, sized by the blobs its snapshots link to.
    fn emit_hf(&mut self, label: &str, src: u32, hub: u32, parent: u32) -> u32 {
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let last = self
            .input
            .today
            .saturating_sub(self.tree.node(src).last_used_day);
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models downloaded from Hugging Face.",
        );
        let models = (self.input.hf_models)(Path::new(&self.tree.path(hub)));
        let mut left = size;
        for m in models {
            let bytes = m.bytes.min(left);
            if bytes < MIN_ITEM {
                continue;
            }
            left -= bytes;
            let name = m.repo.rsplit('/').next().unwrap_or(&m.repo).to_string();
            let mp = self.display(&m.dir.to_string_lossy());
            let id = self.virtual_node(
                &name,
                &mp,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &format!(
                    "{} from Hugging Face. Downloads again if a project needs it.",
                    m.repo
                ),
            );
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = last;
        }
        // Leftover bytes (configs, partial downloads) stay with the cache, as one tile when big
        // enough to matter, otherwise on the cache tile itself.
        if left >= MIN_ITEM {
            let id = self.virtual_node(
                "Other cache files",
                &path,
                Some(node),
                Category::AiModels,
                NodeKind::File,
                Safety::YourCall,
                "Small models, partial downloads and other files in this cache.",
            );
            self.nodes[id as usize].size = left;
            self.nodes[id as usize].last_opened_days = last;
        } else if left > 0 && self.nodes[node as usize].children.is_empty() {
            self.nodes[node as usize].size = left;
        } else if left > 0 {
            let biggest = self.nodes[node as usize].children[0];
            self.nodes[biggest as usize].size += left;
        }
        node
    }

    fn claim_all(&mut self, sources: &[(String, String)]) -> Vec<(String, u32)> {
        sources
            .iter()
            .filter_map(|(label, path)| {
                self.claim(path)
                    .filter(|&id| self.tree.node(id).size > 0)
                    .map(|id| (label.clone(), id))
            })
            .collect()
    }

    /// Apps in /Applications and ~/Applications, with leftovers claimed from ~/Library.
    fn claim_apps(&mut self, home: &str) -> Vec<App> {
        let mut out = Vec::new();
        for dir in ["/Applications".to_string(), format!("{home}/Applications")] {
            if let Some(id) = self.tree.find(&self.arena_path(&dir)) {
                out.extend(self.apps_in(id, home, 0));
            }
        }
        out
    }

    fn apps_in(&mut self, dir: u32, home: &str, depth: u32) -> Vec<App> {
        let kids: Vec<u32> = self.tree.children(dir).collect();
        let mut out = Vec::new();
        for c in kids {
            let name = self.tree.name(c).to_string();
            if self.tree.node(c).kind != Kind::Dir {
                continue;
            }
            if let Some(stem) = name.strip_suffix(".app") {
                let display = self.display(&self.tree.path(c));
                let Some(bundle) = self.claim(&display) else {
                    continue;
                };
                let id = (self.input.bundle_id)(Path::new(&self.tree.path(c)));
                let leftovers = self.claim_leftovers(home, stem, id.as_deref());
                out.push(App {
                    bundle,
                    name: stem.to_string(),
                    leftovers,
                    folder: None,
                });
            } else if depth == 0 {
                let display = self.display(&self.tree.path(c));
                let inner = self.apps_in(c, home, depth + 1);
                if !inner.is_empty() {
                    if let Some(folder) = self.claim(&display) {
                        out.push(App {
                            bundle: folder,
                            name,
                            leftovers: Vec::new(),
                            folder: Some((folder, inner)),
                        });
                    }
                }
            }
        }
        out
    }

    fn claim_leftovers(
        &mut self,
        home: &str,
        name: &str,
        id: Option<&str>,
    ) -> Vec<(Leftover, &'static str, u32)> {
        let lib = format!("{home}/Library");
        let mut candidates: Vec<(Leftover, &'static str, String)> = Vec::new();
        if let Some(id) = id {
            candidates.push((Leftover::Caches, "Caches", format!("{lib}/Caches/{id}")));
            candidates.push((
                Leftover::Support,
                "Application Support",
                format!("{lib}/Application Support/{id}"),
            ));
            candidates.push((
                Leftover::Containers,
                "Containers",
                format!("{lib}/Containers/{id}"),
            ));
            candidates.push((Leftover::Logs, "Logs", format!("{lib}/Logs/{id}")));
            candidates.push((
                Leftover::Preferences,
                "Preferences",
                format!("{lib}/Preferences/{id}.plist"),
            ));
            candidates.push((
                Leftover::SavedState,
                "Saved Application State",
                format!("{lib}/Saved Application State/{id}.savedState"),
            ));
        }
        candidates.push((
            Leftover::Support,
            "Application Support",
            format!("{lib}/Application Support/{name}"),
        ));
        candidates.push((Leftover::Logs, "Logs", format!("{lib}/Logs/{name}")));
        candidates.push((Leftover::Caches, "Caches", format!("{lib}/Caches/{name}")));
        let mut out = Vec::new();
        for (kind, label, path) in candidates {
            if out.iter().any(|(k, _, _)| *k == kind) {
                continue;
            }
            if let Some(nid) = self.claim(&path) {
                out.push((kind, label, nid));
            }
        }
        out
    }

    fn emit_app(&mut self, app: App, parent: u32) {
        if let Some((folder, inner)) = app.folder {
            let ctx = Ctx {
                category: Category::Apps,
                safety: Safety::YourCall,
                rebuildable: false,
                leftover: None,
            };
            let path = self.display(&self.tree.path(folder));
            let f = self.virtual_node(
                &app.name,
                &path,
                Some(parent),
                Category::Apps,
                NodeKind::Dir,
                ctx.safety,
                "A folder of apps.",
            );
            for a in inner {
                self.emit_app(a, f);
            }
            return;
        }
        let bundle_path = self.tree.path(app.bundle);
        let display = self.display(&bundle_path);
        let running = self
            .input
            .running
            .iter()
            .any(|p| p.starts_with(&bundle_path));
        let last_day = self.tree.node(app.bundle).last_used_day;
        let a = self.virtual_node(
            &app.name,
            &display,
            Some(parent),
            Category::Apps,
            NodeKind::App,
            Safety::YourCall,
            "An app and the data it keeps in your Library.",
        );
        self.nodes[a as usize].running = running;
        self.nodes[a as usize].last_opened_days = self.input.today.saturating_sub(last_day);
        // The bundle itself: one item, not a folder to open.
        let bctx = Ctx {
            category: Category::Apps,
            safety: Safety::YourCall,
            rebuildable: false,
            leftover: None,
        };
        let b = self.leaf_from(app.bundle, a, bctx, NodeKind::Dir);
        self.nodes[b as usize].app_bundle = true;
        self.nodes[b as usize].why = format!("The {} app itself.", app.name);
        for (kind, label, id) in app.leftovers {
            let safe = matches!(
                kind,
                Leftover::Caches | Leftover::Logs | Leftover::SavedState
            );
            let ctx = Ctx {
                category: Category::Apps,
                safety: if safe { Safety::Safe } else { Safety::YourCall },
                rebuildable: kind == Leftover::Caches,
                leftover: Some(kind),
            };
            let n = self.leaf_from(id, a, ctx, NodeKind::Dir);
            self.nodes[n as usize].name = label.into();
            self.nodes[n as usize].why = leftover_why(kind, &app.name);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn virtual_node(
        &mut self,
        name: &str,
        path: &str,
        parent: Option<u32>,
        category: Category,
        kind: NodeKind,
        safety: Safety,
        why: &str,
    ) -> u32 {
        let id = self.nodes.len() as u32;
        let depth = parent.map_or(0, |p| self.nodes[p as usize].depth + 1);
        self.nodes.push(UiNode {
            id,
            name: name.into(),
            path: if path.is_empty() {
                "/".into()
            } else {
                path.into()
            },
            size: 0,
            parent: parent.map_or(-1, i64::from),
            children: Vec::new(),
            depth,
            category,
            kind,
            last_opened_days: 0,
            safety,
            why: why.into(),
            lenses: 0,
            deep: 0,
            match_bytes: MatchBytes::default(),
            grew_by: 0,
            shade: 0,
            duplicate_of: None,
            read_only: false,
            rebuildable: false,
            running: false,
            app_bundle: false,
            leftover: None,
            note: None,
        });
        if let Some(p) = parent {
            self.nodes[p as usize].children.push(id);
        }
        id
    }

    /// One arena node as a single item (no children shown).
    fn leaf_from(&mut self, aid: u32, parent: u32, ctx: Ctx, kind: NodeKind) -> u32 {
        let n = self.emit_node(aid, parent, ctx, kind);
        self.nodes[n as usize].size = self.size(aid);
        n
    }

    fn emit_node(&mut self, aid: u32, parent: u32, ctx: Ctx, kind: NodeKind) -> u32 {
        let name = self.tree.name(aid).to_string();
        let path = self.display(&self.tree.path(aid));
        let node = self.tree.node(aid);
        let (safety, why) = item_safety(&name, node.flags, ctx);
        let id = self.virtual_node(&name, &path, Some(parent), ctx.category, kind, safety, &why);
        let n = &mut self.nodes[id as usize];
        n.last_opened_days = self.input.today.saturating_sub(node.last_used_day);
        n.rebuildable = ctx.rebuildable || is_cache_name(&name);
        n.leftover = ctx.leftover;
        n.read_only = is_read_only_library(&name);
        if n.read_only {
            n.safety = Safety::Keep;
            n.why = "Managed by its app. Shown for size; clean it from inside the app.".into();
        }
        n.note = note_for(node.flags, node.clone_shared);
        id
    }

    /// An arena node and (pruned) everything under it.
    fn emit(&mut self, aid: u32, parent: u32, ctx: Ctx) -> u32 {
        let node = self.tree.node(aid);
        let name = self.tree.name(aid).to_string();
        let opaque = node.kind == Kind::File
            || name.ends_with(".app")
            || is_read_only_library(&name)
            || node.flags & (flag::UNREADABLE | flag::DATALESS | flag::MOUNT) != 0;
        let kind = if node.kind == Kind::File {
            if is_model_file(&name)
                && (node.size >= MODEL_FILE || ctx.category == Category::AiModels)
            {
                NodeKind::Model
            } else {
                NodeKind::File
            }
        } else {
            NodeKind::Dir
        };
        let id = self.emit_node(aid, parent, ctx, kind);
        if opaque {
            self.nodes[id as usize].size = self.size(aid);
            return id;
        }
        // Children inherit "cache" safety from a cache folder.
        let mut child_ctx = ctx;
        if is_cache_name(&name)
            || (ctx.category == Category::Junk && matches!(name.as_str(), "Logs" | "Caches"))
        {
            child_ctx.safety = Safety::Safe;
            child_ctx.rebuildable = true;
        }
        if ctx.category == Category::Junk && is_protected_library(&name) {
            child_ctx.safety = Safety::Keep;
        }
        self.emit_children(aid, id, child_ctx);
        id
    }

    /// The (pruned) children of an arena node under a UI node.
    fn emit_children(&mut self, aid: u32, ui: u32, ctx: Ctx) {
        let mut kids: Vec<(u64, u32)> = self
            .tree
            .children(aid)
            .filter(|c| !self.carved.contains(c))
            .map(|c| (self.size(c), c))
            .filter(|(s, _)| *s > 0)
            .collect();
        kids.sort_by_key(|k| std::cmp::Reverse(k.0));
        let node = self.tree.node(aid);
        let mut rest = node.small_size;
        let mut rest_count = node.small_count as u64;
        for (i, (size, c)) in kids.iter().enumerate() {
            if *size >= MIN_ITEM && i < MAX_CHILDREN {
                self.emit(*c, ui, ctx);
            } else {
                rest += size;
                rest_count += self.tree.node(*c).items.max(1);
            }
        }
        if rest > 0 {
            self.smaller_items(ui, rest, rest_count, ctx, node.last_used_day);
        }
    }

    fn smaller_items(&mut self, parent: u32, size: u64, count: u64, ctx: Ctx, last_day: u32) {
        let path = self.nodes[parent as usize].path.clone();
        let why = format!(
            "{count} smaller item{} in this folder, together.",
            if count == 1 { "" } else { "s" }
        );
        let safety = if ctx.safety == Safety::Safe {
            Safety::Safe
        } else {
            Safety::Keep
        };
        let id = self.virtual_node(
            "Smaller items",
            &path,
            Some(parent),
            ctx.category,
            NodeKind::File,
            safety,
            &why,
        );
        let n = &mut self.nodes[id as usize];
        n.size = size;
        n.last_opened_days = self.input.today.saturating_sub(last_day);
        n.rebuildable = ctx.rebuildable;
    }

    /// Sizes, lens bits and lens bytes, bottom-up; "what grew" against the previous scan.
    fn finish(&mut self, id: u32) {
        let kids = self.nodes[id as usize].children.clone();
        for &k in &kids {
            self.finish(k);
        }
        // Largest first, like the arena.
        let mut sorted = kids.clone();
        sorted.sort_by(|a, b| {
            self.nodes[*b as usize]
                .size
                .cmp(&self.nodes[*a as usize].size)
        });
        let mut size = if kids.is_empty() {
            self.nodes[id as usize].size
        } else {
            0
        };
        let mut mb = MatchBytes::default();
        let mut deep = 0;
        for &k in &kids {
            let c = &self.nodes[k as usize];
            size += c.size;
            mb.add(&c.match_bytes);
            deep |= c.deep;
        }
        let previous = self
            .input
            .previous
            .as_ref()
            .and_then(|p| p.get(&self.nodes[id as usize].path).copied());
        let has_previous = self.input.previous.is_some();
        let n = &mut self.nodes[id as usize];
        n.children = sorted;
        n.size = size;
        n.grew_by = match previous {
            Some(prev) => size as i64 - prev as i64,
            None if has_previous && n.kind != NodeKind::Volume => size as i64,
            None => 0,
        };
        if kids.is_empty() {
            let mut bits = 0;
            match n.category {
                Category::Junk if n.safety == Safety::Safe => {
                    bits |= lens::JUNK;
                    mb.junk = size;
                }
                Category::Apps => {
                    bits |= lens::APPS;
                    mb.apps = size;
                }
                Category::AiModels => {
                    bits |= lens::AI_MODELS;
                    mb.ai_models = size;
                }
                Category::Developer => {
                    bits |= lens::DEV;
                    mb.dev = size;
                }
                _ => {}
            }
            if n.kind != NodeKind::Volume
                && size >= 1_000_000_000
                && n.last_opened_days >= 180
                && n.safety != Safety::Keep
            {
                bits |= lens::LARGE_OLD;
                mb.large_old = size;
            }
            if n.grew_by >= 50_000_000 {
                bits |= lens::GREW;
                mb.grew = n.grew_by as u64;
            }
            n.lenses = bits;
            deep = bits;
        }
        n.deep = deep | n.lenses;
        n.match_bytes = mb;
        // A folder's last use is its most recent child's.
        if !kids.is_empty() {
            let recent = kids
                .iter()
                .map(|&k| self.nodes[k as usize].last_opened_days)
                .min()
                .unwrap_or(0);
            self.nodes[id as usize].last_opened_days = recent;
        }
    }
}

fn item_safety(name: &str, flags: u8, ctx: Ctx) -> (Safety, String) {
    if flags & flag::UNREADABLE != 0 {
        return (
            Safety::Keep,
            "Wrasse couldn't read this folder. Grant Full Disk Access to see inside it.".into(),
        );
    }
    if flags & flag::DATALESS != 0 {
        return (
            Safety::Keep,
            "In iCloud only. It takes no space on this Mac.".into(),
        );
    }
    if flags & flag::MOUNT != 0 {
        return (
            Safety::Keep,
            "Another disk is mounted here. It isn't part of this scan.".into(),
        );
    }
    let why = match ctx.category {
        Category::Files => "Your file. Wrasse never removes it without asking.",
        Category::AiModels => "Local AI model data.",
        Category::Developer if ctx.rebuildable => "Rebuilt automatically by its tool when needed.",
        Category::Developer => "Developer tools and data.",
        Category::Apps => "App data.",
        Category::System => "Used by macOS. Protected.",
        Category::Junk if ctx.safety == Safety::Safe || is_cache_name(name) => {
            "Cache or log files. Apps recreate them when needed."
        }
        Category::Junk if is_protected_library(name) || ctx.safety == Safety::Keep => {
            "Personal data apps keep in your Library. Protected."
        }
        Category::Junk => "App data in your Library.",
        Category::Other => "Outside your home folder.",
    };
    let safety = if is_cache_name(name) && ctx.category != Category::System {
        Safety::Safe
    } else if ctx.category == Category::Junk && is_protected_library(name) {
        Safety::Keep
    } else {
        ctx.safety
    };
    (safety, why.into())
}

fn leftover_why(kind: Leftover, app: &str) -> String {
    match kind {
        Leftover::Caches => format!("{app}'s caches. It rebuilds them when needed."),
        Leftover::Support => format!("Data {app} keeps: settings, downloads, libraries."),
        Leftover::Containers => format!("{app}'s sandbox: its documents and data."),
        Leftover::Logs => format!("{app}'s logs."),
        Leftover::Preferences => format!("{app}'s settings."),
        Leftover::SavedState => format!("Windows {app} restores at launch."),
    }
}

fn note_for(flags: u8, clone_shared: u64) -> Option<String> {
    if flags & flag::DATALESS != 0 {
        Some("In iCloud only".into())
    } else if flags & flag::UNREADABLE != 0 {
        Some("Couldn't read".into())
    } else if clone_shared > 0 {
        Some("Shares space with clones".into())
    } else {
        None
    }
}

/// Folder sizes by path, for "what grew" next time.
pub fn sizes_by_path(disk: &UiDisk) -> HashMap<String, u64> {
    disk.nodes
        .iter()
        .filter(|n| n.kind != NodeKind::File || n.name != "Smaller items")
        .map(|n| (n.path.clone(), n.size))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{scan, ScanControl};
    use std::fs;
    use std::io::Write;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn categories_add_up_and_each_byte_is_shown_once() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-breakdown-{nanos}"));
        let home = root.join("Users/me");
        write(&home.join("Documents/report.pdf"), 30_000_000);
        write(&home.join(".ollama/models/blobs/sha256-1"), 40_000_000);
        write(
            &home.join("Library/Developer/Xcode/DerivedData/a.o"),
            20_000_000,
        );
        write(
            &home.join("Library/Caches/com.example.App/blob"),
            15_000_000,
        );
        write(&home.join("Library/Caches/other/blob"), 12_000_000);
        write(
            &root.join("Applications/Example.app/Contents/MacOS/Example"),
            25_000_000,
        );
        write(&root.join("Library/Fonts/big.ttc"), 11_000_000);

        let tree = scan(&root, &ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let scanned = tree.node(0).size;
        let input = Input {
            tree: &tree,
            display_prefix: &prefix,
            home: Some(PathBuf::from("/Users/me")),
            volumes: Volumes {
                capacity: scanned * 4,
                available: scanned * 2,
                os_used: scanned / 2,
            },
            volume_name: "Test HD".into(),
            previous: None,
            running: vec![],
            today: 20_000,
            env: HashMap::new(),
            hf_models: &|_: &Path| Vec::new(),
            bundle_id: &|p: &Path| {
                p.ends_with("Example.app")
                    .then(|| "com.example.App".to_string())
            },
        };
        let disk = build(&input);
        let n = |name: &str| {
            disk.nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("no {name}"))
        };

        // The root is everything in use: scanned + macOS + System data.
        assert_eq!(disk.nodes[0].size, scanned * 2);
        assert_eq!(n("System data").size, scanned * 2 - scanned / 2 - scanned);
        // Each category holds its folders, and nothing is counted twice.
        assert!(n("AI Models").size >= 40_000_000);
        assert!(n("Developer").size >= 20_000_000);
        let app = disk.nodes.iter().find(|n| n.kind == NodeKind::App).unwrap();
        assert_eq!(app.name, "Example");
        let caches = disk
            .nodes
            .iter()
            .find(|n| n.leftover == Some(Leftover::Caches))
            .unwrap();
        assert_eq!(caches.parent, i64::from(app.id));
        assert!(
            n("Your Files").size >= 30_000_000 && n("Your Files").size < 31_000_000 + 1_000_000
        );
        let cats: u64 = disk.nodes[0]
            .children
            .iter()
            .map(|&c| disk.nodes[c as usize].size)
            .sum();
        assert_eq!(cats, disk.nodes[0].size);
        // Lens bytes: AI models and dev match their lens.
        assert!(disk.nodes[0].match_bytes.ai_models >= 40_000_000);
        assert!(disk.nodes[0].match_bytes.dev >= 20_000_000);
        assert!(disk.nodes[0].match_bytes.junk >= 12_000_000);
        crate::platform::remove_test_dir(&root);
    }

    #[test]
    fn models_are_found_anywhere_and_grouped_by_project() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-models-{nanos}"));
        let proj = root.join("Users/me/Documents/vyakta");
        write(&proj.join("env/hf_cache/hub/blobs/w1"), 60_000_000);
        let snap = proj.join("env/hf_cache/hub/models--mlx-community--Mistral-7B/snapshots/rev");
        fs::create_dir_all(&snap).unwrap();
        std::os::unix::fs::symlink("../../../blobs/w1", snap.join("model.safetensors")).unwrap();
        write(
            &proj.join("models/checkpoint-1/adapter_model.safetensors"),
            55_000_000,
        );
        write(&proj.join("notes/readme.bin"), 2_000_000);
        write(&root.join("Users/me/Documents/report.pdf"), 30_000_000);

        let tree = scan(&root, &ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let scanned = tree.node(0).size;
        let input = Input {
            tree: &tree,
            display_prefix: &prefix,
            home: Some(PathBuf::from("/Users/me")),
            volumes: Volumes {
                capacity: scanned * 4,
                available: scanned * 2,
                os_used: 0,
            },
            volume_name: "Test HD".into(),
            previous: None,
            running: vec![],
            today: 20_000,
            env: HashMap::new(),
            bundle_id: &|_: &Path| None,
            hf_models: &crate::scan::hf::hf_models,
        };
        let disk = build(&input);
        let n = |name: &str| {
            disk.nodes
                .iter()
                .find(|n| n.name == name && n.size > 0)
                .unwrap_or_else(|| panic!("no {name}"))
        };
        let ai = n("AI Models");
        let group = n("vyakta");
        assert_eq!(group.parent, i64::from(ai.id));
        let mistral = n("Mistral-7B");
        assert_eq!(mistral.kind, NodeKind::Model);
        assert!(mistral.size >= 60_000_000);
        let ckpt = n("checkpoint-1 · adapter_model.safetensors");
        assert_eq!(ckpt.kind, NodeKind::Model);
        // Claimed bytes leave Your Files, and nothing is counted twice.
        assert!(n("Your Files").size < 30_000_000 + 5_000_000);
        let cats: u64 = disk.nodes[0]
            .children
            .iter()
            .map(|&c| disk.nodes[c as usize].size)
            .sum();
        assert_eq!(cats, disk.nodes[0].size);
        assert!(disk.nodes[0].match_bytes.ai_models >= 115_000_000);
        crate::platform::remove_test_dir(&root);
    }
}
