//! One whole-volume scan, end to end: walk, break down into categories, compare with the last
//! scan ("what grew") and save the result. Read-only apart from the cache file.

use crate::platform::{self, Volumes};
use crate::scan::breakdown::{self, Input, UiDisk};
use crate::scan::{cache, rescan, scan, Change, ScanControl, Tree};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variables that move model folders (PLAN.md §6.6).
const MODEL_ENV: [&str; 3] = ["OLLAMA_MODELS", "HF_HOME", "HF_HUB_CACHE"];

pub fn scan_volume(control: &ScanControl, cache_dir: Option<&Path>) -> io::Result<UiDisk> {
    let (root, prefix) = platform::scan_root();
    // The change-log position goes with the tree, taken *before* walking so that anything that
    // changes during the walk is replayed next time.
    let (position, log_id) = platform::change_log_position(&root);
    let (tree, changed) = walk(&root, control, cache_dir)?;
    let scanned = tree.node(tree.root()).size;
    let volumes = platform::volumes().unwrap_or(Volumes {
        capacity: scanned,
        available: 0,
        os_used: 0,
    });
    let previous = cache_dir
        .and_then(cache::load)
        .map(|(d, _)| breakdown::sizes_by_path(&d));
    let env: HashMap<String, String> = MODEL_ENV
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .collect();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Apps and the data they left (read-only; Spotlight for last use).
    let home = platform::home_dir();
    let apps = home
        .as_deref()
        .map(platform::installed_apps)
        .unwrap_or_default();
    let orphans = home
        .as_deref()
        .map(|h| platform::orphaned_leftovers(h, &apps))
        .unwrap_or_default();
    let input = Input {
        tree: &tree,
        display_prefix: prefix,
        home,
        volumes,
        volume_name: platform::volume_name(),
        previous,
        running: platform::running_executables(),
        today: (now / 86_400) as u32,
        env,
        apps: &apps,
        orphans: &orphans,
        hf_models: &crate::scan::hf::hf_models,
    };
    let mut disk = breakdown::build(&input);
    if let (Some(home), Some(dir)) = (platform::home_dir(), cache_dir) {
        let arena_size = |p: &Path| {
            let arena = format!("{}{}", prefix.trim_end_matches('/'), p.display());
            tree.find(&arena)
                .map(|id| tree.node(id).size)
                .unwrap_or_else(|| crate::quarantine::size_on_disk(p))
        };
        let report = junk_with(&home, dir, &arena_size);
        annotate(&mut disk, &report.matches);
    }
    disk.summary.incremental = changed.is_some();
    disk.summary.changed_folders = changed.unwrap_or(0) as u64;
    if let Some(dir) = cache_dir {
        if !disk.summary.cancelled {
            // Caches that can't be written only cost a full scan and "what grew" next time.
            let _ = cache::save(dir, &disk, now);
            let _ = cache::save_tree(dir, &tree, position, log_id.as_deref(), now);
        }
    }
    drop(tree);
    platform::release_free_memory();
    Ok(disk)
}

/// The last saved scan, to show at launch while a new one runs.
pub fn cached(cache_dir: &Path) -> Option<UiDisk> {
    cache::load(cache_dir).map(|(d, _)| d)
}

/// An incremental rescan when the last tree and the change log allow it, else a full walk.
/// Returns the tree and, for an incremental rescan, how many changed folders were re-read.
fn walk(
    root: &Path,
    control: &ScanControl,
    cache_dir: Option<&Path>,
) -> io::Result<(Tree, Option<usize>)> {
    let saved = cache_dir
        .and_then(cache::load_tree)
        .filter(|s| Path::new(&s.tree.root_path) == root);
    if let Some(saved) = saved {
        if let Some(events) = platform::changes_since(root, saved.event_id, saved.log_id.as_deref())
        {
            let changes: Vec<Change> = events
                .into_iter()
                .map(|(path, recursive)| Change {
                    path: inside(root, path),
                    recursive,
                })
                .collect();
            let count = changes.len();
            let tree = rescan(&saved.tree, &changes, control)?;
            return Ok((tree, Some(count)));
        }
    }
    Ok((scan(root, control)?, None))
}

/// FSEvents may report firmlinked paths (`/Users/…`) for the data volume; map them under the
/// scan root (`/System/Volumes/Data/Users/…`).
fn inside(root: &Path, path: PathBuf) -> PathBuf {
    if path.starts_with(root) {
        return path;
    }
    match path.strip_prefix("/") {
        Ok(rel) if root != Path::new("/") => root.join(rel),
        _ => path,
    }
}

/// What the junk rules find, for the System Junk screen.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JunkReport {
    pub matches: Vec<crate::rules::Match>,
    pub held: Vec<crate::rules::Held>,
    pub errors: Vec<crate::rules::RuleError>,
}

/// Junk rules applied now, with sizes from the last saved scan where possible.
pub fn junk(data_dir: &Path) -> Option<JunkReport> {
    let home = platform::home_dir()?;
    let (_, prefix) = platform::scan_root();
    let saved = cache::load_tree(data_dir);
    let size = |p: &Path| {
        let arena = format!("{}{}", prefix.trim_end_matches('/'), p.display());
        saved
            .as_ref()
            .and_then(|s| s.tree.find(&arena).map(|id| s.tree.node(id).size))
            .unwrap_or_else(|| crate::quarantine::size_on_disk(p))
    };
    Some(junk_with(&home, data_dir, &size))
}

fn junk_with(home: &Path, data_dir: &Path, size: &dyn Fn(&Path) -> u64) -> JunkReport {
    let protected = platform::protected_paths(home, data_dir);
    let (rules, errors) = crate::rules::load_all(home, data_dir, &protected);
    let running = platform::running_bundle_ids();
    let (matches, held) = crate::rules::find(&rules, &running, size);
    JunkReport {
        matches,
        held,
        errors,
    }
}

/// Gives scan items covered by a rule that rule's safety and explanation.
fn annotate(disk: &mut UiDisk, matches: &[crate::rules::Match]) {
    let by_path: HashMap<String, &crate::rules::Match> = matches
        .iter()
        .map(|m| (m.path.to_string_lossy().into_owned(), m))
        .collect();
    for n in &mut disk.nodes {
        let Some(m) = by_path.get(&n.path) else {
            continue;
        };
        n.rule_id = Some(m.rule_id.clone());
        n.rule_root = Some(m.root.to_string_lossy().into_owned());
        n.why = m.explanation.clone();
        n.rebuildable = m.regenerates;
        n.delegate = m.delegate.is_some();
        n.safety = match m.safety {
            crate::safety::Safety::Safe => breakdown::Safety::Safe,
            crate::safety::Safety::YourCall => breakdown::Safety::YourCall,
        };
    }
}
