//! Cleanup commands for the UI (`app/src/api/tauri.ts`). Every removal goes through
//! `wrasse_core::safety`; this file only holds the state and turns UI requests into safety calls.
//!
//! Dry-run: dev builds start every launch in dry-run (nothing is moved or deleted; the UI shows
//! what would happen). It can be switched off from Settings; the DEV badge then turns into LIVE.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use wrasse_core::platform;
use wrasse_core::quarantine;
use wrasse_core::safety::{self, Context, Delegated, Mode, Request, Safety};
use wrasse_core::scan::cache;
use wrasse_core::session;

pub struct CleanupState {
    ctx: Mutex<Option<Context>>,
    dry_run: AtomicBool,
}

impl Default for CleanupState {
    fn default() -> Self {
        // Turns back on at every launch of a dev build (PLAN.md §4.3 rule 13).
        CleanupState {
            ctx: Mutex::new(None),
            dry_run: AtomicBool::new(cfg!(debug_assertions)),
        }
    }
}

/// Sets up the safety context once: finishes any batch a crash left half-done, purges quarantine
/// older than the keep period, and keeps Wrasse's data folder out of backups and Spotlight.
pub fn init(app: &AppHandle) {
    let Some(data) = cache::data_dir(&app.config().identifier) else {
        return;
    };
    let Some(home) = platform::home_dir() else {
        return;
    };
    match Context::new(home, data.clone()) {
        Ok(ctx) => {
            let _ = ctx.quarantine.reconcile();
            let _ = ctx
                .quarantine
                .purge_older_than(quarantine::DEFAULT_KEEP_DAYS);
            let _ = platform::exclude_from_backups(&data);
            if let Ok(mut slot) = app.state::<CleanupState>().ctx.lock() {
                *slot = Some(ctx);
            }
        }
        Err(e) => eprintln!("[cleanup] couldn't set up safety: {e}"),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRequest {
    path: PathBuf,
    /// "safe" or "yourCall"
    safety: String,
    #[serde(default)]
    rule_id: Option<String>,
    #[serde(default)]
    rule_root: Option<PathBuf>,
    #[serde(default)]
    hand_picked: bool,
}

impl ItemRequest {
    fn into_request(self) -> Request {
        Request {
            path: self.path,
            safety: if self.safety == "safe" {
                Safety::Safe
            } else {
                Safety::YourCall
            },
            rule_id: self.rule_id,
            root: self.rule_root,
            hand_picked: self.hand_picked,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    bytes: u64,
    undoable: bool,
    synced: Vec<PathBuf>,
    skipped: Vec<safety::Skipped>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayEntry {
    id: String,
    name: String,
    path: PathBuf,
    bytes: u64,
    at: u64,
    rule_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    dry_run: bool,
    dev: bool,
    tray: Vec<TrayEntry>,
    log_path: Option<PathBuf>,
}

fn with_ctx<T>(
    state: &State<'_, CleanupState>,
    f: impl FnOnce(&mut Context) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state.ctx.lock().map_err(|e| e.to_string())?;
    let ctx = guard
        .as_mut()
        .ok_or("cleanup isn't available (no data folder)")?;
    f(ctx)
}

fn tray(ctx: &Context) -> Vec<TrayEntry> {
    ctx.quarantine
        .stored()
        .into_iter()
        .map(|(b, e)| TrayEntry {
            name: e
                .original
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: e.original,
            bytes: e.bytes,
            at: b.created_at,
            rule_id: e.rule_id,
            id: e.id,
        })
        .collect()
}

#[tauri::command]
pub fn cleanup_status(state: State<'_, CleanupState>) -> Result<Status, String> {
    with_ctx(&state, |ctx| {
        Ok(Status {
            dry_run: state.dry_run.load(Ordering::Relaxed),
            dev: cfg!(debug_assertions),
            tray: tray(ctx),
            log_path: Some(ctx.audit.path().to_path_buf()),
        })
    })
}

#[tauri::command]
pub fn set_dry_run(state: State<'_, CleanupState>, on: bool) {
    state.dry_run.store(on, Ordering::Relaxed);
}

/// What removing these would do: bytes, iCloud-synced items, what would be skipped and why.
#[tauri::command]
pub fn preview_removal(
    state: State<'_, CleanupState>,
    items: Vec<ItemRequest>,
    delete_now: bool,
) -> Result<Preview, String> {
    with_ctx(&state, |ctx| {
        let reqs: Vec<Request> = items.into_iter().map(ItemRequest::into_request).collect();
        let plan = safety::prepare(
            ctx,
            &reqs,
            if delete_now {
                Mode::DeleteNow
            } else {
                Mode::Quarantine
            },
        );
        Ok(Preview {
            bytes: plan.bytes,
            undoable: plan.undoable,
            synced: plan
                .items
                .iter()
                .filter(|i| i.warnings.contains(&safety::Warning::SyncedToICloud))
                .map(|i| i.request.path.clone())
                .collect(),
            skipped: plan.skipped,
        })
    })
}

/// Quarantines (or, with `delete_now`, deletes) the items. Dry-run reports without acting.
#[tauri::command]
pub fn remove_items(
    state: State<'_, CleanupState>,
    items: Vec<ItemRequest>,
    delete_now: bool,
) -> Result<safety::Report, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        let reqs: Vec<Request> = items.into_iter().map(ItemRequest::into_request).collect();
        let plan = safety::prepare(
            ctx,
            &reqs,
            if delete_now {
                Mode::DeleteNow
            } else {
                Mode::Quarantine
            },
        );
        safety::remove(ctx, &plan, dry_run).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn restore_items(
    state: State<'_, CleanupState>,
    ids: Vec<String>,
) -> Result<Vec<(String, PathBuf)>, String> {
    with_ctx(&state, |ctx| {
        safety::restore(ctx, &ids).map_err(|e| e.to_string())
    })
}

/// Empties quarantine entries for good. Not undoable.
#[tauri::command]
pub fn purge_items(state: State<'_, CleanupState>, ids: Vec<String>) -> Result<u64, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        safety::purge(ctx, &ids, dry_run).map_err(|e| e.to_string())
    })
}

/// Empties the Trash through the OS. Not undoable.
#[tauri::command]
pub fn empty_trash(state: State<'_, CleanupState>) -> Result<u64, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        safety::delegate(ctx, Delegated::EmptyTrash, dry_run).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn junk_report(app: AppHandle) -> Result<session::JunkReport, String> {
    let dir = cache::data_dir(&app.config().identifier).ok_or("no data folder")?;
    session::junk(&dir).ok_or_else(|| "no home folder".into())
}

#[tauri::command]
pub fn never_touch(
    state: State<'_, CleanupState>,
    path: PathBuf,
    on: bool,
) -> Result<Vec<PathBuf>, String> {
    with_ctx(&state, |ctx| {
        if on {
            ctx.exclusions.add(path).map_err(|e| e.to_string())?;
        } else {
            ctx.exclusions.remove(&path).map_err(|e| e.to_string())?;
        }
        Ok(ctx.exclusions.paths.clone())
    })
}

#[tauri::command]
pub fn never_touch_list(state: State<'_, CleanupState>) -> Result<Vec<PathBuf>, String> {
    with_ctx(&state, |ctx| Ok(ctx.exclusions.paths.clone()))
}

#[tauri::command]
pub fn audit_recent(state: State<'_, CleanupState>) -> Result<Vec<serde_json::Value>, String> {
    with_ctx(&state, |ctx| Ok(ctx.audit.recent(500)))
}
