//! App discovery on macOS (PLAN.md §6.4, docs/history/phase3-plan.md §2). Read-only: lists bundles in the
//! Applications folders, tells where each came from (Homebrew cask, App Store, the OS, or copied
//! in), who owns it, when it was last opened (Spotlight), and which Library folders it left behind.
//!
//! Everything that touches the real system goes through [`Places`] (folders) and [`Probe`]
//! (Spotlight, LaunchServices, ownership), so tests run on fixture folders with fake answers.

use crate::apps::{
    is_bundle_id, vendor, App, Leftover, LeftoverKind, MatchedBy, Orphan, Owner, Source,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Folders discovery reads.
#[derive(Debug, Clone)]
pub(crate) struct Places {
    pub home: PathBuf,
    /// `/Applications` and `~/Applications`.
    pub app_dirs: Vec<PathBuf>,
    /// Homebrew's Caskrooms (`/opt/homebrew/Caskroom`, `/usr/local/Caskroom`).
    pub caskrooms: Vec<PathBuf>,
    /// The system Library (`/Library`): leftovers there need admin rights.
    pub system_library: PathBuf,
}

impl Places {
    pub(crate) fn system(home: &Path) -> Self {
        Places {
            home: home.to_path_buf(),
            app_dirs: vec![PathBuf::from("/Applications"), home.join("Applications")],
            caskrooms: vec![
                PathBuf::from("/opt/homebrew/Caskroom"),
                PathBuf::from("/usr/local/Caskroom"),
            ],
            system_library: PathBuf::from("/Library"),
        }
    }
}

/// What discovery asks the OS.
pub trait Probe {
    /// Owner uid of a path.
    fn owner(&self, path: &Path) -> Option<u32>;
    /// The current user's uid.
    fn uid(&self) -> Option<u32>;
    /// (days since last used, days since added) from Spotlight.
    fn usage(&self, app: &Path, today: u32) -> (Option<u32>, Option<u32>);
    /// How many apps LaunchServices knows with this bundle id, anywhere (other disks included).
    fn registered(&self, id: &str) -> usize;
    /// Today, in days since the Unix epoch.
    fn today(&self) -> u32;
}

/// The real thing: file ownership, Spotlight and LaunchServices.
pub struct System;

impl Probe for System {
    fn owner(&self, path: &Path) -> Option<u32> {
        use std::os::unix::fs::MetadataExt;
        fs::symlink_metadata(path).ok().map(|m| m.uid())
    }
    fn uid(&self) -> Option<u32> {
        // SAFETY: getuid has no preconditions.
        Some(unsafe { libc::getuid() })
    }
    fn usage(&self, app: &Path, today: u32) -> (Option<u32>, Option<u32>) {
        let days = |t: Option<i64>| t.map(|s| today.saturating_sub((s / 86_400) as u32));
        (
            days(super::spotlight::last_used(app)),
            days(super::spotlight::date_added(app)),
        )
    }
    fn registered(&self, id: &str) -> usize {
        super::spotlight::apps_with_bundle_id(id)
    }
    fn today(&self) -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| (d.as_secs() / 86_400) as u32)
    }
}

struct Bundle {
    id: Option<String>,
    version: Option<String>,
}

fn read_bundle(app: &Path) -> Bundle {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok();
    let dict = info.as_ref().and_then(|v| v.as_dictionary());
    let get = |k: &str| {
        dict.and_then(|d| d.get(k))
            .and_then(|v| v.as_string())
            .map(str::to_string)
    };
    Bundle {
        id: get("CFBundleIdentifier"),
        version: get("CFBundleShortVersionString").or_else(|| get("CFBundleVersion")),
    }
}

fn is_app(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "app") && p.is_dir()
}

/// `.app` bundles directly in each Applications folder and one plain folder deep.
fn bundles(places: &Places) -> Vec<(PathBuf, Option<PathBuf>)> {
    let mut out = Vec::new();
    for dir in &places.app_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) {
                continue; // a link to an app elsewhere is not an install
            }
            if is_app(&p) {
                out.push((p, None));
            } else if p.is_dir() {
                let Ok(inner) = fs::read_dir(&p) else {
                    continue;
                };
                let mut inner: Vec<PathBuf> = inner
                    .flatten()
                    .map(|e| e.path())
                    .filter(|q| is_app(q))
                    .collect();
                inner.sort();
                for q in inner {
                    out.push((q, Some(p.clone())));
                }
            }
        }
    }
    out
}

/// App bundle names each installed cask put in the Applications folders: name → token.
pub(crate) fn cask_apps(caskrooms: &[PathBuf]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for room in caskrooms {
        let Ok(tokens) = fs::read_dir(room) else {
            continue;
        };
        for t in tokens.flatten() {
            let token = t.file_name().to_string_lossy().into_owned();
            if !valid_token(&token) {
                continue;
            }
            for name in apps_of_cask(&t.path(), &token) {
                out.insert(name, token.clone());
            }
        }
    }
    out
}

/// Cask tokens are lowercase letters, digits and `-@._+` (Homebrew's own rule).
pub(crate) fn valid_token(token: &str) -> bool {
    let mut chars = token.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '@' | '.' | '_' | '+')
        })
        && !token.contains("..")
}

/// The `.app` names a cask installs, from its saved metadata (`.metadata/<version>/<stamp>/Casks/
/// <token>.json` or `.rb`).
fn apps_of_cask(dir: &Path, token: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.join(".metadata")];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name == format!("{token}.json") {
                if let Ok(v) = fs::read(&p)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .ok_or(())
                {
                    collect_json_apps(&v, &mut found);
                }
            } else if name == format!("{token}.rb") {
                if let Ok(text) = fs::read_to_string(&p) {
                    for line in text.lines() {
                        let l = line.trim();
                        if let Some(rest) = l.strip_prefix("app ") {
                            if let Some(n) = rest
                                .trim()
                                .strip_prefix('"')
                                .and_then(|r| r.split('"').next())
                            {
                                if n.ends_with(".app") {
                                    found.push(file_name(n));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn file_name(s: &str) -> String {
    Path::new(s)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn collect_json_apps(v: &serde_json::Value, out: &mut Vec<String>) {
    let Some(arts) = v.get("artifacts").and_then(|a| a.as_array()) else {
        return;
    };
    for a in arts {
        if let Some(apps) = a.get("app").and_then(|x| x.as_array()) {
            for x in apps {
                if let Some(s) = x.as_str().filter(|s| s.ends_with(".app")) {
                    out.push(file_name(s));
                }
            }
        }
    }
}

/// Every app in the Applications folders, with its leftovers.
pub(crate) fn discover(places: &Places, probe: &dyn Probe) -> Vec<App> {
    let casks = cask_apps(&places.caskrooms);
    let today = probe.today();
    let uid = probe.uid();
    let mut apps: Vec<App> = bundles(places)
        .into_iter()
        .map(|(bundle, folder)| {
            let b = read_bundle(&bundle);
            let file = bundle
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = file.strip_suffix(".app").unwrap_or(&file).to_string();
            let receipt = bundle.join("Contents/_MASReceipt/receipt").is_file();
            let owner = match (probe.owner(&bundle), uid) {
                (Some(o), Some(u)) if o == u => Owner::User,
                _ => Owner::Root,
            };
            let source = if let Some(token) = casks.get(&file) {
                Source::Cask {
                    token: token.clone(),
                }
            } else if receipt {
                Source::AppStore
            } else if b.id.as_deref().is_some_and(|i| i.starts_with("com.apple."))
                && owner == Owner::Root
            {
                Source::System
            } else {
                Source::Folder
            };
            let (last_used_days, added_days) = probe.usage(&bundle, today);
            App {
                id: b.id,
                name,
                bundle,
                version: b.version,
                folder,
                source,
                owner,
                last_used_days,
                added_days,
                leftovers: Vec::new(),
            }
        })
        .collect();
    let keys: Vec<(Option<String>, String)> = apps
        .iter()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    for i in 0..apps.len() {
        if apps[i].source == Source::System {
            continue; // part of the OS: its data is the OS's business
        }
        let mut found = leftovers(places, apps[i].id.as_deref(), &apps[i].name);
        for l in &mut found {
            l.shared_with = keys
                .iter()
                .enumerate()
                .filter(|&(j, (id, name))| j != i && matches_path(&l.path, id.as_deref(), name))
                .map(|(j, _)| apps[j].name.clone())
                .collect();
        }
        apps[i].leftovers = found;
    }
    apps
}

/// The app at `bundle`, discovered fresh (for uninstall: never trust what the UI sent).
pub fn app_at(places: &Places, probe: &dyn Probe, bundle: &Path) -> Option<App> {
    discover(places, probe)
        .into_iter()
        .find(|a| a.bundle == bundle)
}

/// Names too generic to tie a folder to one app by name alone.
const GENERIC: [&str; 12] = [
    "google",
    "microsoft",
    "adobe",
    "apple",
    "mozilla",
    "jetbrains",
    "app",
    "data",
    "cache",
    "caches",
    "logs",
    "temp",
];

fn name_usable(name: &str) -> bool {
    name.chars().count() >= 4 && !GENERIC.contains(&name.to_lowercase().as_str())
}

/// Whether a leftover path belongs to an app with this id or name.
fn matches_path(path: &Path, id: Option<&str>, name: &str) -> bool {
    let Some(file) = path.file_name().map(|f| f.to_string_lossy().into_owned()) else {
        return false;
    };
    if let Some(id) = id {
        if id_of_entry(&file).is_some_and(|e| e.eq_ignore_ascii_case(id)) {
            return true;
        }
    }
    name_usable(name) && file.eq_ignore_ascii_case(name)
}

/// The bundle id a Library entry is named after: `com.x.y`, `com.x.y.plist`, `com.x.y.savedState`,
/// `group.com.x.y`, `TEAMID1234.com.x.y`, `com.x.y.helper.plist` (launch items; prefix only).
fn id_of_entry(file: &str) -> Option<String> {
    let mut s = file;
    for suffix in [".plist", ".savedState", ".binarycookies"] {
        if let Some(r) = s.strip_suffix(suffix) {
            s = r;
        }
    }
    if let Some(r) = s.strip_prefix("group.") {
        s = r;
    } else if let Some((team, rest)) = s.split_once('.') {
        if team.len() == 10
            && team
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            s = rest;
        }
    }
    is_bundle_id(s).then(|| s.to_string())
}

/// Where leftovers live: (folder, kind, needs admin).
fn leftover_places(places: &Places) -> Vec<(PathBuf, LeftoverKind, bool)> {
    let lib = places.home.join("Library");
    let sys = &places.system_library;
    vec![
        (lib.join("Caches"), LeftoverKind::Caches, false),
        (
            lib.join("Application Support"),
            LeftoverKind::Support,
            false,
        ),
        (lib.join("Containers"), LeftoverKind::Containers, false),
        (
            lib.join("Group Containers"),
            LeftoverKind::GroupContainers,
            false,
        ),
        (lib.join("Preferences"), LeftoverKind::Preferences, false),
        (
            lib.join("Saved Application State"),
            LeftoverKind::SavedState,
            false,
        ),
        (lib.join("Logs"), LeftoverKind::Logs, false),
        (lib.join("HTTPStorages"), LeftoverKind::HttpStorages, false),
        (lib.join("WebKit"), LeftoverKind::WebKit, false),
        (lib.join("LaunchAgents"), LeftoverKind::LaunchAgents, false),
        (sys.join("Application Support"), LeftoverKind::Support, true),
        (sys.join("Caches"), LeftoverKind::Caches, true),
        (sys.join("LaunchAgents"), LeftoverKind::LaunchAgents, true),
        (sys.join("LaunchDaemons"), LeftoverKind::LaunchDaemons, true),
        (
            sys.join("PrivilegedHelperTools"),
            LeftoverKind::HelperTools,
            true,
        ),
    ]
}

/// Folders kinds where an entry may be named after the app's name (not only its id).
fn by_name(kind: LeftoverKind) -> bool {
    matches!(
        kind,
        LeftoverKind::Caches | LeftoverKind::Support | LeftoverKind::Logs
    )
}

fn leftovers(places: &Places, id: Option<&str>, name: &str) -> Vec<Leftover> {
    let mut out: Vec<Leftover> = Vec::new();
    for (dir, kind, needs_admin) in leftover_places(places) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            let file = p
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let launch = matches!(
                kind,
                LeftoverKind::LaunchAgents
                    | LeftoverKind::LaunchDaemons
                    | LeftoverKind::HelperTools
            );
            let by_id = id.is_some_and(|id| {
                let entry = id_of_entry(&file);
                entry.as_deref().is_some_and(|e| e.eq_ignore_ascii_case(id))
                    // Launch items and helpers are often `<id>.helper`, `<id>.agent.plist`.
                    || (launch && file.to_lowercase().starts_with(&format!("{}.", id.to_lowercase())))
            });
            let named =
                !by_id && by_name(kind) && name_usable(name) && file.eq_ignore_ascii_case(name);
            if !(by_id || named) {
                continue;
            }
            out.push(Leftover {
                path: p,
                kind,
                matched_by: if by_id {
                    MatchedBy::BundleId
                } else {
                    MatchedBy::Name
                },
                shared_with: Vec::new(),
                needs_admin,
            });
        }
    }
    out
}

/// Data folders named after a bundle id that no app has any more (docs/history/phase3-plan.md decision 5):
/// - named like a bundle id, not `com.apple.*`;
/// - no discovered app, running app or app LaunchServices knows (anywhere) shares its vendor;
/// - nothing in it changed for 30 days.
pub fn orphans(
    places: &Places,
    probe: &dyn Probe,
    apps: &[App],
    running_ids: &[String],
) -> Vec<Orphan> {
    let today = probe.today();
    let mut vendors: HashSet<String> = apps
        .iter()
        .filter_map(|a| a.id.as_deref())
        .chain(running_ids.iter().map(String::as_str))
        .map(vendor)
        .collect();
    vendors.insert("com.apple".into());
    let mut by_id: HashMap<String, Orphan> = HashMap::new();
    let mut checked: HashMap<String, bool> = HashMap::new();
    for (dir, kind, needs_admin) in leftover_places(places) {
        if needs_admin {
            continue; // the system Library: not ours to judge
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let file = e.file_name().to_string_lossy().into_owned();
            let Some(id) = id_of_entry(&file) else {
                continue;
            };
            let v = vendor(&id);
            if vendors.contains(&v) {
                continue;
            }
            let gone = *checked
                .entry(v.clone())
                .or_insert_with(|| probe.registered(&id) == 0);
            if !gone {
                continue;
            }
            let path = e.path();
            let untouched = today.saturating_sub(newest_day(&path));
            if untouched < 30 {
                continue;
            }
            let o = by_id.entry(id.to_lowercase()).or_insert_with(|| Orphan {
                id: id.clone(),
                items: Vec::new(),
                untouched_days: untouched,
            });
            o.untouched_days = o.untouched_days.min(untouched);
            o.items.push(Leftover {
                path,
                kind,
                matched_by: MatchedBy::BundleId,
                shared_with: Vec::new(),
                needs_admin: false,
            });
        }
    }
    let mut out: Vec<Orphan> = by_id.into_values().collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The most recent modification day of a path and (for folders) everything in it, to two levels.
fn newest_day(path: &Path) -> u32 {
    fn day(p: &Path) -> u32 {
        fs::symlink_metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(u32::MAX, |d| (d.as_secs() / 86_400) as u32)
    }
    let mut newest = day(path);
    let mut stack = vec![(path.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        if depth >= 2 {
            continue;
        }
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            newest = newest.max(day(&p));
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push((p, depth + 1));
            }
        }
    }
    // Unreadable dates count as "just now" (u32::MAX), so nothing unreadable looks abandoned.
    newest
}

/// Test doubles for discovery, shared with other modules' tests.
#[cfg(test)]
pub mod fake {
    use super::*;

    /// Fake answers: two root-owned bundles, fixed usage, one id registered on another disk.
    #[derive(Default)]
    pub(crate) struct Fake {
        pub root_owned: Vec<PathBuf>,
        pub usage: HashMap<String, (Option<u32>, Option<u32>)>,
        pub registered: Vec<String>,
    }

    impl Probe for Fake {
        fn owner(&self, path: &Path) -> Option<u32> {
            Some(if self.root_owned.iter().any(|r| r == path) {
                0
            } else {
                501
            })
        }
        fn uid(&self) -> Option<u32> {
            Some(501)
        }
        fn usage(&self, app: &Path, _today: u32) -> (Option<u32>, Option<u32>) {
            let name = app.file_name().unwrap().to_string_lossy().into_owned();
            self.usage.get(&name).copied().unwrap_or((None, None))
        }
        fn registered(&self, id: &str) -> usize {
            self.registered.iter().filter(|r| *r == id).count()
        }
        fn today(&self) -> u32 {
            System.today()
        }
    }
}

#[cfg(test)]
mod tests;
