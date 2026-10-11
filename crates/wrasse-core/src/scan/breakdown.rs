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

/// What kind of app leftover a node is (Caches, Containers, …).
pub use crate::apps::LeftoverKind as Leftover;

/// For an app node: where it came from and how it can be removed (docs/phase3-plan.md §2–3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMeta {
    pub source: crate::apps::Source,
    pub owner: crate::apps::Owner,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    /// Spotlight has no last-used date and it has been installed over 30 days.
    #[serde(default)]
    pub never_opened: bool,
    /// Spotlight's last-used date is known (otherwise `last_opened_days` is the newest file date).
    #[serde(default)]
    pub last_used_known: bool,
    pub route: crate::apps::uninstall::Route,
    /// Leftovers in the system Library: shown as "Needs admin", not removable by Wrasse.
    #[serde(default)]
    pub admin_leftovers: Vec<String>,
}

/// For an app's leftover: how sure Wrasse is it belongs to the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverMeta {
    pub matched_by: crate::apps::MatchedBy,
    #[serde(default)]
    pub shared_with: Vec<String>,
    /// Not ticked by default in the uninstall sheet (matched by name only, or shared).
    #[serde(default)]
    pub default_off: bool,
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
    /// The age came from file dates (the newer of access and modification), not from a known
    /// last-used date. Free Up Space starts a file or model unticked only when this is set and
    /// the age is over a year.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub approx: bool,
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub leftover_meta: Option<LeftoverMeta>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub app: Option<AppMeta>,
    /// Data left by an app that's no longer installed.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub orphan: bool,
    /// "Smaller items": many small files shown together. Its path is the folder they're in, so
    /// it can never be removed as one item (that would remove the whole folder).
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub aggregate: bool,
    /// Labels beyond the UI's type: iCloud-only, unreadable, shares blocks with clones.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
    /// The junk rule that covers this item ("Why is this safe?" shows its explanation).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rule_id: Option<String>,
    /// That rule's root: removal checks the item stays inside it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rule_root: Option<String>,
    /// Cleaned through the OS rather than item by item (the Trash, snapshots, a tool).
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub delegate: bool,
    /// Which OS action this node runs: `emptyTrash`, `thinSnapshots`, `brewCleanup`,
    /// `dockerPrune`, `deleteUnavailableSimulators`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegate_action: Option<String>,
    /// Developer item: `modules`, `target`, `venv`, `conda`, `cache`, `archive`, `keep`, `delegate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_kind: Option<String>,
    /// Set on a model: which tool, how it's quantized, and how a removal has to work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<crate::llm::ModelMeta>,
}

impl UiNode {
    /// A node with the flags the scanner fills in later left unset.
    #[allow(clippy::too_many_arguments)]
    pub fn plain(
        id: u32,
        name: impl Into<String>,
        path: impl Into<String>,
        parent: i64,
        depth: u32,
        category: Category,
        kind: NodeKind,
        safety: Safety,
        why: impl Into<String>,
    ) -> Self {
        let path = path.into();
        Self {
            id,
            name: name.into(),
            path: if path.is_empty() { "/".into() } else { path },
            size: 0,
            parent,
            children: Vec::new(),
            depth,
            category,
            kind,
            last_opened_days: 0,
            approx: false,
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
            leftover_meta: None,
            app: None,
            orphan: false,
            aggregate: false,
            note: None,
            rule_id: None,
            rule_root: None,
            delegate: false,
            delegate_action: None,
            dev_kind: None,
            model: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDisk {
    pub nodes: Vec<UiNode>,
    pub root: u32,
    pub capacity: u64,
    /// The "held by snapshots" node, or -1 when no local snapshot is holding space.
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
    /// Installed apps (`platform::installed_apps`); injected so tests need no real apps.
    pub apps: &'a [crate::apps::App],
    /// Data folders whose app is gone (`platform::orphaned_leftovers`).
    pub orphans: &'a [crate::apps::Orphan],
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
    let mut sources = ai_sources(&home_display, &input.env);
    if let Some(home) = input.home.as_ref() {
        let arena = PathBuf::from(format!(
            "{}{}",
            input.display_prefix.trim_end_matches('/'),
            home.display()
        ));
        if let Some(dir) = crate::llm::lmstudio_download_dir(&arena) {
            let display = strip_prefix(input.display_prefix, &dir);
            if !sources.iter().any(|(_, p)| overlaps(p, &display)) {
                sources.push(("LM Studio".into(), display));
            }
        }
    }
    let ai = b.claim_all(&sources);
    // Models anywhere else on the Mac (a project's own cache, downloaded weights).
    let found = b.discover_models(&home_display);
    let dev = b.claim_all(&dev_sources(&home_display));
    // Project folders (stale node_modules, Cargo target, Python envs) before the home
    // folder is claimed, so they can move into Developer. The Wrasse checkout is skipped.
    let projects = b.discover_projects();
    let apps = b.claim_apps();
    let orphans = b.claim_orphans();
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
                let src_path = b.display(&b.tree.path(id));
                b.emit_hf(&label, id, hub, ai_node, project_hf_cache(&src_path));
            }
            None if b.emit_ollama(&label, id, ai_node) => {}
            None if b.emit_tool_models(&label, id, ai_node) => {}
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
                let why = if f.group == "Project models" {
                    "Models in your projects. Your choice, and never selected for you."
                } else {
                    "AI models found in this folder."
                };
                let id = b.virtual_node(
                    &f.group,
                    &f.group_path,
                    Some(ai_node),
                    Category::AiModels,
                    NodeKind::Dir,
                    Safety::YourCall,
                    why,
                );
                groups.push((f.group.clone(), id));
                id
            }
        };
        match f.item {
            FoundItem::Hub(id) => {
                let hub_path = b.display(&b.tree.path(id));
                let project = f.group == "Project models" || project_hf_cache(&hub_path);
                let label = if project {
                    "Hugging Face files"
                } else {
                    "Hugging Face cache"
                };
                b.emit_hf(label, id, id, group, project);
            }
            FoundItem::Ollama(id) => {
                if !b.emit_ollama("Ollama models", id, group) {
                    let n = b.emit(id, group, ai_ctx);
                    b.nodes[n as usize].name = "Ollama models".into();
                }
            }
            FoundItem::Weights(id) => {
                let n = b.emit(id, group, ai_ctx);
                let parent = b.tree.name(b.tree.node(id).parent).to_string();
                let file = b.tree.name(id).to_string();
                b.nodes[n as usize].name = format!("{parent} · {file}");
                b.nodes[n as usize].kind = NodeKind::Model;
                b.attach_loose(n, id);
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
    for (id, kind, project) in projects {
        let ctx = Ctx {
            category: Category::Developer,
            safety: Safety::YourCall,
            rebuildable: false,
            leftover: None,
        };
        let n = b.emit(id, dev_node, ctx);
        b.nodes[n as usize].name = crate::devjunk::project_label(kind, &project);
        b.nodes[n as usize].dev_kind = Some(crate::devjunk::project_kind(kind).into());
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
    let mut folders: Vec<(String, u32)> = Vec::new();
    for app in apps {
        let parent = match app
            .info
            .folder
            .as_ref()
            .map(|f| f.to_string_lossy().into_owned())
        {
            None => apps_node,
            Some(f) => match folders.iter().find(|(p, _)| *p == f) {
                Some(&(_, id)) => id,
                None => {
                    let name = Path::new(&f)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let id = b.virtual_node(
                        &name,
                        &f,
                        Some(apps_node),
                        Category::Apps,
                        NodeKind::Dir,
                        Safety::YourCall,
                        "A folder of apps.",
                    );
                    folders.push((f, id));
                    id
                }
            },
        };
        b.emit_app(app, parent);
    }
    b.emit_orphans(orphans, apps_node);

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
        b.nodes[n as usize].delegate = true;
        b.nodes[n as usize].delegate_action = Some("emptyTrash".into());
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

    b.attach_dupes();
    b.finish(root);
    // Ages are final (a folder's age is its most recent child). Policy then decides
    // what Free Up Space may offer. The Wrasse checkout stays Keep wherever it sits.
    b.apply_dev_policy(dev_node);
    b.protect_wrasse();
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
        ("Jan".into(), jan_support(home)),
        ("Jan".into(), format!("{home}/jan")),
        ("GPT4All".into(), gpt4all_support(home)),
    ]
}

fn jan_support(home: &str) -> String {
    if cfg!(windows) {
        format!("{home}/AppData/Roaming/Jan")
    } else {
        format!("{home}/Library/Application Support/Jan")
    }
}

fn gpt4all_support(home: &str) -> String {
    if cfg!(windows) {
        format!("{home}/AppData/Local/nomic.ai/GPT4All")
    } else {
        format!("{home}/Library/Application Support/nomic.ai/GPT4All")
    }
}

fn strip_prefix(prefix: &str, path: &Path) -> String {
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

fn overlaps(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

fn tool_of(label: &str) -> crate::llm::ModelTool {
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
fn first_weight(dir: &Path) -> Option<PathBuf> {
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

fn removal_of(tool: crate::llm::ModelTool) -> crate::llm::ModelRemoval {
    match tool {
        crate::llm::ModelTool::Ollama => crate::llm::ModelRemoval::Ollama,
        crate::llm::ModelTool::HuggingFace => crate::llm::ModelRemoval::HuggingFace,
        _ => crate::llm::ModelRemoval::Quarantine,
    }
}

/// True when `dir` has an entry named `name`. Does not open the file.
fn has_dir_entry(dir: &str, name: &str) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .is_some_and(|rd| rd.flatten().any(|e| e.file_name() == name))
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

fn is_rebuildable_dev(label: &str) -> bool {
    matches!(
        label,
        "npm cache" | "pnpm store" | "Yarn cache" | "CocoaPods cache" | "pip cache"
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

/// Model weights worth listing on their own. The extension and size only decide whether to look;
/// [`super::weights::is_model_weights`] has to see a real header or a sibling `config.json`
/// with `model_type` or `architectures`.
fn is_weights_file(path: &str, name: &str, size: u64) -> bool {
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
    sized && super::weights::is_model_weights(Path::new(path))
}

/// Documents, Desktop, and iCloud Drive. Models here are project files, shown together.
fn project_place(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    parts.iter().any(|s| matches!(*s, "Documents" | "Desktop"))
        || p.contains("/Library/Mobile Documents/")
        || p.contains("/com~apple~CloudDocs/")
}

/// A Hugging Face cache that lives in a project (for example `Documents/vyakta/env/hf_cache`),
/// not the account-wide cache under `~/.cache`. It is project data.
pub fn project_hf_cache(path: &str) -> bool {
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
fn spotlight_matches_birth(path: &Path, secs: i64) -> bool {
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

struct App {
    /// The `.app` bundle in the arena.
    bundle: u32,
    info: crate::apps::App,
    /// Leftovers found in the scan: (arena node, what discovery knows about it).
    leftovers: Vec<(u32, crate::apps::Leftover)>,
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
                if is_weights_file(&self.tree.path(id), name, node.size) {
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
    /// `project` is a cache inside Documents, Desktop, or iCloud: project files, not junk.
    fn emit_hf(&mut self, label: &str, src: u32, hub: u32, parent: u32, project: bool) -> u32 {
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
        let mut accounted = 0u64;
        for m in models {
            let shares = m.bytes == 0 && m.logical_bytes >= MIN_ITEM;
            let bytes = if shares {
                m.logical_bytes
            } else {
                m.bytes.min(size.saturating_sub(accounted))
            };
            if !shares && bytes < MIN_ITEM {
                continue;
            }
            if !shares {
                accounted += bytes;
            }
            let name = m.repo.rsplit('/').next().unwrap_or(&m.repo).to_string();
            let mp = self.display(&m.dir.to_string_lossy());
            let why = if let Some(other) = &m.shares_with {
                format!("{name} from Hugging Face. Weights are already counted with {other}.")
            } else if project {
                format!("{name} is part of this project. It stays until you choose it.")
            } else {
                format!("{name} from Hugging Face. Downloads again if a project needs it.")
            };
            let id = self.virtual_node(
                &name,
                &mp,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &why,
            );
            let weight = first_weight(&m.dir);
            let quant = crate::llm::quant_of(&m.repo, weight.as_deref())
                .or_else(|| crate::llm::quant_from_config(&m.dir));
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = last;
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool: crate::llm::ModelTool::HuggingFace,
                quantization: quant,
                removal: crate::llm::ModelRemoval::HuggingFace,
                ollama_name: None,
                models_dir: None,
                weight: weight.as_ref().map(|p| self.display(&p.to_string_lossy())),
                partner_weight: None,
                dupe_level: None,
                content_hash: weight
                    .as_ref()
                    .and_then(|p| crate::llm::blob_content_hash(p)),
                orphan_blob: false,
                clone_copy: false,
                shares_weights: shares,
                shares_with: m.shares_with.clone(),
            });
            if let Some(w) = &weight {
                self.refine_used(id, w);
                self.mark_clone(id, w);
            } else {
                self.refine_used(id, &m.dir);
            }
        }
        self.fold_remainder(node, size, accounted, last);
        node
    }

    /// Ollama, one row per manifest plus orphaned blobs. `false` when this folder isn't a store.
    fn emit_ollama(&mut self, label: &str, src: u32, parent: u32) -> bool {
        let arena = PathBuf::from(self.tree.path(src));
        let Some(store) = crate::llm::Store::open(&arena) else {
            return false;
        };
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models downloaded with Ollama.",
        );
        let mut accounted = 0u64;
        let dir_display = self.display(&store.dir.to_string_lossy());
        for m in &store.models {
            let shares = m.shares_weights && m.logical >= MIN_ITEM;
            let own = !m.shares_weights && m.exclusive >= MIN_ITEM;
            if !shares && !own {
                continue;
            }
            let bytes = if shares { m.logical } else { m.exclusive };
            let weight = m
                .weight
                .as_ref()
                .map(|p| self.display(&p.to_string_lossy()));
            let manifest = m
                .manifest
                .as_ref()
                .map(|p| self.display(&p.to_string_lossy()));
            let node_path = manifest
                .clone()
                .or(weight.clone())
                .unwrap_or_else(|| path.clone());
            let why = if m.orphan {
                "An Ollama blob no manifest uses.".to_string()
            } else if let Some(other) = &m.shares_with {
                format!("Weights are already counted with {other}.")
            } else {
                format!("{} in Ollama.", m.name)
            };
            let id = self.virtual_node(
                &m.name,
                &node_path,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &why,
            );
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = self.input.today.saturating_sub(m.last_day);
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool: crate::llm::ModelTool::Ollama,
                quantization: m.quantization.clone(),
                removal: if m.orphan {
                    crate::llm::ModelRemoval::Quarantine
                } else {
                    crate::llm::ModelRemoval::Ollama
                },
                ollama_name: (!m.orphan).then(|| m.name.clone()),
                models_dir: Some(dir_display.clone()),
                weight: weight.clone(),
                partner_weight: None,
                dupe_level: None,
                content_hash: m.content_hash.clone(),
                orphan_blob: m.orphan,
                clone_copy: false,
                shares_weights: shares,
                shares_with: m.shares_with.clone(),
            });
            let stamp = m.weight.clone().or_else(|| m.manifest.clone());
            if let Some(p) = stamp {
                self.refine_used(id, &p);
                self.mark_clone(id, &p);
            }
            if !shares {
                accounted += m.exclusive;
            }
        }
        let last = self.nodes[node as usize].last_opened_days;
        self.fold_remainder(node, size, accounted, last);
        true
    }

    /// Weight files inside a known tool folder (LM Studio, Jan, GPT4All). `false` when there
    /// are none big enough to list, so the folder is shown as one item instead.
    fn emit_tool_models(&mut self, label: &str, src: u32, parent: u32) -> bool {
        let mut files = Vec::new();
        self.collect_weights(src, &mut files, 0);
        if files.is_empty() {
            return false;
        }
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models kept by this tool.",
        );
        let tool = tool_of(label);
        let removal = removal_of(tool);
        let mut accounted = 0u64;
        for aid in files {
            let bytes = self.size(aid);
            if bytes < MIN_ITEM {
                continue;
            }
            let parent_name = self.tree.name(self.tree.node(aid).parent);
            let file = self.tree.name(aid);
            let title = if parent_name == self.tree.name(src) {
                file.to_string()
            } else {
                format!("{parent_name} · {file}")
            };
            let display = self.display(&self.tree.path(aid));
            let id = self.virtual_node(
                &title,
                &display,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &format!("{title} in {label}."),
            );
            let arena = PathBuf::from(self.tree.path(aid));
            let day = self.tree.node(aid).last_used_day;
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = self.input.today.saturating_sub(day);
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool,
                quantization: crate::llm::quant_of(&title, Some(&arena)),
                removal,
                ollama_name: None,
                models_dir: None,
                weight: Some(display),
                partner_weight: None,
                dupe_level: None,
                content_hash: crate::llm::blob_content_hash(&arena),
                orphan_blob: false,
                clone_copy: false,
                shares_weights: false,
                shares_with: None,
            });
            self.refine_used(id, &arena);
            self.mark_clone(id, &arena);
            accounted += bytes;
        }
        let last = self
            .input
            .today
            .saturating_sub(self.tree.node(src).last_used_day);
        self.fold_remainder(node, size, accounted, last);
        true
    }

    fn collect_weights(&self, id: u32, out: &mut Vec<u32>, depth: u32) {
        if depth > 8 {
            return;
        }
        for c in self.tree.children(id) {
            let node = self.tree.node(c);
            let name = self.tree.name(c);
            if name.ends_with(".app") {
                continue;
            }
            if node.kind == Kind::File {
                if node.size >= MIN_ITEM
                    && super::weights::is_model_weights(Path::new(&self.tree.path(c)))
                {
                    out.push(c);
                }
            } else {
                self.collect_weights(c, out, depth + 1);
            }
        }
    }

    fn attach_loose(&mut self, ui: u32, aid: u32) {
        let arena = PathBuf::from(self.tree.path(aid));
        let name = self.nodes[ui as usize].name.clone();
        let display = self.nodes[ui as usize].path.clone();
        self.nodes[ui as usize].model = Some(crate::llm::ModelMeta {
            tool: crate::llm::ModelTool::Loose,
            quantization: crate::llm::quant_of(&name, Some(&arena)),
            removal: crate::llm::ModelRemoval::Quarantine,
            ollama_name: None,
            models_dir: None,
            weight: Some(display),
            partner_weight: None,
            dupe_level: None,
            content_hash: crate::llm::blob_content_hash(&arena),
            orphan_blob: false,
            clone_copy: false,
            shares_weights: false,
            shares_with: None,
        });
        self.refine_used(ui, &arena);
        self.mark_clone(ui, &arena);
    }

    fn refine_used(&mut self, id: u32, arena: &Path) {
        let Some(secs) = crate::platform::last_used(arena) else {
            return;
        };
        // Spotlight fills kMDItemLastUsedDate with the creation instant on items
        // that were never opened. That is not a later open, so the file-date age
        // stays approximate.
        if secs < 0 || spotlight_matches_birth(arena, secs) {
            return;
        }
        let day = (secs / 86_400) as u32;
        let n = &mut self.nodes[id as usize];
        n.last_opened_days = self.input.today.saturating_sub(day);
        n.approx = false;
    }

    fn mark_clone(&mut self, id: u32, arena: &Path) {
        if !crate::llm::is_clone_file(arena) {
            return;
        }
        if let Some(m) = self.nodes[id as usize].model.as_mut() {
            m.clone_copy = true;
        }
        self.nodes[id as usize].note = Some(crate::llm::CLONE_NOTE.into());
    }

    /// Bytes of the folder that no listed model owns: a tile when they're large, otherwise
    /// folded onto the largest model so the category total still matches the scan.
    fn fold_remainder(&mut self, node: u32, folder_size: u64, accounted: u64, last: u32) {
        let left = folder_size.saturating_sub(accounted);
        if left == 0 {
            return;
        }
        let path = self.nodes[node as usize].path.clone();
        let absorb = self.nodes[node as usize]
            .children
            .iter()
            .copied()
            .filter(|&k| {
                !self.nodes[k as usize]
                    .model
                    .as_ref()
                    .is_some_and(|m| m.shares_weights)
            })
            .max_by_key(|&k| self.nodes[k as usize].size);
        if left >= MIN_ITEM || absorb.is_none() {
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
            self.nodes[id as usize].approx = true;
        } else if let Some(biggest) = absorb {
            self.nodes[biggest as usize].size += left;
        }
    }

    /// Size, then a sample of the first and last 16 MB. Dataless files are not opened.
    fn attach_dupes(&mut self) {
        let mut idxs = Vec::new();
        let mut cands = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(weight) = n.model.as_ref().and_then(|m| m.weight.clone()) else {
                continue;
            };
            let arena = self.arena_path(&weight);
            let path = PathBuf::from(&arena);
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let dataless = crate::platform::is_dataless(&md);
            let len = if dataless {
                0
            } else if md.file_type().is_symlink() {
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            } else {
                md.len()
            };
            let hash = n.model.as_ref().and_then(|m| m.content_hash.clone());
            cands.push(crate::llm::DupeCand {
                path,
                len,
                content_hash: hash,
                dataless,
            });
            idxs.push(i);
        }
        for (k, cand) in cands.iter().enumerate() {
            if !cand.dataless {
                self.mark_clone(idxs[k] as u32, &cand.path);
            }
        }
        // Two seconds of edge reads. Longer than that, the copies stay unlabeled and the
        // scan ends; removing one still requires a full hash.
        let pairs = crate::llm::pair_duplicates_until(
            &cands,
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2)),
        );
        for (k, pair) in pairs.into_iter().enumerate() {
            let Some(pair) = pair else { continue };
            let display = self.display(&pair.partner.to_string_lossy());
            let id = idxs[k];
            if let Some(m) = self.nodes[id].model.as_mut() {
                m.partner_weight = Some(display.clone());
                m.dupe_level = Some(pair.level);
            }
            if pair.level == crate::llm::DupeLevel::Full {
                if let Some(pid) = self.nodes.iter().position(|n| {
                    n.model.as_ref().and_then(|m| m.weight.as_deref()) == Some(display.as_str())
                }) {
                    self.nodes[id].duplicate_of = Some(pid as u32);
                }
            } else if self.nodes[id].note.is_none() {
                self.nodes[id].note = Some(crate::llm::SAMPLE_NOTE.into());
            }
        }
        self.note_icloud_copies();
        for &i in &idxs {
            if self.nodes[i].model.as_ref().is_some_and(|m| m.clone_copy) {
                self.nodes[i].note = Some(crate::llm::CLONE_NOTE.into());
            }
        }
        self.note_two_formats();
    }

    /// An iCloud Drive copy that matches another model. The warning is on the synced copy.
    fn note_icloud_copies(&mut self) {
        let mut partners = Vec::new();
        let mut tagged = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(m) = &n.model else { continue };
            if m.dupe_level.is_none() && n.duplicate_of.is_none() {
                continue;
            }
            let own = m.weight.clone().unwrap_or_else(|| n.path.clone());
            let partner = m.partner_weight.clone().unwrap_or_default();
            let touches = crate::llm::icloud_synced_path(&own)
                || crate::llm::icloud_synced_path(&n.path)
                || crate::llm::icloud_synced_path(&partner);
            if !touches {
                continue;
            }
            if !partner.is_empty() {
                partners.push(partner);
            }
            if crate::llm::icloud_synced_path(&own) || crate::llm::icloud_synced_path(&n.path) {
                tagged.push(i);
            }
        }
        for (i, n) in self.nodes.iter().enumerate() {
            let own = n
                .model
                .as_ref()
                .and_then(|m| m.weight.clone())
                .unwrap_or_else(|| n.path.clone());
            let synced =
                crate::llm::icloud_synced_path(&own) || crate::llm::icloud_synced_path(&n.path);
            if synced && partners.iter().any(|p| p == &own || p == &n.path) {
                tagged.push(i);
            }
        }
        tagged.sort_unstable();
        tagged.dedup();
        for i in tagged {
            add_note(&mut self.nodes[i], crate::llm::ICLOUD_DUPE_NOTE);
        }
    }

    /// The same checkpoint saved as pytorch_model.bin and model.safetensors. Headers only.
    fn note_two_formats(&mut self) {
        let mut groups: Vec<(std::path::PathBuf, Vec<usize>)> = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(weight) = n.model.as_ref().and_then(|m| m.weight.clone()) else {
                continue;
            };
            let arena = std::path::PathBuf::from(self.arena_path(&weight));
            let Some(dir) = arena.parent().map(|p| p.to_path_buf()) else {
                continue;
            };
            if let Some((_, ids)) = groups.iter_mut().find(|(d, _)| d == &dir) {
                ids.push(i);
            } else {
                groups.push((dir, vec![i]));
            }
        }
        for (dir, ids) in groups {
            if !super::weights::same_weights_two_formats(&dir) {
                continue;
            }
            for i in ids {
                add_note(&mut self.nodes[i], crate::llm::FORMAT_NOTE);
            }
        }
    }

    /// Stale project folders and Python envs, claimed so they show under Developer.
    /// Anything inside the Wrasse checkout (it contains `.toolchain`) stays put.
    fn discover_projects(&mut self) -> Vec<(u32, crate::devjunk::ProjectItem, String)> {
        struct Frame {
            id: u32,
            in_modules: bool,
            in_wrasse: bool,
        }
        let mut stack = vec![Frame {
            id: self.tree.root(),
            in_modules: false,
            in_wrasse: false,
        }];
        let mut hits = Vec::new();
        while let Some(frame) = stack.pop() {
            if self.carved.contains(&frame.id) || self.inside_carved(frame.id) {
                continue;
            }
            let node = self.tree.node(frame.id);
            if node.kind == Kind::File {
                continue;
            }
            let name = self.tree.name(frame.id);
            let child_names: Vec<&str> = self
                .tree
                .children(frame.id)
                .map(|c| self.tree.name(c))
                .collect();
            let wrasse_here = frame.in_wrasse || crate::devjunk::is_wrasse_root(&child_names);
            let parent = node.parent;
            let parent_name = if parent == super::tree::NONE {
                ""
            } else {
                self.tree.name(parent)
            };
            let grand = if parent == super::tree::NONE {
                super::tree::NONE
            } else {
                self.tree.node(parent).parent
            };
            let grand_name = if grand == super::tree::NONE {
                ""
            } else {
                self.tree.name(grand)
            };
            let parent_children: Vec<&str> = if parent == super::tree::NONE {
                Vec::new()
            } else {
                self.tree
                    .children(parent)
                    .map(|c| self.tree.name(c))
                    .collect()
            };
            let age = self.input.today.saturating_sub(node.last_used_day);
            // Small files are not nodes in the tree. A `pyvenv.cfg` or `Cargo.toml` is
            // almost always smaller than that, so look the name up on disk.
            let mut children = child_names.clone();
            if matches!(name, "venv" | ".venv" | "env")
                && has_dir_entry(&self.tree.path(frame.id), "pyvenv.cfg")
                && !children.contains(&"pyvenv.cfg")
            {
                children.push("pyvenv.cfg");
            }
            let mut parent_kids = parent_children.clone();
            if name == "target"
                && parent != super::tree::NONE
                && has_dir_entry(&self.tree.path(parent), "Cargo.toml")
                && !parent_kids.contains(&"Cargo.toml")
            {
                parent_kids.push("Cargo.toml");
            }
            if !wrasse_here {
                if let Some(kind) = crate::devjunk::project_item(&crate::devjunk::ProjectSpot {
                    name,
                    children: &children,
                    parent_children: &parent_kids,
                    parent_name,
                    grandparent_name: grand_name,
                    age_days: age,
                    in_modules: frame.in_modules,
                    in_wrasse: false,
                }) {
                    if self.size(frame.id) >= MIN_ITEM {
                        if let Some(id) = self.claim_id(frame.id) {
                            let project = match kind {
                                crate::devjunk::ProjectItem::Conda => name.to_string(),
                                _ => {
                                    if parent_name.is_empty() {
                                        "Project".into()
                                    } else {
                                        parent_name.to_string()
                                    }
                                }
                            };
                            hits.push((id, kind, project));
                            continue;
                        }
                    }
                }
            }
            let in_modules = frame.in_modules || name == "node_modules";
            for c in self.tree.children(frame.id) {
                stack.push(Frame {
                    id: c,
                    in_modules,
                    in_wrasse: wrasse_here,
                });
            }
        }
        hits
    }

    /// Applies developer policy once folder ages include their newest child.
    fn apply_dev_policy(&mut self, dev: u32) {
        let mut stack = vec![dev];
        while let Some(id) = stack.pop() {
            let kids = self.nodes[id as usize].children.clone();
            stack.extend(kids);
            if id == dev {
                continue;
            }
            let path = self.nodes[id as usize].path.clone();
            let age = self.nodes[id as usize].last_opened_days;
            let Some(policy) = crate::devjunk::policy_for(&path, age) else {
                continue;
            };
            let n = &mut self.nodes[id as usize];
            n.safety = policy.safety;
            n.rebuildable = policy.rebuildable;
            n.read_only = policy.read_only;
            n.dev_kind = policy.dev_kind.map(str::to_string);
            n.why = policy.why;
        }
    }

    /// The Wrasse checkout and its toolchain are never offered and never removed.
    fn protect_wrasse(&mut self) {
        for n in &mut self.nodes {
            if crate::devjunk::wrasse_path(&n.path) {
                n.safety = Safety::Keep;
                n.read_only = true;
                n.rebuildable = false;
                n.why =
                    "Part of the Wrasse project, including its toolchain. Wrasse leaves it alone."
                        .into();
            }
        }
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

    /// The installed apps' bundles and leftovers, claimed so their bytes show under Apps.
    fn claim_apps(&mut self) -> Vec<App> {
        let mut out = Vec::new();
        for info in self.input.apps {
            let Some(bundle) = self.claim(&info.bundle.to_string_lossy()) else {
                continue;
            };
            let leftovers = info
                .leftovers
                .iter()
                .filter(|l| !l.needs_admin)
                .filter_map(|l| {
                    self.claim(&l.path.to_string_lossy())
                        .map(|id| (id, l.clone()))
                })
                .collect();
            out.push(App {
                bundle,
                info: info.clone(),
                leftovers,
            });
        }
        out
    }

    /// Orphaned leftovers in the scan: (orphan, its items' arena nodes).
    fn claim_orphans(&mut self) -> Vec<(crate::apps::Orphan, Vec<(u32, crate::apps::Leftover)>)> {
        let mut out = Vec::new();
        for o in self.input.orphans {
            let items: Vec<(u32, crate::apps::Leftover)> = o
                .items
                .iter()
                .filter_map(|l| {
                    self.claim(&l.path.to_string_lossy())
                        .map(|id| (id, l.clone()))
                })
                .collect();
            if !items.is_empty() {
                out.push((o.clone(), items));
            }
        }
        out
    }

    fn emit_app(&mut self, app: App, parent: u32) {
        use crate::apps::{uninstall::route, Source};
        let info = &app.info;
        let bundle_path = self.tree.path(app.bundle);
        let display = self.display(&bundle_path);
        let running = self
            .input
            .running
            .iter()
            .any(|p| p.starts_with(&bundle_path) || p.starts_with(&info.bundle));
        let system = info.source == Source::System;
        let a = self.virtual_node(
            &info.name,
            &display,
            Some(parent),
            Category::Apps,
            NodeKind::App,
            if system {
                Safety::Keep
            } else {
                Safety::YourCall
            },
            if system {
                "Part of macOS. It can't be removed."
            } else {
                "An app and the data it keeps in your Library."
            },
        );
        let last_day = self.tree.node(app.bundle).last_used_day;
        let n = &mut self.nodes[a as usize];
        n.running = running;
        n.last_opened_days = info
            .last_used_days
            .unwrap_or_else(|| self.input.today.saturating_sub(last_day));
        n.app = Some(AppMeta {
            source: info.source.clone(),
            owner: info.owner,
            bundle_id: info.id.clone(),
            version: info.version.clone(),
            never_opened: info.never_opened(),
            last_used_known: info.last_used_days.is_some(),
            route: route(info),
            admin_leftovers: info
                .leftovers
                .iter()
                .filter(|l| l.needs_admin)
                .map(|l| l.path.to_string_lossy().into_owned())
                .collect(),
        });
        // The bundle itself: one item, not a folder to open.
        let bctx = Ctx {
            category: Category::Apps,
            safety: if system {
                Safety::Keep
            } else {
                Safety::YourCall
            },
            rebuildable: false,
            leftover: None,
        };
        let last_used = self.nodes[a as usize].last_opened_days;
        let b = self.leaf_from(app.bundle, a, bctx, NodeKind::Dir);
        self.nodes[b as usize].app_bundle = true;
        // The app's parts were last used when the app was (PLAN.md §6.5: caches use their app's).
        self.nodes[b as usize].last_opened_days = last_used;
        self.nodes[b as usize].why = format!("The {} app itself.", info.name);
        if system {
            self.nodes[b as usize].safety = Safety::Keep;
        }
        for (id, l) in app.leftovers {
            let ctx = Ctx {
                category: Category::Apps,
                safety: if l.kind.regenerates() {
                    Safety::Safe
                } else {
                    Safety::YourCall
                },
                rebuildable: l.kind == Leftover::Caches,
                leftover: Some(l.kind),
            };
            let n = self.leaf_from(id, a, ctx, NodeKind::Dir);
            let node = &mut self.nodes[n as usize];
            node.name = l.kind.label().into();
            node.why = leftover_why(&l, &info.name);
            node.last_opened_days = last_used;
            node.leftover_meta = Some(LeftoverMeta {
                matched_by: l.matched_by,
                shared_with: l.shared_with.clone(),
                default_off: !l.ticked_by_default(),
            });
        }
    }

    /// "Leftovers from removed apps": one node per bundle id, its folders as items.
    fn emit_orphans(
        &mut self,
        orphans: Vec<(crate::apps::Orphan, Vec<(u32, crate::apps::Leftover)>)>,
        parent: u32,
    ) {
        if orphans.is_empty() {
            return;
        }
        let home = self
            .input
            .home
            .as_ref()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default();
        let group = self.virtual_node(
            "Leftovers from removed apps",
            &format!("{home}/Library"),
            Some(parent),
            Category::Apps,
            NodeKind::Dir,
            Safety::YourCall,
            "Data apps left behind after they were removed. No installed app uses these folders.",
        );
        self.nodes[group as usize].orphan = true;
        for (o, items) in orphans {
            let first = self.display(&self.tree.path(items[0].0));
            let node = self.virtual_node(
                &o.id,
                &first,
                Some(group),
                Category::Apps,
                NodeKind::Dir,
                Safety::YourCall,
                &format!(
                    "Left by an app with the id {}, which isn't installed on this Mac. Nothing here changed in {} days.",
                    o.id, o.untouched_days
                ),
            );
            self.nodes[node as usize].orphan = true;
            self.nodes[node as usize].last_opened_days = o.untouched_days;
            for (id, l) in items {
                let ctx = Ctx {
                    category: Category::Apps,
                    safety: Safety::YourCall,
                    rebuildable: false,
                    leftover: Some(l.kind),
                };
                let n = self.leaf_from(id, node, ctx, NodeKind::Dir);
                let item = &mut self.nodes[n as usize];
                item.orphan = true;
                item.name = l.kind.label().into();
                item.why = format!(
                    "{} of an app that's no longer installed ({}).",
                    l.kind.label(),
                    o.id
                );
                item.last_opened_days = o.untouched_days;
                item.leftover_meta = Some(LeftoverMeta {
                    matched_by: l.matched_by,
                    shared_with: Vec::new(),
                    default_off: true,
                });
            }
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
        self.nodes.push(UiNode::plain(
            id,
            name,
            path,
            parent.map_or(-1, i64::from),
            depth,
            category,
            kind,
            safety,
            why,
        ));
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
        let project_hf = project_hf_cache(&path);
        let (mut safety, mut why) = item_safety(&name, node.flags, ctx);
        if project_hf {
            safety = Safety::YourCall;
            why = "Project files. Wrasse leaves them until you choose.".into();
        }
        let id = self.virtual_node(&name, &path, Some(parent), ctx.category, kind, safety, &why);
        let n = &mut self.nodes[id as usize];
        n.last_opened_days = self.input.today.saturating_sub(node.last_used_day);
        // The tree date is the file-date fallback. Spotlight last-used is not stored per file.
        n.approx = matches!(kind, NodeKind::File | NodeKind::Model);
        n.rebuildable = !project_hf && (ctx.rebuildable || is_cache_name(&name));
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
        let file_kind = node.kind;
        let file_size = node.size;
        let opaque = file_kind == Kind::File
            || name.ends_with(".app")
            || is_read_only_library(&name)
            || node.flags & (flag::UNREADABLE | flag::DATALESS | flag::MOUNT) != 0;
        // Extension is not enough: a zero-filled .bin is a file, not a model.
        let kind = if file_kind == Kind::File {
            if (file_size >= MODEL_FILE || ctx.category == Category::AiModels)
                && super::weights::is_model_weights(Path::new(&self.tree.path(aid)))
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
        // Children inherit "cache" safety from a cache folder. A project's own Hugging Face
        // cache is the project's data, so it does not inherit that.
        let mut child_ctx = ctx;
        let display = self.display(&self.tree.path(aid));
        if !project_hf_cache(&display)
            && (is_cache_name(&name)
                || (ctx.category == Category::Junk && matches!(name.as_str(), "Logs" | "Caches")))
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
        n.approx = true;
        n.rebuildable = ctx.rebuildable;
        n.aggregate = true;
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
            // A model whose weights are already counted on another row stays visible, but its
            // bytes are not added again.
            let shared = c.model.as_ref().is_some_and(|m| m.shares_weights);
            if !shared {
                size += c.size;
                mb.add(&c.match_bytes);
            }
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

fn add_note(n: &mut UiNode, text: &str) {
    if n.note.as_deref() == Some(crate::llm::CLONE_NOTE) {
        return;
    }
    match &mut n.note {
        None => n.note = Some(text.to_string()),
        Some(s) if s.contains(text) => {}
        Some(s) => {
            s.push(' ');
            s.push_str(text);
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

fn leftover_why(l: &crate::apps::Leftover, app: &str) -> String {
    let what = match l.kind {
        Leftover::Caches => format!("{app}'s caches. It rebuilds them when needed."),
        Leftover::Support => format!("Data {app} keeps: settings, downloads, libraries."),
        Leftover::Containers => format!("{app}'s sandbox: its documents and data."),
        Leftover::GroupContainers => format!("Data {app} shares with its helpers or extensions."),
        Leftover::Logs => format!("{app}'s logs."),
        Leftover::Preferences => format!("{app}'s settings."),
        Leftover::SavedState => format!("Windows {app} restores at launch."),
        Leftover::HttpStorages => format!("Cookies and web data {app} stored."),
        Leftover::WebKit => format!("Web content {app} cached."),
        Leftover::LaunchAgents => format!("Starts part of {app} when you log in."),
        Leftover::LaunchDaemons | Leftover::HelperTools => {
            format!("A background helper {app} installed for all users.")
        }
    };
    if !l.shared_with.is_empty() {
        format!(
            "{what} Also matches {}, so check before removing it.",
            l.shared_with.join(", ")
        )
    } else if l.matched_by == crate::apps::MatchedBy::Name {
        format!("{what} Matched by the app's name, so check it's really {app}'s.")
    } else {
        what
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

    /// A real safetensors header, then padding, so the file is both valid and large.
    fn write_safetensors(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let header = br#"{"__metadata__":{"format":"pt"}}"#;
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&(header.len() as u64).to_le_bytes()).unwrap();
        f.write_all(header).unwrap();
        let used = 8 + header.len();
        assert!(bytes > used);
        f.write_all(&vec![0u8; bytes - used]).unwrap();
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
        write(&home.join("Library/Caches/other/tiny-1"), 5_000);
        write(&home.join("Library/Caches/other/tiny-2"), 5_000);
        write(
            &root.join("Applications/Example.app/Contents/MacOS/Example"),
            25_000_000,
        );
        write(&root.join("Library/Fonts/big.ttc"), 11_000_000);

        let tree = scan(&root, &ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let scanned = tree.node(0).size;
        let apps = [crate::apps::App {
            id: Some("com.example.App".into()),
            name: "Example".into(),
            bundle: PathBuf::from("/Applications/Example.app"),
            version: None,
            folder: None,
            source: crate::apps::Source::Folder,
            owner: crate::apps::Owner::User,
            last_used_days: Some(12),
            added_days: Some(100),
            leftovers: vec![crate::apps::Leftover {
                path: PathBuf::from("/Users/me/Library/Caches/com.example.App"),
                kind: Leftover::Caches,
                matched_by: crate::apps::MatchedBy::BundleId,
                shared_with: vec![],
                needs_admin: false,
            }],
        }];
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
            apps: &apps,
            orphans: &[],
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
        assert_eq!(app.last_opened_days, 12, "from Spotlight");
        // "Smaller items" carries its folder's path, so it must be marked as an aggregate (the UI
        // and the ranker never remove it as one item).
        let smaller: Vec<&UiNode> = disk
            .nodes
            .iter()
            .filter(|n| n.name == "Smaller items")
            .collect();
        assert!(!smaller.is_empty());
        for n in smaller {
            assert!(n.aggregate);
            assert_eq!(n.path, disk.nodes[n.parent as usize].path);
        }
        assert_eq!(
            app.app.as_ref().unwrap().route,
            crate::apps::uninstall::Route::Tray
        );
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

    /// Stale project folders move into Developer. The Wrasse checkout does not.
    /// A Python env in Documents is shown and is not pre-selected.
    #[test]
    fn developer_junk_is_claimed_and_the_wrasse_checkout_is_not() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-devjunk-{nanos}"));
        let home = root.join("Users/me");
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let times = fs::FileTimes::new().set_accessed(old).set_modified(old);
        let age = |path: &Path| {
            fn rec(p: &Path, times: fs::FileTimes) {
                let meta = fs::symlink_metadata(p).unwrap();
                if meta.is_dir() {
                    for entry in fs::read_dir(p).unwrap() {
                        rec(&entry.unwrap().path(), times);
                    }
                }
                fs::File::options()
                    .read(true)
                    .open(p)
                    .unwrap()
                    .set_times(times)
                    .unwrap();
            }
            rec(path, times);
        };

        write(
            &home.join("Projects/web/node_modules/pkg/index.js"),
            12_000_000,
        );
        write(&home.join("Projects/web/package.json"), 20);
        age(&home.join("Projects/web/node_modules"));
        write(
            &home.join("Projects/fresh/node_modules/pkg/index.js"),
            12_000_000,
        );
        write(&home.join("Documents/Wrasse/.toolchain/rust/README"), 20);
        write(
            &home.join("Documents/Wrasse/target/debug/wrasse"),
            12_000_000,
        );
        write(&home.join("Documents/Wrasse/Cargo.toml"), 20);
        age(&home.join("Documents/Wrasse"));
        write(&home.join("Documents/app/.venv/pyvenv.cfg"), 30);
        write(&home.join("Documents/app/.venv/lib/python"), 12_000_000);
        age(&home.join("Documents/app/.venv"));
        write(&home.join("code/api/Cargo.toml"), 20);
        write(&home.join("code/api/target/debug/api"), 12_000_000);
        age(&home.join("code/api/target"));
        write(
            &home.join("Library/Containers/com.docker.docker/Data/vms/disk"),
            12_000_000,
        );

        let tree = scan(&root, &ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let scanned = tree.node(0).size;
        let input = Input {
            tree: &tree,
            display_prefix: &prefix,
            home: Some(PathBuf::from("/Users/me")),
            volumes: Volumes {
                capacity: scanned * 4,
                available: scanned,
                os_used: 0,
            },
            volume_name: "Test HD".into(),
            previous: None,
            running: vec![],
            today: 20_000,
            env: HashMap::new(),
            apps: &[],
            orphans: &[],
            hf_models: &|_: &Path| Vec::new(),
        };
        let disk = build(&input);
        let find = |name: &str| disk.nodes.iter().find(|n| n.name == name && n.size > 0);
        let modules = find("web · node_modules").expect("stale node_modules");
        assert_eq!(modules.category, Category::Developer);
        assert_eq!(modules.dev_kind.as_deref(), Some("modules"));
        assert!(modules.rebuildable);
        assert_eq!(modules.safety, Safety::Safe);
        assert!(find("fresh · node_modules").is_none());
        let fresh = disk
            .nodes
            .iter()
            .find(|n| n.path.ends_with("/Projects/fresh/node_modules"))
            .expect("recent node_modules stays in the project");
        assert_ne!(fresh.category, Category::Developer);

        let wrasse = disk
            .nodes
            .iter()
            .find(|n| n.path.contains("/Documents/Wrasse/target"))
            .expect("wrasse target stays visible");
        assert_ne!(wrasse.category, Category::Developer);
        assert!(wrasse.read_only && wrasse.safety == Safety::Keep);

        let venv = find("app · Python").expect("documents venv");
        assert_eq!(venv.dev_kind.as_deref(), Some("venv"));
        assert_eq!(venv.safety, Safety::YourCall);
        assert!(!venv.rebuildable);

        let target = find("api · target").expect("cargo target");
        assert!(target.rebuildable && target.safety == Safety::Safe);
        assert_eq!(target.dev_kind.as_deref(), Some("target"));

        let docker = disk
            .nodes
            .iter()
            .find(|n| n.path.contains("com.docker.docker") && n.name == "Docker")
            .expect("docker");
        assert!(docker.read_only && docker.safety == Safety::Keep);

        let limits = crate::rank::Limits {
            hand_pick_only: vec![PathBuf::from("/Users/me/Documents")],
            ..Default::default()
        };
        let (offered, _, _) = crate::rank::candidates(&disk, &limits);
        let offered_paths: Vec<&str> = offered
            .iter()
            .map(|c| disk.nodes[c.id as usize].path.as_str())
            .collect();
        assert!(offered_paths
            .iter()
            .any(|p| p.ends_with("/Projects/web/node_modules")));
        assert!(offered_paths
            .iter()
            .any(|p| p.ends_with("/code/api/target")));
        assert!(offered_paths.iter().all(|p| !p.contains("/Documents/")));
        assert!(offered_paths.iter().all(|p| !p.contains("/Wrasse")));
        assert!(offered_paths
            .iter()
            .all(|p| !p.contains("com.docker.docker")));
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
        write_safetensors(
            &proj.join("models/checkpoint-1/adapter_model.safetensors"),
            55_000_000,
        );
        // Named like a weight file and large. A config without model keys does not make it one.
        write(&proj.join("notes/pytorch_model.bin"), 50_000_000);
        fs::write(proj.join("notes/config.json"), br#"{"name":"not-a-model"}"#).unwrap();
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
            apps: &[],
            orphans: &[],
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
        // Your Files also has a "vyakta" folder now (the zero-filled bin lives there).
        let group = disk
            .nodes
            .iter()
            .find(|n| n.name == "Project models" && n.category == Category::AiModels && n.size > 0)
            .expect("project model group");
        assert_eq!(group.parent, i64::from(ai.id));
        assert_eq!(group.safety, Safety::YourCall);
        let mistral = n("Mistral-7B");
        assert_eq!(mistral.kind, NodeKind::Model);
        assert_eq!(mistral.safety, Safety::YourCall);
        assert!(!mistral.rebuildable);
        assert!(
            mistral.why.contains("part of this project"),
            "a project cache is project data: {}",
            mistral.why
        );
        assert!(!mistral.why.contains("Downloads again"));
        assert!(disk
            .nodes
            .iter()
            .filter(|n| n.path.contains("hf_cache"))
            .all(|n| {
                n.category != Category::Junk && n.safety != Safety::Safe && !n.rebuildable
            }));
        assert!(
            mistral.approx,
            "a model dated from file dates is approximate"
        );
        assert!(!ai.approx, "a category folder is not an approximate date");
        assert!(mistral.size >= 60_000_000);
        let ckpt = n("checkpoint-1 · adapter_model.safetensors");
        assert_eq!(ckpt.kind, NodeKind::Model);
        // The zero-filled .bin stays a file under Your Files. It is not an AI model.
        let stray = disk
            .nodes
            .iter()
            .find(|n| n.name.contains("pytorch_model"))
            .expect("zero-filled bin stays in the tree");
        assert_ne!(stray.kind, NodeKind::Model);
        assert_ne!(stray.category, Category::AiModels);
        assert!(stray.approx, "the file's age still comes from file dates");
        // Claimed bytes leave Your Files, and nothing is counted twice.
        assert!(n("Your Files").size >= 50_000_000 + 30_000_000);
        assert!(n("Your Files").size < 50_000_000 + 30_000_000 + 15_000_000);
        let cats: u64 = disk.nodes[0]
            .children
            .iter()
            .map(|&c| disk.nodes[c as usize].size)
            .sum();
        assert_eq!(cats, disk.nodes[0].size);
        assert!(disk.nodes[0].match_bytes.ai_models >= 115_000_000);
        assert!(disk.nodes[0].match_bytes.ai_models < 200_000_000);
        crate::platform::remove_test_dir(&root);
    }

    #[test]
    fn project_copies_in_icloud_are_duplicates_and_two_formats_are_labeled() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-project-models-{nanos}"));
        let home = root.join("Users/me");
        let local = home.join("Documents/vyakta/models/fpc/model.safetensors");
        let cloud = home.join(
            "Library/Mobile Documents/com~apple~CloudDocs/Vyakta_Backup/fpc/model.safetensors",
        );
        write_safetensors(&local, 50_000_000);
        write_safetensors(&cloud, 50_000_000);
        let gector = home.join("Documents/vyakta/models/gector");
        write_safetensors(&gector.join("model.safetensors"), 50_000_000);
        write_pytorch(&gector.join("pytorch_model.bin"), 51_000_000);
        fs::write(gector.join("config.json"), br#"{"model_type":"deberta"}"#).unwrap();

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
            apps: &[],
            orphans: &[],
            hf_models: &|_: &Path| Vec::new(),
        };
        let disk = build(&input);
        let models: Vec<&UiNode> = disk
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Model && n.category == Category::AiModels)
            .collect();
        assert!(
            models.len() >= 4,
            "local, icloud, and both formats: {models:?}"
        );
        assert!(models.iter().all(|n| n.safety == Safety::YourCall));
        let cloud_node = models
            .iter()
            .find(|n| n.path.contains("Mobile Documents"))
            .expect("icloud copy");
        assert!(
            cloud_node
                .note
                .as_deref()
                .is_some_and(|s| s.contains(crate::llm::ICLOUD_DUPE_NOTE)),
            "icloud note: {:?}",
            cloud_node.note
        );
        assert!(
            cloud_node.model.as_ref().unwrap().dupe_level.is_some()
                || cloud_node.duplicate_of.is_some()
        );
        let local_node = models
            .iter()
            .find(|n| n.path.contains("/Documents/") && n.path.ends_with("fpc/model.safetensors"))
            .expect("local copy");
        assert!(
            local_node
                .note
                .as_deref()
                .map(|s| !s.contains("iCloud"))
                .unwrap_or(true),
            "the Documents copy is not the synced warning: {:?}",
            local_node.note
        );
        let formats: Vec<&&UiNode> = models
            .iter()
            .filter(|n| n.path.contains("/gector/"))
            .collect();
        assert_eq!(formats.len(), 2, "{formats:?}");
        assert!(formats.iter().all(|n| {
            n.note
                .as_deref()
                .is_some_and(|s| s.contains(crate::llm::FORMAT_NOTE))
                && n.duplicate_of.is_none()
        }));
        let limits = crate::rank::Limits {
            hand_pick_only: vec![
                PathBuf::from("/Users/me/Documents"),
                PathBuf::from("/Users/me/Desktop"),
                PathBuf::from("/Users/me/Library/Mobile Documents"),
            ],
            ..Default::default()
        };
        let (offered, _, _) = crate::rank::candidates(&disk, &limits);
        let offered_ids: Vec<u32> = offered.iter().map(|c| c.id).collect();
        for n in models {
            assert!(!offered_ids.contains(&n.id), "{} was pre-selected", n.path);
        }
        crate::platform::remove_test_dir(&root);
    }

    fn write_pytorch(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let magic = b"PK\x03\x04data.pkl";
        let mut f = fs::File::create(path).unwrap();
        f.write_all(magic).unwrap();
        assert!(bytes > magic.len());
        f.write_all(&vec![0u8; bytes - magic.len()]).unwrap();
    }

    fn write_gguf(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&vec![0u8; bytes - 4]).unwrap();
    }

    #[test]
    fn each_tool_lists_its_models_and_shared_weights_count_once() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wrasse-tools-{nanos}"));
        let home = root.join("Users/me");
        let models = home.join(".ollama/models");
        let shared = "a".repeat(64);
        let own = "b".repeat(64);
        write(&models.join(format!("blobs/sha256-{shared}")), 10_000_000);
        write(&models.join(format!("blobs/sha256-{own}")), 10_000_000);
        write(&models.join("blobs/sha256-orphan"), 10_000_000);
        let big = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{shared}","size":1}},{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{own}","size":2}}]}}"#
        );
        let tiny = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{shared}","size":1}}]}}"#
        );
        let manifest = models.join("manifests/registry.ollama.ai/library/llama-Q4_K_M/latest");
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, big).unwrap();
        let other = models.join("manifests/registry.ollama.ai/library/tiny/latest");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, tiny).unwrap();
        write_gguf(
            &home.join("Library/Application Support/Jan/models/coder-Q5_K_M.gguf"),
            10_000_000,
        );
        write_gguf(
            &home.join("Library/Application Support/nomic.ai/GPT4All/gpt-Q8_0.gguf"),
            10_000_000,
        );
        let lms = home.join("Models");
        write_gguf(&lms.join("publisher/story-Q4_K_M.gguf"), 10_000_000);
        let settings = home.join("Library/Application Support/LM Studio/settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(
            &settings,
            format!(r#"{{"downloadsFolder":"{}"}}"#, lms.display()),
        )
        .unwrap();
        let hub = home.join(".cache/huggingface/hub");
        write(&hub.join("blobs/w"), 10_000_000);
        let repo = hub.join("models--org--plain");
        fs::create_dir_all(repo.join("snapshots/rev")).unwrap();
        std::os::unix::fs::symlink(
            "../../../blobs/w",
            repo.join("snapshots/rev/model.safetensors"),
        )
        .unwrap();
        fs::write(
            repo.join("snapshots/rev/config.json"),
            r#"{"quantization_config":{"bits":4,"quant_method":"bitsandbytes"}}"#,
        )
        .unwrap();

        let tree = scan(&root, &ScanControl::default()).unwrap();
        let prefix = root.to_string_lossy().into_owned();
        let scanned = tree.node(0).size;
        let input = Input {
            tree: &tree,
            display_prefix: &prefix,
            home: Some(PathBuf::from("/Users/me")),
            volumes: Volumes {
                capacity: scanned * 4,
                available: scanned,
                os_used: 0,
            },
            volume_name: "Test HD".into(),
            previous: None,
            running: vec![],
            today: 20_000,
            env: HashMap::new(),
            apps: &[],
            orphans: &[],
            hf_models: &crate::scan::hf::hf_models,
        };
        let disk = build(&input);
        let find = |name: &str| {
            disk.nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("no {name}"))
        };
        let llama = find("llama-Q4_K_M:latest");
        assert_eq!(llama.kind, NodeKind::Model);
        assert_eq!(
            llama.model.as_ref().unwrap().quantization.as_deref(),
            Some("Q4_K_M")
        );
        assert!(llama.size >= 20_000_000);
        assert!(!llama.model.as_ref().unwrap().shares_weights);
        let tiny = find("tiny:latest");
        assert!(tiny.model.as_ref().unwrap().shares_weights);
        assert_eq!(
            tiny.model.as_ref().unwrap().shares_with.as_deref(),
            Some("llama-Q4_K_M:latest")
        );
        let orphan = disk
            .nodes
            .iter()
            .find(|n| n.model.as_ref().is_some_and(|m| m.orphan_blob))
            .unwrap();
        assert!(orphan.size >= 10_000_000);
        let jan = find("models · coder-Q5_K_M.gguf");
        assert_eq!(jan.model.as_ref().unwrap().tool, crate::llm::ModelTool::Jan);
        assert_eq!(
            jan.model.as_ref().unwrap().quantization.as_deref(),
            Some("Q5_K_M")
        );
        let gpt = find("gpt-Q8_0.gguf");
        assert_eq!(
            gpt.model.as_ref().unwrap().tool,
            crate::llm::ModelTool::Gpt4all
        );
        let story = find("publisher · story-Q4_K_M.gguf");
        assert_eq!(
            story.model.as_ref().unwrap().tool,
            crate::llm::ModelTool::LmStudio
        );
        let hf = find("plain");
        assert_eq!(
            hf.model.as_ref().unwrap().quantization.as_deref(),
            Some("4-bit")
        );
        assert_eq!(
            hf.model.as_ref().unwrap().tool,
            crate::llm::ModelTool::HuggingFace
        );
        let ai = find("AI Models").size;
        assert!(ai >= 70_000_000, "{ai}");
        assert!(ai < 85_000_000, "shared blob counted once: {ai}");
        crate::platform::remove_test_dir(&root);
    }
}
