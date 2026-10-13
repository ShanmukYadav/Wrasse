//! The shapes sent to the UI (`Disk` in `app/src/api/types.ts`) and the scan's input.

use crate::platform::Volumes;
use crate::scan::tree::Tree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Items smaller than this fold into "Smaller items".
pub(crate) const MIN_ITEM: u64 = 10_000_000;
/// At most this many items are listed under one folder.
pub(crate) const MAX_CHILDREN: usize = 60;
/// Loose model files at least this large count as models (PLAN.md §6.6).
pub(super) const MODEL_FILE: u64 = 500_000_000;

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

/// For an app node: where it came from and how it can be removed (docs/history/phase3-plan.md §2–3).
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
    pub(crate) const JUNK: u32 = 1;
    pub(crate) const APPS: u32 = 2;
    pub(crate) const AI_MODELS: u32 = 4;
    pub const DEV: u32 = 8;
    pub(crate) const LARGE_OLD: u32 = 16;
    pub(crate) const GREW: u32 = 32;
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
    pub(super) fn add(&mut self, o: &MatchBytes) {
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
    pub hf_models: &'a dyn Fn(&Path) -> Vec<crate::scan::hf::HfModel>,
}
