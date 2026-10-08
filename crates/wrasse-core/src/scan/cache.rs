//! The last scan, saved outside the project (PLAN.md §5.4): `~/Library/Application Support/
//! com.shanmukyadav.wrasse/` on macOS. Used to show the map instantly at launch and for
//! "what grew". Writing it is the only file Wrasse creates in this phase.

use super::breakdown::UiDisk;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Caches above this size are not written (PLAN.md §5.4 budget).
const MAX_BYTES: usize = 200_000_000;
const FILE: &str = "scan-cache.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheFile {
    version: u32,
    /// Seconds since the Unix epoch.
    scanned_at: u64,
    disk: UiDisk,
}

/// The app's data folder for this OS.
pub fn data_dir(identifier: &str) -> Option<PathBuf> {
    let home = crate::platform::home_dir()?;
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support").join(identifier))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(identifier))
    } else {
        Some(home.join(".local/share").join(identifier))
    }
}

pub fn save(dir: &Path, disk: &UiDisk, scanned_at: u64) -> io::Result<()> {
    let json = serde_json::to_vec(&CacheFile {
        version: VERSION,
        scanned_at,
        disk: disk.clone(),
    })
    .map_err(io::Error::other)?;
    if json.len() > MAX_BYTES {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    fs::write(dir.join(FILE), json)
}

/// The cached disk and when it was scanned, if a readable cache of this version exists.
pub fn load(dir: &Path) -> Option<(UiDisk, u64)> {
    let bytes = fs::read(dir.join(FILE)).ok()?;
    let file: CacheFile = serde_json::from_slice(&bytes).ok()?;
    (file.version == VERSION).then_some((file.disk, file.scanned_at))
}
