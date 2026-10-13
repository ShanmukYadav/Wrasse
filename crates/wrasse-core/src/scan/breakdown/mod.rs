//! Turns a raw scan into what the UI shows: a volume split into categories (Your Files,
//! AI Models, Developer, Apps, Library & caches, System, Other), each a tree of folders and
//! large items. This is the `Disk` shape in `app/src/api/types.ts`.
//!
//! The tree sent to the UI is pruned: under each folder, items smaller than [`MIN_ITEM`] (and the
//! folder's small files) are folded into one "Smaller items" entry, and at most [`MAX_CHILDREN`]
//! items are listed. The full tree stays in the core.

mod app_nodes;
mod dev;
mod models;

use super::tree::{flag, Kind, Tree};

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

mod sources;
mod types;
pub(crate) use sources::project_hf_cache;
use sources::*;
pub use types::*;

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
pub(crate) fn sizes_by_path(disk: &UiDisk) -> HashMap<String, u64> {
    disk.nodes
        .iter()
        .filter(|n| n.kind != NodeKind::File || n.name != "Smaller items")
        .map(|n| (n.path.clone(), n.size))
        .collect()
}

#[cfg(test)]
mod tests;
