//! The only destructive entrypoints in Wrasse: [`remove`] and [`delegate`] (PLAN.md §4.1).
//! Nothing outside `safety/`, `quarantine/` and `platform/` may delete, rename or spawn
//! processes; `scripts/check-forbidden-apis.sh` enforces this (CI and the pre-commit hook).
//!
//! Removal is two steps, so the user always sees what will happen before it does:
//! 1. [`prepare`] checks every item against the rules below and returns a [`Plan`]: what would be
//!    removed, how many bytes, what was skipped and why, and whether it can be undone.
//! 2. [`remove`] re-checks each planned item right before acting (same file, not swapped for a
//!    link, and for "your call" items unchanged) and then moves it to quarantine or deletes it,
//!    or, in dry-run, only reports what it would do. Every outcome goes to the audit log.
//!
//! Checks (PLAN.md §4.2–4.3): protected locations (hardcoded per OS in `platform`), the user's
//! never-touch list, the startup disk only, the current user's files only, no iCloud-only
//! (dataless) items, no symlink escapes out of a rule's root, open files left alone, "Delete now"
//! only for items labeled Safe, items in Documents/Desktop/iCloud Drive only when picked by hand
//! and only to quarantine. Platform-neutral: OS specifics come from `platform`.

pub mod audit;
mod check;
mod delegate;
#[cfg(test)]
mod dupes_tests;
pub mod exclusions;
mod remove;
#[cfg(test)]
mod uninstall_tests;

pub use check::prepare;
use check::*;
pub use delegate::*;
use remove::*;
pub use remove::{purge, remove, remove_own_data, restore, OwnData};

use crate::platform::{self, Protected, Protection};
use crate::quarantine::Quarantine;
use audit::{AuditLog, Record};
use exclusions::Exclusions;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

/// One operation at a time (PLAN.md §4.3 rule 9).
static OPERATION: Mutex<()> = Mutex::new(());

/// Whether a cleanup is running right now (the app waits for it before handing over to the
/// menu bar).
pub fn busy() -> bool {
    OPERATION.try_lock().is_err()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Moved aside, restorable until the quarantine is emptied.
    Quarantine,
    /// Deleted for good. Only for items labeled Safe. Not undoable.
    DeleteNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    Safe,
    YourCall,
}

/// One thing the user (or a rule) wants removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub path: PathBuf,
    pub safety: Safety,
    #[serde(default)]
    pub rule_id: Option<String>,
    /// For rule items: the rule's root; the item must stay inside it after resolving links.
    #[serde(default)]
    pub root: Option<PathBuf>,
    /// The user picked this item themselves (needed for Documents, Desktop and iCloud Drive).
    #[serde(default)]
    pub hand_picked: bool,
    /// Size and modification time from the scan. When set, a file that no longer matches is
    /// skipped (Large & Old). Absent for every other removal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanned: Option<Scanned>,
}

/// What the scan recorded for a file, so a later move can see that it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scanned {
    /// Allocated bytes (`st_blocks * 512`), the size the scan stored.
    pub bytes: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Protected,
    /// In Documents, Desktop or iCloud Drive and not picked by hand (or asked for Delete now).
    HandPickOnly,
    Excluded,
    OtherVolume,
    OtherUser,
    InCloudOnly,
    OutsideRoot,
    NotFound,
    /// It changed between the preview and the action (different file, now a link, or edited).
    Changed,
    /// Its size or modification time no longer matches the scan that listed it.
    ChangedSinceScan,
    InUse,
    /// An app that's open (its bundle, while a process runs from it).
    AppRunning,
    /// "Delete now" is only for items labeled Safe.
    NotSafeForDeleteNow,
    /// A duplicate that has not been confirmed byte for byte.
    NotConfirmed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Warning {
    /// iCloud syncs this folder: removing it here removes it on the user's other devices too.
    SyncedToICloud,
}

/// What an item was when planned, to re-check right before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
    pub uid: Option<u32>,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub len: u64,
    pub mtime: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Planned {
    pub request: Request,
    pub identity: Identity,
    pub bytes: u64,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub mode: Mode,
    pub items: Vec<Planned>,
    pub skipped: Vec<Skipped>,
    pub bytes: u64,
    /// False for Delete now: shown as "Not undoable" before the user confirms.
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub path: PathBuf,
    pub bytes: u64,
    /// The quarantine entry, for restore.
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub done: Vec<Done>,
    pub skipped: Vec<Skipped>,
    pub bytes: u64,
    pub dry_run: bool,
}

/// Everything the checks need, gathered once.
pub struct Context {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub protected: Vec<Protected>,
    pub synced: Vec<PathBuf>,
    pub exclusions: Exclusions,
    pub quarantine: Quarantine,
    pub audit: AuditLog,
    /// Device of the startup (data) volume; v1 cleans only this disk.
    pub startup_dev: u64,
    pub uid: Option<u32>,
}

impl Context {
    pub fn new(home: PathBuf, data_dir: PathBuf) -> io::Result<Self> {
        // Compared against resolved item paths, so resolve these too.
        fs::create_dir_all(&data_dir)?;
        let home = resolve_dir(&home)?;
        let data_dir = resolve_dir(&data_dir)?;
        let quarantine = Quarantine::open(data_dir.join("Quarantine"))?;
        let startup_dev = fs::symlink_metadata(&home).map(|m| platform::file_identity(&m).0)?;
        Ok(Context {
            protected: platform::protected_paths(&home, &data_dir),
            synced: platform::synced_folders(&home),
            exclusions: Exclusions::load(&data_dir),
            audit: AuditLog::new(&data_dir),
            uid: platform::current_uid(),
            home,
            data_dir,
            quarantine,
            startup_dev,
        })
    }
}

#[cfg(test)]
mod tests;
