//! Settings the owner changes in the app, saved as `settings.json` in Wrasse's data folder.
//! A missing or unreadable file means the defaults; values out of range are clamped.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Days a batch may stay in the tray before it's emptied automatically (PLAN.md §6.11).
pub const KEEP_DAYS_MIN: u64 = 1;
pub const KEEP_DAYS_MAX: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Days a batch stays in the tray (1–30).
    pub keep_days: u64,
    /// Check GitHub Releases for an update when Wrasse starts. Off until the owner turns it on:
    /// Wrasse makes no network request while it's off.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            keep_days: crate::quarantine::DEFAULT_KEEP_DAYS,
            check_updates: false,
        }
    }
}

impl Settings {
    fn clamped(mut self) -> Self {
        self.keep_days = self.keep_days.clamp(KEEP_DAYS_MIN, KEEP_DAYS_MAX);
        self
    }
}

const FILE: &str = "settings.json";

/// The saved settings, or the defaults.
pub fn load(data_dir: &Path) -> Settings {
    std::fs::read(data_dir.join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
        .unwrap_or_default()
        .clamped()
}

/// Saves the settings (clamped) and returns what was saved.
pub fn save(data_dir: &Path, settings: Settings) -> io::Result<Settings> {
    let settings = settings.clamped();
    let json = serde_json::to_vec_pretty(&settings).map_err(io::Error::other)?;
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join(FILE), json)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wrasse-settings-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_when_missing_or_broken() {
        let d = dir("missing");
        assert_eq!(load(&d), Settings::default());
        std::fs::write(d.join(FILE), b"not json").unwrap();
        assert_eq!(load(&d).keep_days, crate::quarantine::DEFAULT_KEEP_DAYS);
        crate::platform::remove_test_dir(&d);
    }

    #[test]
    fn keep_days_round_trip_and_clamp() {
        let d = dir("clamp");
        assert_eq!(
            save(
                &d,
                Settings {
                    keep_days: 14,
                    ..Default::default()
                }
            )
            .unwrap()
            .keep_days,
            14
        );
        assert_eq!(load(&d).keep_days, 14);
        assert_eq!(
            save(
                &d,
                Settings {
                    keep_days: 0,
                    ..Default::default()
                }
            )
            .unwrap()
            .keep_days,
            1
        );
        assert_eq!(
            save(
                &d,
                Settings {
                    keep_days: 99,
                    ..Default::default()
                }
            )
            .unwrap()
            .keep_days,
            30
        );
        assert_eq!(load(&d).keep_days, 30);
        crate::platform::remove_test_dir(&d);
    }
}
