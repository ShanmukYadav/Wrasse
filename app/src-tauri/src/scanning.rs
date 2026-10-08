//! Scan commands for the UI (`app/src/api/tauri.ts`). All work happens in wrasse-core; this
//! file only runs it off the main thread and relays progress as `scan-progress` events.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use wrasse_core::platform;
use wrasse_core::scan::breakdown::UiDisk;
use wrasse_core::scan::{cache, ScanControl};
use wrasse_core::session;

/// The scan in progress, if any (for cancel and pause).
#[derive(Default)]
pub struct ScanState(Mutex<Option<Arc<ScanControl>>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    phase: &'static str,
    items: u64,
    bytes: u64,
    current: String,
}

/// Progress events at most 10 times a second (PLAN.md §5.4).
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    cache::data_dir(&app.config().identifier)
}

/// Paths as the user knows them (`/Users/…`, not `/System/Volumes/Data/Users/…`).
fn display(path: &str) -> String {
    let (_, prefix) = platform::scan_root();
    path.strip_prefix(prefix)
        .filter(|p| !p.is_empty())
        .unwrap_or(path)
        .to_string()
}

#[tauri::command]
pub async fn scan(app: AppHandle, state: State<'_, ScanState>) -> Result<UiDisk, String> {
    let control = Arc::new(ScanControl::default());
    {
        let mut current = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(old) = current.take() {
            old.cancel();
        }
        *current = Some(control.clone());
    }

    // Relay progress until the scan finishes.
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ticker = {
        let (app, control, done) = (app.clone(), control.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let p = control.progress();
                let _ = app.emit(
                    "scan-progress",
                    ProgressEvent {
                        phase: "walking",
                        items: p.items,
                        bytes: p.bytes,
                        current: display(&p.current),
                    },
                );
                std::thread::sleep(PROGRESS_EVERY);
            }
        })
    };

    let dir = cache_dir(&app);
    let worker = control.clone();
    let started = std::time::Instant::now();
    let result =
        tauri::async_runtime::spawn_blocking(move || session::scan_volume(&worker, dir.as_deref()))
            .await;
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = ticker.join();

    if let Ok(mut current) = app.state::<ScanState>().0.lock() {
        if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &control)) {
            *current = None;
        }
    }
    let disk = result
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if cfg!(debug_assertions) {
        let s = &disk.summary;
        println!(
            "[scan] {:.2}s files={} dirs={} scanned={} home={} os={} unaccounted={} unreadable={} dataless={} hardlinks={} clones={} uiNodes={} cancelled={}",
            started.elapsed().as_secs_f64(),
            s.files,
            s.dirs,
            s.scanned,
            s.home,
            s.os_used,
            s.unaccounted,
            s.unreadable_dirs,
            s.dataless_items,
            s.hardlinks_deduped,
            s.clone_shared_bytes,
            disk.nodes.len(),
            s.cancelled
        );
    }
    let p = control.progress();
    let phase = if disk.summary.cancelled {
        "cancelled"
    } else {
        "done"
    };
    let _ = app.emit(
        "scan-progress",
        ProgressEvent {
            phase,
            items: p.items,
            bytes: p.bytes,
            current: String::new(),
        },
    );
    Ok(disk)
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, ScanState>) {
    if let Ok(current) = state.0.lock() {
        if let Some(c) = current.as_ref() {
            c.cancel();
        }
    }
}

#[tauri::command]
pub fn pause_scan(state: State<'_, ScanState>, paused: bool) {
    if let Ok(current) = state.0.lock() {
        if let Some(c) = current.as_ref() {
            c.set_paused(paused);
        }
    }
}

#[tauri::command]
pub fn cached_scan(app: AppHandle) -> Option<UiDisk> {
    cache_dir(&app).and_then(|d| session::cached(&d))
}

#[tauri::command]
pub fn full_disk_access() -> bool {
    platform::full_disk_access()
}

#[tauri::command]
pub fn open_full_disk_access_settings() -> Result<(), String> {
    platform::open_full_disk_access_settings().map_err(|e| e.to_string())
}
