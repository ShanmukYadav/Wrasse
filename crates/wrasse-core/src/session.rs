//! One whole-volume scan, end to end: walk, break down into categories, compare with the last
//! scan ("what grew") and save the result. Read-only apart from the cache file.

use crate::platform::{self, Volumes};
use crate::scan::breakdown::{self, Input, UiDisk};
use crate::scan::{cache, scan, ScanControl};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variables that move model folders (PLAN.md §6.6).
const MODEL_ENV: [&str; 3] = ["OLLAMA_MODELS", "HF_HOME", "HF_HUB_CACHE"];

pub fn scan_volume(control: &ScanControl, cache_dir: Option<&Path>) -> io::Result<UiDisk> {
    let (root, prefix) = platform::scan_root();
    let tree = scan(&root, control)?;
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
    let input = Input {
        tree: &tree,
        display_prefix: prefix,
        home: platform::home_dir(),
        volumes,
        volume_name: platform::volume_name(),
        previous,
        running: platform::running_executables(),
        today: (now / 86_400) as u32,
        env,
        bundle_id: &platform::bundle_id,
    };
    let disk = breakdown::build(&input);
    drop(tree);
    if let Some(dir) = cache_dir {
        if !disk.summary.cancelled {
            // A cache that can't be written only costs "what grew" next time.
            let _ = cache::save(dir, &disk, now);
        }
    }
    Ok(disk)
}

/// The last saved scan, to show at launch while a new one runs.
pub fn cached(cache_dir: &Path) -> Option<UiDisk> {
    cache::load(cache_dir).map(|(d, _)| d)
}
