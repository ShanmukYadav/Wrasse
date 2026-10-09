//! Large & Old and Duplicates (docs/phase3-plan.md §5) for the UI. Both read the last saved scan
//! tree on demand (nothing stays in memory); duplicate removal goes through the core's checks.

use crate::cleanup::CleanupState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use wrasse_core::files::{self, dupes};
use wrasse_core::platform;
use wrasse_core::scan::cache;

/// The running duplicate search, to cancel it.
#[derive(Default)]
pub struct DupesState(Mutex<Option<Arc<dupes::Control>>>);

fn today() -> u32 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        / 86_400) as u32
}

/// Documents, Desktop and iCloud Drive: picked by hand only.
fn hand_pick_places(home: &Path, data: &Path) -> Vec<PathBuf> {
    platform::protected_paths(home, data)
        .into_iter()
        .filter(|p| p.protection == platform::Protection::HandPickOnly)
        .map(|p| p.path)
        .collect()
}

/// The biggest files in the home folder of at least `min_bytes` (at most 500), with last use.
#[tauri::command]
pub async fn large_files(app: AppHandle, min_bytes: u64) -> Result<Vec<files::BigFile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = cache::data_dir(&app.config().identifier).ok_or("no data folder")?;
        let saved = cache::load_tree(&dir).ok_or("scan first")?;
        let home = platform::home_dir().ok_or("no home folder")?;
        let (_, prefix) = platform::scan_root();
        Ok(files::large_files(
            &saved.tree,
            prefix,
            &home,
            min_bytes.max(10_000_000),
            500,
            today(),
            &platform::last_used,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DupesProgress {
    hashed: u64,
    to_hash: u64,
}

/// Byte-identical files of 10 MB or more in the home folder. Emits `dupes-progress` while
/// hashing; `cancel_duplicates` stops it.
#[tauri::command]
pub async fn find_duplicates(
    app: AppHandle,
    state: State<'_, DupesState>,
) -> Result<Vec<dupes::Group>, String> {
    let control = Arc::new(dupes::Control::default());
    if let Ok(mut slot) = state.0.lock() {
        if let Some(old) = slot.replace(control.clone()) {
            old.cancel.store(true, Ordering::Relaxed);
        }
    }
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ticker = {
        let (app, control, done) = (app.clone(), control.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let _ = app.emit(
                    "dupes-progress",
                    DupesProgress {
                        hashed: control.hashed.load(Ordering::Relaxed),
                        to_hash: control.to_hash.load(Ordering::Relaxed),
                    },
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        })
    };
    let worker = control.clone();
    let app2 = app.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<dupes::Group>, String> {
            let dir = cache::data_dir(&app2.config().identifier).ok_or("no data folder")?;
            let saved = cache::load_tree(&dir).ok_or("scan first")?;
            let home = platform::home_dir().ok_or("no home folder")?;
            let (_, prefix) = platform::scan_root();
            let candidates = files::files_over(&saved.tree, prefix, &home, dupes::MIN_BYTES);
            drop(saved);
            let places = hand_pick_places(&home, &dir);
            let hand = |p: &Path| places.iter().any(|d| p.starts_with(d));
            Ok(dupes::find(candidates, &hand, today(), &worker))
        })
        .await;
    done.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    if let Ok(mut slot) = app.state::<DupesState>().0.lock() {
        if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &control)) {
            *slot = None;
        }
    }
    platform::release_free_memory();
    if control.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    result.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_duplicates(state: State<'_, DupesState>) {
    if let Ok(slot) = state.0.lock() {
        if let Some(c) = slot.as_ref() {
            c.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// Moves the chosen copies to the tray, keeping `keep`, after checking every copy is still
/// identical (see `wrasse_core::files::dupes::remove`). Dry run reports only.
#[tauri::command]
pub async fn remove_duplicates(
    app: AppHandle,
    keep: PathBuf,
    remove: Vec<PathBuf>,
) -> Result<wrasse_core::safety::Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<CleanupState>();
        let dry_run = state.dry_run();
        state.with(|ctx| dupes::remove(ctx, &keep, &remove, dry_run).map_err(|e| e.to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
