//! Developer junk (PLAN.md §6.8). Discovery is a read of the scan tree.
//! Homebrew, Docker and unavailable simulators are previews here; removing them
//! goes through `safety::delegate` and only when that tool is the one doing it.
//! The Wrasse project and its `.toolchain` are never offered.

use crate::scan::breakdown::{Category, NodeKind, Safety, UiDisk, UiNode};
use serde::{Deserialize, Serialize};

/// A project folder unused this long is offered. Younger ones stay visible, unticked.
pub(crate) const STALE_DAYS: u32 = 90;

/// Something inside a project worth lifting into the Developer category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectItem {
    /// `node_modules` last touched at least [`STALE_DAYS`] ago.
    Modules,
    /// A Cargo `target/` next to `Cargo.toml`, stale.
    Target,
    /// `venv`, `.venv` or `env` with a `pyvenv.cfg`.
    Venv,
    /// One environment under a conda `envs` directory.
    Conda,
}

/// How a developer node should be shown and whether Free Up Space may offer it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Policy {
    pub safety: Safety,
    pub rebuildable: bool,
    pub read_only: bool,
    pub dev_kind: Option<&'static str>,
    pub why: String,
}

/// A tool's own cleanup, measured without running it. `bytes` is already inside
/// some other folder, so attaching it does not add to the volume total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveExtra {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub why: String,
    pub action: String,
}

/// True when `names` are the children of the Wrasse checkout (it holds `.toolchain`).
pub(crate) fn is_wrasse_root(child_names: &[&str]) -> bool {
    child_names.contains(&".toolchain")
}

/// The checkout, its `.toolchain`, and everything inside either.
pub(crate) fn wrasse_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let toolchain = p.split('/').any(|c| c == ".toolchain");
    let wrasse = p.split('/').any(|c| c == "Wrasse")
        && (p.contains("/Wrasse/")
            || p.ends_with("/Wrasse")
            || p.starts_with("Wrasse/")
            || p == "Wrasse");
    toolchain || wrasse
}

/// Where a folder sits in a project, for [`project_item`].
#[derive(Copy, Clone)]
pub(crate) struct ProjectSpot<'a> {
    pub name: &'a str,
    pub children: &'a [&'a str],
    pub parent_children: &'a [&'a str],
    pub parent_name: &'a str,
    pub grandparent_name: &'a str,
    pub age_days: u32,
    /// Inside another `node_modules`. Nested copies stay with the outer one.
    pub in_modules: bool,
    /// Inside the Wrasse checkout, even before the path string is known.
    pub in_wrasse: bool,
}

/// A project item to carve out, or nothing.
pub(crate) fn project_item(spot: &ProjectSpot<'_>) -> Option<ProjectItem> {
    let ProjectSpot {
        name,
        children,
        parent_children,
        parent_name,
        grandparent_name,
        age_days,
        in_modules,
        in_wrasse,
    } = *spot;
    if in_wrasse || name == ".toolchain" {
        return None;
    }
    if !in_modules && name == "node_modules" && age_days >= STALE_DAYS {
        return Some(ProjectItem::Modules);
    }
    if name == "target" && parent_children.contains(&"Cargo.toml") && age_days >= STALE_DAYS {
        return Some(ProjectItem::Target);
    }
    if matches!(name, "venv" | ".venv" | "env") && children.contains(&"pyvenv.cfg") {
        return Some(ProjectItem::Venv);
    }
    if parent_name == "envs" && is_conda_root(grandparent_name) {
        return Some(ProjectItem::Conda);
    }
    None
}

fn is_conda_root(name: &str) -> bool {
    matches!(
        name,
        "miniconda3"
            | "anaconda3"
            | ".conda"
            | "miniforge3"
            | "mambaforge"
            | "Miniconda3"
            | "anaconda"
    )
}

pub(crate) fn project_label(kind: ProjectItem, project: &str) -> String {
    match kind {
        ProjectItem::Modules => format!("{project} · node_modules"),
        ProjectItem::Target => format!("{project} · target"),
        ProjectItem::Venv => format!("{project} · Python"),
        ProjectItem::Conda => format!("{project} · conda"),
    }
}

pub(crate) fn project_kind(kind: ProjectItem) -> &'static str {
    match kind {
        ProjectItem::Modules => "modules",
        ProjectItem::Target => "target",
        ProjectItem::Venv => "venv",
        ProjectItem::Conda => "conda",
    }
}

/// Documents, Desktop and iCloud Drive stay Your call (never pre-selected).
pub(crate) fn hand_pick(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.contains("/Documents/")
        || p.contains("/Desktop/")
        || p.ends_with("/Documents")
        || p.ends_with("/Desktop")
        || p.contains("/Library/Mobile Documents/")
        || p.contains("/com~apple~CloudDocs/")
}

/// Policy for a path already in the Developer tree. `None` leaves the node as the
/// scan built it. Age decides whether a project folder is offered; that happens
/// in the ranker via [`offer_rebuildable`].
pub(crate) fn policy_for(path: &str, age_days: u32) -> Option<Policy> {
    let p = path.replace('\\', "/");
    if wrasse_path(&p) {
        return Some(kept(
            "Part of the Wrasse project, including its toolchain. Wrasse leaves it alone.",
        ));
    }
    if p.contains("/Library/Containers/com.docker.docker")
        || p.ends_with("/.docker")
        || p.contains("/Containers/com.docker.docker")
    {
        return Some(kept(
            "Docker's own files. Wrasse cleans Docker only with docker commands, and only while Docker is running.",
        ));
    }
    if p == "/opt/homebrew"
        || p.starts_with("/opt/homebrew/")
        || p == "/usr/local/Homebrew"
        || p.starts_with("/usr/local/Homebrew/")
    {
        return Some(kept(
            "Homebrew itself stays. A cleanup preview is separate, and it runs only when you ask.",
        ));
    }
    if p.ends_with("/DerivedData") {
        return Some(cache(
            "cache",
            "Intermediate build files Xcode recreates the next time you build. Your projects aren't touched.",
        ));
    }
    if p.ends_with("/CoreSimulator/Caches") {
        return Some(cache(
            "cache",
            "Simulator caches. Xcode recreates them. Unavailable simulators are listed on their own.",
        ));
    }
    if archive_item(&p) {
        return Some(Policy {
            safety: Safety::YourCall,
            rebuildable: false,
            read_only: false,
            dev_kind: Some("archive"),
            why: "An Xcode archive. Keep it if you might resubmit or debug that build.".into(),
        });
    }
    if conda_env(&p) {
        let your_call = hand_pick(&p);
        return Some(Policy {
            safety: if your_call { Safety::YourCall } else { Safety::Safe },
            rebuildable: false,
            read_only: false,
            dev_kind: Some("conda"),
            why: format!("A conda environment, last changed {age_days} days ago. The project can recreate it."),
        });
    }
    if p.ends_with("/.venv") || p.ends_with("/venv") || p.ends_with("/env") {
        return Some(project_policy("venv", "Python environment", age_days, &p));
    }
    if let Some(why) = cache_path(&p) {
        let your_call = hand_pick(&p);
        return Some(Policy {
            safety: if your_call {
                Safety::YourCall
            } else {
                Safety::Safe
            },
            rebuildable: !your_call,
            read_only: false,
            dev_kind: Some("cache"),
            why: why.into(),
        });
    }
    if p.ends_with("/node_modules") {
        return Some(project_policy("modules", "node_modules", age_days, &p));
    }
    if p.ends_with("/target") {
        return Some(project_policy("target", "target", age_days, &p));
    }
    None
}

fn project_policy(kind: &'static str, what: &str, age_days: u32, path: &str) -> Policy {
    let stale = age_days >= STALE_DAYS;
    let your_call = hand_pick(path);
    Policy {
        safety: if your_call {
            Safety::YourCall
        } else {
            Safety::Safe
        },
        // A recent folder stays in the list and is not offered. A Documents copy is
        // never pre-selected even when it is stale (the ranker skips hand-pick paths).
        rebuildable: stale && !your_call,
        read_only: false,
        dev_kind: Some(kind),
        why: format!(
            "{what} in this project, last changed {age_days} days ago. The tool can rebuild it. Your choice."
        ),
    }
}

fn cache(kind: &'static str, why: &str) -> Policy {
    Policy {
        safety: Safety::Safe,
        rebuildable: true,
        read_only: false,
        dev_kind: Some(kind),
        why: why.into(),
    }
}

fn kept(why: &str) -> Policy {
    Policy {
        safety: Safety::Keep,
        rebuildable: false,
        read_only: true,
        dev_kind: Some("keep"),
        why: why.into(),
    }
}

fn cache_path(path: &str) -> Option<&'static str> {
    const PATHS: &[(&str, &str)] = &[
        (
            "/.cargo/registry",
            "Cargo's downloaded crates. The next build fetches them again.",
        ),
        (
            "/.cargo/git",
            "Git checkouts Cargo cloned. The next build fetches them again.",
        ),
        (
            "/.gradle/caches",
            "Gradle's downloaded dependencies. The next build fetches them again.",
        ),
        (
            "/.m2/repository",
            "Maven's downloaded dependencies. The next build fetches them again.",
        ),
        (
            "/go/pkg/mod",
            "Go modules already downloaded. The next build fetches them again.",
        ),
        (
            "/.nuget/packages",
            "NuGet packages already downloaded. The next build fetches them again.",
        ),
        (
            "/.npm",
            "npm's package cache. npm downloads it again when a project needs it.",
        ),
        (
            "/Library/pnpm",
            "pnpm's package store. pnpm downloads it again when a project needs it.",
        ),
        (
            "/pnpm/store",
            "pnpm's package store. pnpm downloads it again when a project needs it.",
        ),
        (
            "/Library/Caches/Yarn",
            "Yarn's package cache. Yarn downloads it again when a project needs it.",
        ),
        (
            "/Library/Caches/pip",
            "pip's download cache. pip downloads it again when you install.",
        ),
        (
            "/Library/Caches/CocoaPods",
            "CocoaPods' download cache. pod install fetches it again.",
        ),
    ];
    PATHS
        .iter()
        .find(|(suffix, _)| path.ends_with(suffix))
        .map(|(_, why)| *why)
}

/// `…/miniconda3/envs/ml` and the same shape under the other conda roots.
fn conda_env(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    parts
        .windows(3)
        .any(|w| is_conda_root(w[0]) && w[1] == "envs")
}

fn archive_item(path: &str) -> bool {
    let Some(rest) = path.split("/Xcode/Archives/").nth(1) else {
        return false;
    };
    !rest.is_empty() && !rest.contains('/')
}

/// Whether Free Up Space offers this developer node, and as what.
/// Rebuildable caches are offered even when recent. Project envs and archives
/// wait until they are stale. `None` means look inside the folder.
pub(crate) fn rank_why(
    rebuildable: bool,
    dev_kind: Option<&str>,
    age_days: u32,
) -> Option<RankWhy> {
    if rebuildable {
        return Some(RankWhy::Rebuilds);
    }
    if age_days >= STALE_DAYS && matches!(dev_kind, Some("venv" | "conda" | "archive")) {
        return Some(RankWhy::Unused);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RankWhy {
    Rebuilds,
    Unused,
}

/// Homebrew cleanup preview, Docker's reclaimable total, and unavailable simulators.
/// Each call is read-only. A missing tool is omitted, not an error.
pub fn live_extras(home: &std::path::Path) -> Vec<LiveExtra> {
    let mut out = Vec::new();
    if let Some(bytes) = crate::platform::brew_cleanup_preview() {
        if bytes > 0 {
            out.push(LiveExtra {
                name: "Homebrew cleanup".into(),
                path: "homebrew:cleanup".into(),
                bytes,
                why: "What `brew cleanup` would remove. Homebrew itself stays. Nothing runs until you ask."
                    .into(),
                action: "brewCleanup".into(),
            });
        }
    }
    if let Some(bytes) = crate::platform::docker_reclaimable() {
        if bytes > 0 {
            out.push(LiveExtra {
                name: "Docker reclaimable".into(),
                path: "docker:prune".into(),
                bytes,
                why: "Unused Docker images, stopped containers and build cache, from `docker system df`. Volumes stay. This runs only while Docker is running, and only through Docker."
                    .into(),
                action: "dockerPrune".into(),
            });
        }
    }
    if let Some((bytes, count)) = crate::platform::unavailable_simulators(home) {
        if count > 0 && bytes > 0 {
            out.push(LiveExtra {
                name: format!(
                    "{count} unavailable simulator{}",
                    if count == 1 { "" } else { "s" }
                ),
                path: "simctl:unavailable".into(),
                bytes,
                why: "Simulators Xcode can no longer boot. Removing them uses `xcrun simctl delete unavailable`."
                    .into(),
                action: "deleteUnavailableSimulators".into(),
            });
        }
    }
    out
}

/// Adds the live previews under Developer. Their bytes are already on disk inside
/// another folder, so the volume total is left as it is. They show in the drill-in.
pub(crate) fn attach_live(disk: &mut UiDisk, extras: &[LiveExtra]) {
    if extras.is_empty() {
        return;
    }
    let Some(dev) = disk
        .nodes
        .iter()
        .position(|n| n.name == "Developer" && n.category == Category::Developer)
    else {
        return;
    };
    let dev = dev as u32;
    let root = disk.root;
    // A category smaller than the map's minimum was folded into System data. Bring
    // the tile back for these previews. Their bytes move out of System data, so the
    // volume total does not grow, and they are not added again on top of a Developer
    // tile that is already showing.
    if !disk.nodes[root as usize].children.contains(&dev) {
        let bytes: u64 = extras.iter().map(|e| e.bytes).sum();
        let taken = take_from_system(disk, bytes);
        if taken == 0 {
            return;
        }
        disk.nodes[dev as usize].size = taken;
        disk.nodes[dev as usize].parent = i64::from(root);
        disk.nodes[root as usize].children.push(dev);
        // Root already counted these bytes inside System data. Moving them
        // onto the Developer tile leaves the volume total unchanged.
    }
    let depth = disk.nodes[dev as usize].depth + 1;
    for extra in extras {
        let id = disk.nodes.len() as u32;
        let mut node = UiNode::plain(
            id,
            extra.name.clone(),
            extra.path.clone(),
            i64::from(dev),
            depth,
            Category::Developer,
            NodeKind::Dir,
            Safety::YourCall,
            extra.why.clone(),
        );
        node.size = extra.bytes;
        node.delegate = true;
        node.delegate_action = Some(extra.action.clone());
        node.dev_kind = Some("delegate".into());
        node.last_opened_days = STALE_DAYS;
        disk.nodes.push(node);
        disk.nodes[dev as usize].children.push(id);
    }
}

/// Moves `bytes` out of System data and its ancestors, except the volume root
/// (the caller adds the tile back onto the root). Returns what it could move.
fn take_from_system(disk: &mut UiDisk, bytes: u64) -> u64 {
    let Some(sd) = disk
        .nodes
        .iter()
        .position(|n| n.name == "System data" && n.category == Category::System)
    else {
        return 0;
    };
    let take = bytes.min(disk.nodes[sd].size);
    if take == 0 {
        return 0;
    }
    let root = disk.root;
    let mut cur = Some(sd as u32);
    while let Some(id) = cur {
        if id != root {
            disk.nodes[id as usize].size = disk.nodes[id as usize].size.saturating_sub(take);
        }
        let parent = disk.nodes[id as usize].parent;
        cur = (parent >= 0).then_some(parent as u32);
    }
    take
}

/// Bytes `brew cleanup -s --dry-run` says it would free.
pub fn parse_brew_cleanup(text: &str) -> u64 {
    for line in text.lines().rev() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("would free") || lower.contains("freed") {
            if let Some(bytes) = last_size(line) {
                return bytes;
            }
        }
    }
    0
}

/// Reclaimable column of `docker system df`, excluding volumes (prune does not remove them).
pub fn parse_docker_df(text: &str) -> u64 {
    let mut total = 0u64;
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("local volumes") || lower.starts_with("type") {
            continue;
        }
        if lower.starts_with("images")
            || lower.starts_with("containers")
            || lower.starts_with("build cache")
        {
            // The reclaimable field is the last size before an optional percent.
            if let Some(bytes) = last_size(line) {
                total += bytes;
            }
        }
    }
    total
}

/// Device UUIDs marked unavailable in `xcrun simctl list devices unavailable`.
pub fn parse_unavailable_simulators(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.contains("(unavailable)") {
            continue;
        }
        let Some(start) = line.rfind('(') else {
            continue;
        };
        let Some(end) = line.rfind(')') else { continue };
        if end < start {
            continue;
        }
        // The last parentheses are "(unavailable)"; the UUID is the pair before that.
        let head = line[..start].trim_end();
        let Some(us) = head.rfind('(') else { continue };
        let Some(ue) = head.rfind(')') else { continue };
        if ue > us {
            let id = head[us + 1..ue].trim();
            if id.len() >= 8 && id.contains('-') {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn last_size(line: &str) -> Option<u64> {
    // Walk tokens from the right, skipping "(57%)" style tails.
    let mut saw = None;
    for token in line.split_whitespace() {
        let token = token.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
        if token.ends_with('%') {
            continue;
        }
        if let Some(bytes) = parse_size_token(token) {
            saw = Some(bytes);
        }
    }
    saw
}

/// A size like `12.3MB`, `1.5GB`, `200kB`, `3GiB`. Decimal units, matching Finder on macOS.
fn parse_size_token(text: &str) -> Option<u64> {
    let text = text.trim().trim_end_matches('.');
    let split = text.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = text.split_at(split);
    let num: f64 = num.trim().parse().ok()?;
    if !num.is_finite() || num < 0.0 {
        return None;
    }
    let mult: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" | "k" => 1e3,
        "mb" | "m" => 1e6,
        "gb" | "g" => 1e9,
        "tb" | "t" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((num * mult).round() as u64)
}

#[cfg(test)]
mod tests;
