//! Uninstalling an app (docs/history/phase3-plan.md §3). The app is looked up again on disk right before
//! acting (never trusted from the UI), every step goes through `safety`, and nothing runs while
//! the app is open.

use super::{App, Owner, Source};
use crate::safety::{self, Context, Delegated, Mode, Reason, Request, Safety};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// How the bundle itself goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Route {
    /// Wrasse moves it to the tray. Restorable.
    Tray,
    /// `brew uninstall --cask <token>`. Not undoable.
    Homebrew { token: String },
    /// Finder moves it to the user's Trash (asks for the password). Put Back works.
    FinderTrash,
    /// Part of the OS.
    NotRemovable,
}

pub fn route(app: &App) -> Route {
    match (&app.source, app.owner) {
        (Source::System, _) => Route::NotRemovable,
        (Source::Cask { token }, _) => Route::Homebrew {
            token: token.clone(),
        },
        (_, Owner::Root) => Route::FinderTrash,
        (_, Owner::User) => Route::Tray,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub route: Route,
    /// The bundle was removed (or, in dry run, would be).
    pub bundle_removed: bool,
    pub bundle_bytes: u64,
    /// Tray entry for the bundle (route Tray).
    pub bundle_entry: Option<String>,
    /// Why the bundle stayed, when it did.
    pub bundle_skipped: Option<Reason>,
    pub leftovers: safety::Report,
    pub dry_run: bool,
}

/// Removes `app` and the chosen leftovers. `leftovers` must be leftovers discovery found for this
/// app (others are ignored), and never ones that need admin rights. `running` tells whether a
/// process runs from the bundle (checked again here, right before acting).
pub fn uninstall(
    ctx: &Context,
    app: &App,
    leftovers: &[PathBuf],
    running: bool,
    dry_run: bool,
) -> io::Result<Outcome> {
    let route = route(app);
    if route == Route::NotRemovable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is part of the system", app.name),
        ));
    }
    if running {
        return Err(io::Error::other(format!(
            "{} is open; quit it first",
            app.name
        )));
    }
    let mut out = Outcome {
        route: route.clone(),
        bundle_removed: false,
        bundle_bytes: 0,
        bundle_entry: None,
        bundle_skipped: None,
        leftovers: safety::Report::default(),
        dry_run,
    };
    match &route {
        Route::Tray => {
            let req = Request {
                path: app.bundle.clone(),
                safety: Safety::YourCall,
                rule_id: None,
                root: None,
                hand_picked: true,
                scanned: None,
            };
            let plan = safety::prepare(ctx, &[req], Mode::Quarantine);
            let report = safety::remove(ctx, &plan, dry_run)?;
            if let Some(d) = report.done.first() {
                out.bundle_removed = true;
                out.bundle_bytes = d.bytes;
                out.bundle_entry = d.entry.clone();
            } else {
                out.bundle_skipped = report.skipped.first().map(|s| s.reason);
            }
        }
        Route::Homebrew { token } => {
            let action = Delegated::BrewUninstallCask {
                token: token.clone(),
                app: app.bundle.clone(),
            };
            out.bundle_bytes = safety::delegate(ctx, action, dry_run)?;
            out.bundle_removed = true;
        }
        Route::FinderTrash => {
            let action = Delegated::FinderMoveToTrash {
                path: app.bundle.clone(),
            };
            out.bundle_bytes = safety::delegate(ctx, action, dry_run)?;
            out.bundle_removed = true;
        }
        Route::NotRemovable => unreachable!(),
    }
    // Leftovers go to the tray even if the bundle stayed (the user ticked them).
    let requests: Vec<Request> = app
        .leftovers
        .iter()
        .filter(|l| !l.needs_admin && leftovers.iter().any(|p| p == &l.path))
        .map(|l| Request {
            path: l.path.clone(),
            safety: if l.kind.regenerates() {
                Safety::Safe
            } else {
                Safety::YourCall
            },
            rule_id: None,
            root: None,
            hand_picked: true,
            scanned: None,
        })
        .collect();
    if !requests.is_empty() {
        let plan = safety::prepare(ctx, &requests, Mode::Quarantine);
        out.leftovers = safety::remove(ctx, &plan, dry_run)?;
    }
    Ok(out)
}

/// Whether the app at `bundle` is running, given `running` (executable paths). Its extensions
/// and helpers don't count (see [`crate::platform::is_app_process`]).
pub fn runs_from(bundle: &Path, running: &[PathBuf]) -> bool {
    running
        .iter()
        .any(|p| crate::platform::is_app_process(bundle, p))
}
