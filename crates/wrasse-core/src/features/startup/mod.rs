//! Startup Items (PLAN.md §6.10, macOS): what starts with the Mac or at login, from the launchd
//! property lists in `~/Library/LaunchAgents` (yours), `/Library/LaunchAgents` and
//! `/Library/LaunchDaemons` (for all users, display-only: "Needs admin").
//!
//! Only your own agents can be turned off: Wrasse unloads it (`launchctl bootout`, a delegated
//! action) and moves its plist to the tray. Restoring the plist from the tray turns it back on at
//! the next login. Apple's items (`com.apple.*`) are never touched.

use crate::safety::{self, Context, Delegated, Mode, Request, Safety};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// `~/Library/LaunchAgents`: starts at your login. You can turn it off.
    User,
    /// `/Library/LaunchAgents`: starts at every user's login. Needs admin.
    AllUsers,
    /// `/Library/LaunchDaemons`: starts with the Mac. Needs admin.
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub label: String,
    /// The property list.
    pub plist: PathBuf,
    pub scope: Scope,
    /// What it runs.
    pub program: Option<PathBuf>,
    /// The app it belongs to, when the program sits inside an `.app`.
    pub app: Option<String>,
    /// Loaded right now (launchd knows it).
    pub enabled: bool,
    /// The program has a valid code signature; `None` when there's no program to check.
    pub signed: Option<bool>,
    /// Wrasse may turn it off (your own agent, not Apple's).
    pub can_disable: bool,
}

/// What the OS is asked, so tests can answer for it.
pub trait Probe {
    fn loaded(&self, label: &str) -> bool;
    fn signed(&self, program: &Path) -> Option<bool>;
}

/// The real Mac.
pub struct System;

impl Probe for System {
    fn loaded(&self, label: &str) -> bool {
        crate::platform::launch_job_loaded(label)
    }
    fn signed(&self, program: &Path) -> Option<bool> {
        crate::platform::code_signed(program)
    }
}

/// Every startup item in the three places, yours first, then by name.
pub fn list(home: &Path, places: &[(PathBuf, Scope)], probe: &dyn Probe) -> Vec<StartupItem> {
    let mut out = Vec::new();
    for (dir, scope) in places {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if !path.extension().is_some_and(|x| x == "plist") {
                continue;
            }
            if let Some(item) = read(&path, *scope, home, probe) {
                out.push(item);
            }
        }
    }
    out.sort_by(|a, b| {
        (a.scope != Scope::User, a.label.to_lowercase())
            .cmp(&(b.scope != Scope::User, b.label.to_lowercase()))
    });
    out
}

/// The three places on this Mac.
pub fn places(home: &Path) -> Vec<(PathBuf, Scope)> {
    vec![
        (home.join("Library/LaunchAgents"), Scope::User),
        (PathBuf::from("/Library/LaunchAgents"), Scope::AllUsers),
        (PathBuf::from("/Library/LaunchDaemons"), Scope::System),
    ]
}

fn read(path: &Path, scope: Scope, home: &Path, probe: &dyn Probe) -> Option<StartupItem> {
    let value = plist::Value::from_file(path).ok()?;
    let dict = value.as_dictionary()?;
    let label = dict.get("Label")?.as_string()?.to_string();
    let program = dict
        .get("Program")
        .and_then(|v| v.as_string())
        .or_else(|| {
            dict.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_string())
        })
        .map(PathBuf::from);
    let app = program.as_deref().and_then(app_of);
    let apple = label.starts_with("com.apple.");
    let own =
        scope == Scope::User && path.parent() == Some(home.join("Library/LaunchAgents").as_path());
    Some(StartupItem {
        enabled: probe.loaded(&label),
        signed: program.as_deref().and_then(|p| probe.signed(p)),
        can_disable: own && !apple,
        label,
        plist: path.to_path_buf(),
        scope,
        program,
        app,
    })
}

/// "Docker" for `/Applications/Docker.app/Contents/MacOS/com.docker.helper`.
fn app_of(program: &Path) -> Option<String> {
    program
        .ancestors()
        .filter(|a| a.extension().is_some_and(|x| x == "app"))
        .last()
        .and_then(|a| a.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
}

/// Turns off one of your own agents: unloads it if it's loaded (delegated), then moves its plist
/// to the tray. Dry run reports without doing either.
pub fn disable(ctx: &Context, item: &StartupItem, dry_run: bool) -> io::Result<safety::Report> {
    if !item.can_disable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "only your own startup items can be turned off",
        ));
    }
    if item.enabled {
        safety::delegate(
            ctx,
            Delegated::UnloadLaunchAgent {
                plist: item.plist.clone(),
            },
            dry_run,
        )?;
    }
    let plan = safety::prepare(
        ctx,
        &[Request {
            path: item.plist.clone(),
            safety: Safety::YourCall,
            rule_id: None,
            root: None,
            hand_picked: true,
            scanned: None,
        }],
        Mode::Quarantine,
    );
    safety::remove(ctx, &plan, dry_run)
}

#[cfg(test)]
mod tests;
