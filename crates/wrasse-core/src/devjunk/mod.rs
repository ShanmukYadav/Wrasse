//! Developer junk (PLAN.md §6.8). Discovery is a read of the scan tree.
//! Homebrew, Docker and unavailable simulators are previews here; removing them
//! goes through `safety::delegate` and only when that tool is the one doing it.
//! The Wrasse project and its `.toolchain` are never offered.

use crate::scan::breakdown::{Category, NodeKind, Safety, UiDisk, UiNode};
use serde::{Deserialize, Serialize};

/// A project folder unused this long is offered. Younger ones stay visible, unticked.
pub const STALE_DAYS: u32 = 90;

/// Something inside a project worth lifting into the Developer category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectItem {
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
pub struct Policy {
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
pub fn is_wrasse_root(child_names: &[&str]) -> bool {
    child_names.contains(&".toolchain")
}

/// The checkout, its `.toolchain`, and everything inside either.
pub fn wrasse_path(path: &str) -> bool {
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
pub struct ProjectSpot<'a> {
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
pub fn project_item(spot: &ProjectSpot<'_>) -> Option<ProjectItem> {
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

pub fn project_label(kind: ProjectItem, project: &str) -> String {
    match kind {
        ProjectItem::Modules => format!("{project} · node_modules"),
        ProjectItem::Target => format!("{project} · target"),
        ProjectItem::Venv => format!("{project} · Python"),
        ProjectItem::Conda => format!("{project} · conda"),
    }
}

pub fn project_kind(kind: ProjectItem) -> &'static str {
    match kind {
        ProjectItem::Modules => "modules",
        ProjectItem::Target => "target",
        ProjectItem::Venv => "venv",
        ProjectItem::Conda => "conda",
    }
}

/// Documents, Desktop and iCloud Drive stay Your call (never pre-selected).
pub fn hand_pick(path: &str) -> bool {
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
pub fn policy_for(path: &str, age_days: u32) -> Option<Policy> {
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
pub fn rank_why(rebuildable: bool, dev_kind: Option<&str>, age_days: u32) -> Option<RankWhy> {
    if rebuildable {
        return Some(RankWhy::Rebuilds);
    }
    if age_days >= STALE_DAYS && matches!(dev_kind, Some("venv" | "conda" | "archive")) {
        return Some(RankWhy::Unused);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankWhy {
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
pub fn attach_live(disk: &mut UiDisk, extras: &[LiveExtra]) {
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
mod tests {
    use super::*;

    macro_rules! ask {
        ($name:expr, $children:expr, $parents:expr, $parent:expr, $grand:expr, $age:expr, $mods:expr, $wrasse:expr) => {
            project_item(&ProjectSpot {
                name: $name,
                children: $children,
                parent_children: $parents,
                parent_name: $parent,
                grandparent_name: $grand,
                age_days: $age,
                in_modules: $mods,
                in_wrasse: $wrasse,
            })
        };
    }

    #[test]
    fn wrasse_and_its_toolchain_are_never_project_junk() {
        assert!(is_wrasse_root(&["src", ".toolchain", "app"]));
        assert!(!is_wrasse_root(&["src", "app"]));
        assert!(wrasse_path("/Users/me/Documents/Wrasse/.toolchain/rust"));
        assert!(wrasse_path("/Users/me/Documents/Wrasse/target/debug"));
        assert!(wrasse_path("/Users/me/Documents/Wrasse"));
        assert!(!wrasse_path("/Users/me/Library/Developer/Toolchains/x"));
        let kids = ["Cargo.toml", "src"];
        assert_eq!(
            ask!(
                "target",
                &[],
                &kids,
                "Wrasse",
                "Documents",
                400,
                false,
                true
            ),
            None
        );
        assert_eq!(
            ask!(
                "node_modules",
                &[],
                &[],
                "Wrasse",
                "Documents",
                400,
                false,
                true
            ),
            None
        );
        assert_eq!(
            ask!(
                ".toolchain",
                &["rust"],
                &[],
                "Wrasse",
                "Documents",
                400,
                false,
                false
            ),
            None
        );
    }

    #[test]
    fn stale_modules_and_targets_are_claimed_and_recent_ones_are_not() {
        assert_eq!(
            ask!(
                "node_modules",
                &[],
                &[],
                "web",
                "Projects",
                90,
                false,
                false
            ),
            Some(ProjectItem::Modules)
        );
        assert_eq!(
            ask!(
                "node_modules",
                &[],
                &[],
                "web",
                "Projects",
                89,
                false,
                false
            ),
            None
        );
        // A nested copy stays inside the outer one.
        assert_eq!(
            ask!(
                "node_modules",
                &[],
                &[],
                "pkg",
                "node_modules",
                400,
                true,
                false
            ),
            None
        );
        assert_eq!(
            ask!(
                "target",
                &[],
                &["Cargo.toml", "src"],
                "api",
                "code",
                90,
                false,
                false
            ),
            Some(ProjectItem::Target)
        );
        assert_eq!(
            ask!(
                "target",
                &[],
                &["Makefile"],
                "api",
                "code",
                400,
                false,
                false
            ),
            None
        );
        assert_eq!(
            project_label(ProjectItem::Modules, "web"),
            "web · node_modules"
        );
    }

    #[test]
    fn venvs_need_a_pyvenv_and_conda_envs_need_their_root() {
        assert_eq!(
            ask!(
                ".venv",
                &["pyvenv.cfg", "bin"],
                &[],
                "app",
                "code",
                1,
                false,
                false
            ),
            Some(ProjectItem::Venv)
        );
        assert_eq!(
            ask!(
                "env",
                &["pyvenv.cfg"],
                &[],
                "vyakta",
                "Documents",
                10,
                false,
                false
            ),
            Some(ProjectItem::Venv)
        );
        assert_eq!(
            ask!(
                "env",
                &["data"],
                &[],
                "vyakta",
                "Documents",
                10,
                false,
                false
            ),
            None
        );
        assert_eq!(
            ask!("ml", &[], &[], "envs", "miniconda3", 10, false, false),
            Some(ProjectItem::Conda)
        );
        assert_eq!(
            ask!("ml", &[], &[], "envs", "projects", 10, false, false),
            None
        );
    }

    #[test]
    fn docker_and_homebrew_are_shown_and_not_removable() {
        let docker = policy_for("/Users/me/Library/Containers/com.docker.docker", 10).unwrap();
        assert!(docker.read_only && docker.safety == Safety::Keep && !docker.rebuildable);
        let brew = policy_for("/opt/homebrew", 10).unwrap();
        assert!(brew.read_only && brew.safety == Safety::Keep);
        let derived = policy_for("/Users/me/Library/Developer/Xcode/DerivedData", 1).unwrap();
        assert!(derived.rebuildable && derived.safety == Safety::Safe);
        let archive =
            policy_for("/Users/me/Library/Developer/Xcode/Archives/2024-03-01", 100).unwrap();
        assert_eq!(archive.dev_kind, Some("archive"));
        assert!(!archive.rebuildable);
        assert_eq!(rank_why(false, Some("archive"), 100), Some(RankWhy::Unused));
        assert_eq!(rank_why(false, Some("archive"), 10), None);
    }

    #[test]
    fn a_project_cache_in_documents_stays_your_call() {
        let p = policy_for("/Users/me/Documents/app/node_modules", 200).unwrap();
        assert_eq!(p.safety, Safety::YourCall);
        assert!(!p.rebuildable);
        let outside = policy_for("/Users/me/Projects/web/node_modules", 200).unwrap();
        assert_eq!(outside.safety, Safety::Safe);
        assert!(outside.rebuildable);
        let fresh = policy_for("/Users/me/Projects/web/node_modules", 3).unwrap();
        assert!(!fresh.rebuildable);
    }

    #[test]
    fn previews_parse_without_running_the_tools() {
        let brew = "Would remove: /opt/homebrew/Cellar/wget/1.21 (1.2MB)\n==> This operation would free approximately 12.5MB of disk space.\n";
        assert_eq!(parse_brew_cleanup(brew), 12_500_000);
        let df = "\
TYPE            TOTAL     ACTIVE    SIZE      RECLAIMABLE
Images          2         1         2.0GB     1.5GB (75%)
Containers      1         0         100MB     100MB (100%)
Local Volumes   1         1         9.0GB     9.0GB (100%)
Build Cache     4         0         512MB     512MB
";
        // Volumes are not part of `docker system prune` without `--volumes`.
        assert_eq!(
            parse_docker_df(df),
            1_500_000_000 + 100_000_000 + 512_000_000
        );
        let sims = "\
-- iOS 17.0 --
    iPhone 15 (A1B2C3D4-E5F6-7890-ABCD-EF1234567890) (unavailable)
    iPhone 14 (11111111-2222-3333-4444-555555555555) (Shutdown)
";
        assert_eq!(
            parse_unavailable_simulators(sims),
            vec!["A1B2C3D4-E5F6-7890-ABCD-EF1234567890".to_string()]
        );
    }

    /// A folded Developer tile comes back without changing the volume total.
    /// A tile that is already showing lists the preview and does not add its bytes again.
    #[test]
    fn live_previews_keep_the_volume_total() {
        use crate::scan::breakdown::Summary;
        let mut root = UiNode::plain(
            0,
            "Macintosh HD",
            "/",
            -1,
            0,
            Category::Other,
            NodeKind::Volume,
            Safety::Keep,
            "",
        );
        root.size = 100;
        root.children = vec![1];
        let mut system = UiNode::plain(
            1,
            "System data",
            "/",
            0,
            1,
            Category::System,
            NodeKind::File,
            Safety::Keep,
            "",
        );
        system.size = 40;
        let dev = UiNode::plain(
            2,
            "Developer",
            "/Users/me/Library",
            -1,
            1,
            Category::Developer,
            NodeKind::Dir,
            Safety::Safe,
            "",
        );
        let mut disk = UiDisk {
            nodes: vec![root, system, dev],
            root: 0,
            capacity: 200,
            ghost_id: -1,
            leaf_count: 1,
            summary: Summary::default(),
        };
        let extra = LiveExtra {
            name: "Homebrew cleanup".into(),
            path: "homebrew:cleanup".into(),
            bytes: 15,
            why: "preview".into(),
            action: "brewCleanup".into(),
        };
        attach_live(&mut disk, std::slice::from_ref(&extra));
        assert!(disk.nodes[0].children.contains(&2));
        assert_eq!(disk.nodes[0].size, 100);
        assert_eq!(disk.nodes[1].size, 25);
        assert_eq!(disk.nodes[2].size, 15);
        let child = *disk.nodes[2].children.last().unwrap();
        assert_eq!(
            disk.nodes[child as usize].delegate_action.as_deref(),
            Some("brewCleanup")
        );
        assert_eq!(disk.nodes[child as usize].size, 15);

        let kids = disk.nodes[2].children.len();
        attach_live(&mut disk, &[extra]);
        assert_eq!(disk.nodes[0].size, 100);
        assert_eq!(disk.nodes[1].size, 25);
        assert_eq!(disk.nodes[2].size, 15);
        assert_eq!(disk.nodes[2].children.len(), kids + 1);
    }
}
