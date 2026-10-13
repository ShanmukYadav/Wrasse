//! Installed apps (PLAN.md §6.4): what's installed, where it came from, how big it really is
//! (bundle plus the data it left in the user's Library), when it was last opened, and how it can be
//! removed. Platform-neutral: discovery itself lives in `platform` (macOS: `platform/macos/apps.rs`).
//!
//! Removal routes (docs/history/phase3-plan.md §3):
//! - a Homebrew cask is removed by Homebrew (`safety::delegate`), not undoable;
//! - an app the user owns (dragged in) goes to the tray (`safety::remove`), restorable;
//! - a root-owned app (App Store, `.pkg`) goes to the user's Trash through Finder, which asks
//!   for the password (`safety::delegate`); Put Back works;
//! - an app that's part of the OS isn't removable.
//!
//! In every route the leftovers the user ticked go to the tray.

pub mod uninstall;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where an app came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Source {
    /// Copied into an Applications folder by the user (or an installer that runs as the user).
    Folder,
    /// Installed by Homebrew as a cask.
    Cask { token: String },
    /// Installed from the App Store.
    AppStore,
    /// Part of the OS (e.g. Safari); shown, never removable.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Owner {
    /// The bundle belongs to the current user: Wrasse can move it itself.
    User,
    /// Owned by root (App Store and package installs): the OS has to remove it.
    Root,
}

/// What kind of leftover a path is (where it lives in the Library).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeftoverKind {
    Caches,
    Support,
    Containers,
    GroupContainers,
    Preferences,
    SavedState,
    Logs,
    HttpStorages,
    WebKit,
    LaunchAgents,
    LaunchDaemons,
    HelperTools,
}

impl LeftoverKind {
    /// Rebuilt by the app when needed: safe to remove.
    pub fn regenerates(self) -> bool {
        matches!(
            self,
            LeftoverKind::Caches
                | LeftoverKind::Logs
                | LeftoverKind::SavedState
                | LeftoverKind::HttpStorages
                | LeftoverKind::WebKit
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            LeftoverKind::Caches => "Caches",
            LeftoverKind::Support => "Application Support",
            LeftoverKind::Containers => "Containers",
            LeftoverKind::GroupContainers => "Group Containers",
            LeftoverKind::Preferences => "Preferences",
            LeftoverKind::SavedState => "Saved Application State",
            LeftoverKind::Logs => "Logs",
            LeftoverKind::HttpStorages => "HTTP storage",
            LeftoverKind::WebKit => "WebKit data",
            LeftoverKind::LaunchAgents => "Launch agent",
            LeftoverKind::LaunchDaemons => "Launch daemon",
            LeftoverKind::HelperTools => "Helper tool",
        }
    }
}

/// How a leftover was tied to its app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedBy {
    /// Named after the app's bundle identifier: certainly the app's.
    BundleId,
    /// Named after the app: probably the app's, so "your call".
    Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: PathBuf,
    pub kind: LeftoverKind,
    pub matched_by: MatchedBy,
    /// Other installed apps this also matches (a vendor folder): never ticked by default.
    pub shared_with: Vec<String>,
    /// In the system Library, owned by root: shown, not removable by Wrasse.
    pub needs_admin: bool,
}

impl Leftover {
    /// Ticked by default in the uninstall sheet (docs/history/phase3-plan.md decision 4).
    pub fn ticked_by_default(&self) -> bool {
        self.matched_by == MatchedBy::BundleId && self.shared_with.is_empty() && !self.needs_admin
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    /// Bundle identifier, when the bundle has one.
    pub id: Option<String>,
    pub name: String,
    pub bundle: PathBuf,
    pub version: Option<String>,
    /// The plain folder it sits in inside an Applications folder (e.g. "Adobe Creative Cloud").
    pub folder: Option<PathBuf>,
    pub source: Source,
    pub owner: Owner,
    /// Days since it was last opened, from Spotlight. `None`: Spotlight doesn't know.
    pub last_used_days: Option<u32>,
    /// Days since it was added to the Mac, from Spotlight.
    pub added_days: Option<u32>,
    pub leftovers: Vec<Leftover>,
}

impl App {
    pub fn removable(&self) -> bool {
        self.source != Source::System
    }

    /// "Never opened": Spotlight has no last-used date and the app has been here a while.
    pub fn never_opened(&self) -> bool {
        self.last_used_days.is_none() && self.added_days.is_some_and(|d| d > 30)
    }
}

/// Data folders whose app is gone (docs/history/phase3-plan.md §3, decision 5), grouped by the bundle id
/// they're named after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Orphan {
    pub id: String,
    pub items: Vec<Leftover>,
    /// Days since anything in it changed.
    pub untouched_days: u32,
}

/// Looks like a reverse-DNS bundle id: three or more dot-separated parts of letters, digits, `-`
/// and `_`, the first one lowercase letters (com, org, io, …).
pub fn is_bundle_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        && parts[0].chars().all(|c| c.is_ascii_lowercase())
}

/// The vendor part of a bundle id ("com.microsoft" for "com.microsoft.Word").
pub fn vendor(id: &str) -> String {
    id.split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_ids_and_vendors() {
        for ok in ["com.spotify.client", "org.mozilla.firefox", "io.a-b.c_d"] {
            assert!(is_bundle_id(ok), "{ok}");
        }
        for bad in [
            "Spotify",
            "com.apple",
            "Com.x.y",
            "com..x",
            "com.x.y z",
            "a.b.c/d",
        ] {
            assert!(!is_bundle_id(bad), "{bad}");
        }
        assert_eq!(vendor("com.Microsoft.Word"), "com.microsoft");
    }
}
