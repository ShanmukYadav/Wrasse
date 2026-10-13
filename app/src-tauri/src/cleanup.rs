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
use wrasse_core::apps::uninstall;
use wrasse_core::llm::{self, ModelJob, ModelRemoval, Probe};
use wrasse_core::platform;
use wrasse_core::quarantine;
use wrasse_core::rank;
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
    /// Size and modification time from the scan (Large & Old). Absent for other removals.
    #[serde(default)]
    scanned: Option<safety::Scanned>,
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
            scanned: self.scanned,
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
    /// The batch it was moved in (one delete = one batch), for "Restore all" per batch.
    batch: String,
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
    /// Days a batch stays in the tray before it's emptied automatically.
    keep_days: u64,
    log_path: Option<PathBuf>,
}

impl CleanupState {
    pub fn dry_run(&self) -> bool {
        self.dry_run.load(Ordering::Relaxed)
    }

    /// Runs `f` with the safety context (for other modules' commands).
    pub fn with<T>(&self, f: impl FnOnce(&mut Context) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.ctx.lock().map_err(|e| e.to_string())?;
        let ctx = guard
            .as_mut()
            .ok_or("cleanup isn't available (no data folder)")?;
        f(ctx)
    }
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
            batch: b.id,
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
            keep_days: quarantine::DEFAULT_KEEP_DAYS,
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

/// What removing these models would do. Always dry: a full duplicate check may read the weights,
/// and nothing is moved. An Ollama removal is not undoable when the local server answers.
#[tauri::command]
pub fn preview_models(
    state: State<'_, CleanupState>,
    jobs: Vec<ModelJob>,
) -> Result<Preview, String> {
    with_ctx(&state, |ctx| {
        let probe = llm::Live;
        let reachable = probe.reachable();
        let mut bytes = 0u64;
        let mut undoable = true;
        let mut skipped = Vec::new();
        for job in &jobs {
            let one = llm::remove_model_with(ctx, job, true, &probe).map_err(|e| e.to_string())?;
            bytes += one.bytes;
            skipped.extend(one.skipped);
            if job.removal == ModelRemoval::Ollama && reachable && !one.done.is_empty() {
                undoable = false;
            }
        }
        Ok(Preview {
            bytes,
            undoable,
            synced: Vec::new(),
            skipped,
        })
    })
}

/// Removes models. Ollama goes through its server when it answers; everything else is quarantined.
#[tauri::command]
pub fn remove_models(
    state: State<'_, CleanupState>,
    jobs: Vec<ModelJob>,
) -> Result<safety::Report, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        let probe = llm::Live;
        let mut report = safety::Report {
            dry_run,
            ..Default::default()
        };
        for job in &jobs {
            let one =
                llm::remove_model_with(ctx, job, dry_run, &probe).map_err(|e| e.to_string())?;
            report.bytes += one.bytes;
            report.done.extend(one.done);
            report.skipped.extend(one.skipped);
        }
        Ok(report)
    })
}

/// Models Ollama says are loaded, plus LM Studio's process memory. Read-only.
#[tauri::command]
pub fn loaded_models() -> Vec<llm::LoadedModel> {
    llm::loaded_models()
}

/// Asks Ollama to unload a model. Returns whether this was a dry run.
#[tauri::command]
pub fn unload_model(state: State<'_, CleanupState>, name: String) -> Result<bool, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        llm::unload_model(ctx, &name, dry_run).map_err(|e| e.to_string())?;
        Ok(dry_run)
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

/// Free space, purgeable space, local snapshots, Trash and the tray. Read-only.
/// `before` plus `freed_now` is what a cleanup was expected to leave free.
#[tauri::command]
pub fn space_report(
    state: State<'_, CleanupState>,
    before: Option<u64>,
    freed_now: u64,
) -> Result<wrasse_core::diagnostics::Report, String> {
    with_ctx(&state, |ctx| {
        let tray = quarantine::size_on_disk(&ctx.data_dir.join("Quarantine"));
        let report = wrasse_core::diagnostics::measure(&ctx.home, tray);
        Ok(match before {
            Some(b) => wrasse_core::diagnostics::with_expectation(report, b, freed_now),
            None => report,
        })
    })
}

/// One delegated cleanup: snapshots, Homebrew, Docker, unavailable simulators, or the Trash.
/// The byte count for snapshots is measured here. Dev builds stay in dry run.
#[tauri::command]
pub fn run_delegate(state: State<'_, CleanupState>, action: String) -> Result<u64, String> {
    let dry_run = state.dry_run.load(Ordering::Relaxed);
    with_ctx(&state, |ctx| {
        let delegated = match action.as_str() {
            "emptyTrash" => Delegated::EmptyTrash,
            "thinSnapshots" => {
                let report = wrasse_core::diagnostics::measure(&ctx.home, 0);
                let bytes = wrasse_core::diagnostics::thin_request(
                    report.snapshots.len(),
                    report.purgeable,
                );
                Delegated::ThinLocalSnapshots { bytes }
            }
            "brewCleanup" => Delegated::BrewCleanup,
            "dockerPrune" => Delegated::DockerPrune,
            "deleteUnavailableSimulators" => Delegated::SimctlDeleteUnavailable,
            _ => return Err("that action isn't available".into()),
        };
        safety::delegate(ctx, delegated, dry_run).map_err(|e| e.to_string())
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuitResult {
    /// The app is no longer running.
    quit: bool,
}

/// Which of `bundles` are running right now. The UI asks when the window comes back into view, so
/// an app opened or quit elsewhere shows its real state. Read-only.
#[tauri::command]
pub async fn running_apps(bundles: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    tauri::async_runtime::spawn_blocking(move || uninstall::running_of(bundles))
        .await
        .map_err(|e| e.to_string())
}

/// Asks the app at `bundle` to quit (never forces it) and waits up to 10 s for it to close.
#[tauri::command]
pub async fn quit_app(bundle: PathBuf) -> Result<QuitResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let home = platform::home_dir().ok_or("no home folder")?;
        let app = platform::app_at(&home, &bundle).ok_or("that app isn't installed")?;
        let quit = uninstall::quit_and_wait(&app, std::time::Duration::from_secs(10))
            .map_err(|e| e.to_string())?;
        Ok(QuitResult { quit })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Uninstalls the app at `bundle` (looked up again here, never trusted from the UI) and sends the
/// chosen leftovers to the tray. The route (tray, Homebrew, Finder) follows the app's source and
/// owner; see `wrasse_core::apps::uninstall`. Dry run reports without acting.
#[tauri::command]
pub async fn uninstall_app(
    app: AppHandle,
    bundle: PathBuf,
    leftovers: Vec<PathBuf>,
) -> Result<uninstall::Outcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<CleanupState>();
        let dry_run = state.dry_run.load(Ordering::Relaxed);
        let home = platform::home_dir().ok_or("no home folder")?;
        let found = platform::app_at(&home, &bundle).ok_or("that app isn't installed any more")?;
        let running = uninstall::runs_from(&found.bundle, &platform::running_executables());
        with_ctx(&state, |ctx| {
            uninstall::uninstall(ctx, &found, &leftovers, running, dry_run)
                .map_err(|e| e.to_string())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Free Up Space: what to remove to free `target` bytes, best first (`wrasse_core::rank`). Ranks
/// the last saved scan (the one the UI shows), minus `gone` (nodes already removed this session),
/// never offering Documents, Desktop, iCloud Drive or Never touch items. Read-only.
#[tauri::command]
pub fn plan_free(
    app: AppHandle,
    state: State<'_, CleanupState>,
    target: u64,
    gone: Vec<u32>,
) -> Result<rank::Plan, String> {
    let dir = cache::data_dir(&app.config().identifier).ok_or("no data folder")?;
    let (disk, _) = cache::load(&dir).ok_or("scan first")?;
    let home = platform::home_dir().ok_or("no home folder")?;
    let never_touch = with_ctx(&state, |ctx| Ok(ctx.exclusions.paths.clone()))?;
    let limits = rank::Limits {
        gone: gone.into_iter().collect(),
        hand_pick_only: platform::protected_paths(&home, &dir)
            .into_iter()
            .filter(|p| p.protection == platform::Protection::HandPickOnly)
            .map(|p| p.path)
            .collect(),
        never_touch,
    };
    Ok(rank::plan(&disk, target, &limits))
}
