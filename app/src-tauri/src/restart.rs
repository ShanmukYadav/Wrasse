//! Quiet restart (docs/phase6-plan.md, D7). WebKit's page process keeps memory after use (idle
//! after use measured 114–140 MB against a 100 MB budget) and nothing inside the page gives it
//! back. So when the window has been in the background for 5 minutes and nothing is going on (no
//! scan, cleanup, duplicate search, open panel, selection or plan, and the map is on the home
//! level), Wrasse starts a fresh copy of itself with the same window frame and exits. The new
//! copy waits for this one to be gone, opens its window in the same place without taking focus,
//! and shows the home map.

use crate::files::DupesState;
use crate::scanning::ScanState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use wrasse_core::platform;

/// The argument a restarted copy gets, followed by the previous pid and the window frame.
pub const RESTART_ARG: &str = "--restarted";

/// The UI says nothing is open or selected (it reports changes through `ui_resting`).
static RESTING: AtomicBool = AtomicBool::new(false);
/// Since when the window has been in the background.
static UNFOCUSED_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

/// How long in the background before a restart. `WRASSE_RESTART_AFTER_SECS` shortens it for the
/// memory check only.
fn after() -> Duration {
    let secs = std::env::var("WRASSE_RESTART_AFTER_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|s| *s >= 5)
        .unwrap_or(300);
    Duration::from_secs(secs)
}

#[tauri::command]
pub fn ui_resting(resting: bool) {
    RESTING.store(resting, Ordering::Relaxed);
}

/// The main window gained or lost focus.
pub fn focus_changed(focused: bool) {
    if let Ok(mut since) = UNFOCUSED_SINCE.lock() {
        *since = if focused {
            None
        } else {
            since.or(Some(Instant::now()))
        };
    }
}

/// Checks every 5 seconds whether it's time to restart.
pub fn watch(app: &AppHandle) {
    let handle = app.clone();
    let _ = std::thread::Builder::new()
        .name("wrasse-restart".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(5));
            if due(&handle) {
                restart(&handle);
                return;
            }
        });
}

fn due(app: &AppHandle) -> bool {
    let away = UNFOCUSED_SINCE
        .lock()
        .ok()
        .and_then(|s| *s)
        .is_some_and(|since| since.elapsed() >= after());
    away && RESTING.load(Ordering::Relaxed)
        && app.get_webview_window("main").is_some()
        && !app.state::<ScanState>().busy()
        && !app.state::<DupesState>().busy()
        && !wrasse_core::safety::busy()
}

fn restart(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    let pos = pos.to_logical::<f64>(scale);
    let size = size.to_logical::<f64>(scale);
    let pid = std::process::id().to_string();
    let frame = format!(
        "{:.0},{:.0},{:.0},{:.0}",
        pos.x, pos.y, size.width, size.height
    );
    let dry_run = app.state::<crate::cleanup::CleanupState>().dry_run();
    let mut args = vec![RESTART_ARG, &pid, &frame];
    if dry_run {
        args.push("--dry-run");
    }
    if platform::start_self(&args).is_ok() {
        app.exit(0);
    }
}

/// For a restarted copy: the previous pid, the window frame (logical x, y, width, height) and
/// whether dry run was on.
pub struct Restarted {
    pub frame: (f64, f64, f64, f64),
    pub dry_run: bool,
}

/// Reads the restart arguments and waits (up to 10 s) for the previous copy to exit, so the
/// single-instance lock is free.
pub fn restarted() -> Option<Restarted> {
    let args: Vec<String> = std::env::args().collect();
    let at = args.iter().position(|a| a == RESTART_ARG)?;
    let pid: u32 = args.get(at + 1)?.parse().ok()?;
    let parts: Vec<f64> = args
        .get(at + 2)?
        .split(',')
        .filter_map(|p| p.parse().ok())
        .collect();
    let [x, y, w, h] = parts[..] else {
        return None;
    };
    platform::wait_for_exit(pid, Duration::from_secs(10));
    Some(Restarted {
        frame: (x, y, w, h),
        dry_run: args.iter().any(|a| a == "--dry-run"),
    })
}
