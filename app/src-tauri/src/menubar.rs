//! The menu bar item (PLAN.md §6.12): a native menu, no web view. It shows free space (with a
//! low-disk warning under 10%), what grew since the last scan, Scan now, Open Wrasse and Quit.
//!
//! Closing the window leaves Wrasse in the menu bar. WebKit keeps its helper processes (about
//! 90 MB) for as long as the app that used a web view runs, so the app hands over instead: once
//! no scan or cleanup is running it starts a menu-bar-only copy of itself (`--menu-bar`: no
//! window, no web view, no Dock icon) and exits. Open Wrasse in that copy starts the full app
//! again and exits; the full app also stops a leftover copy when it starts. Expired tray batches
//! are emptied from the menu bar too, every 10 minutes.

use crate::cleanup::CleanupState;
use crate::scanning::ScanState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder, Wry};
use wrasse_core::format::{format_bytes, UnitSystem};
use wrasse_core::platform;
use wrasse_core::scan::{cache, ScanControl};
use wrasse_core::session;

const TRAY_ID: &str = "wrasse";
/// How often the menu re-reads free space and empties expired tray batches.
const REFRESH: Duration = Duration::from_secs(60 * 10);
/// Below this share of free space the menu warns.
const LOW_DISK: f64 = 0.10;

static SCANNING: AtomicBool = AtomicBool::new(false);
/// This process is the menu-bar-only copy.
static MENU_BAR_ONLY: AtomicBool = AtomicBool::new(false);
/// The user closed the window (rather than quitting).
static CLOSING: AtomicBool = AtomicBool::new(false);

/// The argument that starts the menu-bar-only copy.
pub const MENU_BAR_ARG: &str = "--menu-bar";
const PID_FILE: &str = "menubar.pid";

/// Adds the menu bar item and starts its refresh loop.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Wrasse")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => open_window(app),
            "scan" => scan_now(app),
            "quit" => quit(app),
            _ => {}
        })
        .build(app)?;
    let handle = app.clone();
    std::thread::Builder::new()
        .name("wrasse-menubar".into())
        .spawn(move || loop {
            purge_expired(&handle);
            refresh(&handle);
            std::thread::sleep(REFRESH);
        })?;
    Ok(())
}

/// Rebuilds the menu with current numbers.
pub fn refresh(app: &AppHandle) {
    let Ok(menu) = build_menu(app) else { return };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn units() -> UnitSystem {
    platform::current().units()
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    let space = platform::system_volume();
    let free_line = match space {
        Some(v) if v.total > 0 => {
            let low = (v.available as f64) < (v.total as f64) * LOW_DISK;
            let free = format_bytes(v.available, units());
            if low {
                format!("Low on space: {free} free")
            } else {
                format!("{free} free on {}", platform::current().name())
            }
        }
        _ => "Free space unknown".to_string(),
    };
    menu.append(&MenuItem::with_id(
        app,
        "free",
        free_line,
        false,
        None::<&str>,
    )?)?;
    let grew = grew_lines(app);
    if !grew.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(
            app,
            "grew-title",
            "Grew since the last scan",
            false,
            None::<&str>,
        )?)?;
        for (i, line) in grew.into_iter().enumerate() {
            menu.append(&MenuItem::with_id(
                app,
                format!("grew-{i}"),
                line,
                false,
                None::<&str>,
            )?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let scanning = SCANNING.load(Ordering::Relaxed);
    let scan_label = if scanning { "Scanning…" } else { "Scan now" };
    menu.append(&MenuItem::with_id(
        app,
        "scan",
        scan_label,
        !scanning,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "open",
        "Open Wrasse",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Wrasse",
        true,
        Some("CmdOrCtrl+Q"),
    )?)?;
    Ok(menu)
}

/// The three folders that grew most since the previous scan, from the saved scan.
fn grew_lines(app: &AppHandle) -> Vec<String> {
    let Some(dir) = cache::data_dir(&app.config().identifier) else {
        return Vec::new();
    };
    let Some(disk) = session::cached(&dir) else {
        return Vec::new();
    };
    let mut grew: Vec<(i64, String)> = disk
        .nodes
        .iter()
        .filter(|n| n.grew_by > 0 && n.depth >= 2)
        .map(|n| (n.grew_by, n.name.clone()))
        .collect();
    grew.sort_by_key(|g| std::cmp::Reverse(g.0));
    grew.into_iter()
        .take(3)
        .map(|(by, name)| format!("{name}  +{}", format_bytes(by.unsigned_abs(), units())))
        .collect()
}

/// Runs as the menu-bar-only copy: no Dock icon, and a pid file so the full app can stop it.
pub fn start_menu_bar_only(app: &AppHandle) {
    MENU_BAR_ONLY.store(true, Ordering::Relaxed);
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    if let Some(dir) = cache::data_dir(&app.config().identifier) {
        let _ = std::fs::write(dir.join(PID_FILE), std::process::id().to_string());
    }
}

/// The full app is starting: a menu-bar-only copy still running is stopped (one menu bar item).
pub fn stop_menu_bar_copy(app: &AppHandle) {
    let Some(dir) = cache::data_dir(&app.config().identifier) else {
        return;
    };
    let pid = std::fs::read_to_string(dir.join(PID_FILE))
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok());
    if let Some(pid) = pid {
        platform::stop_own_copy(pid);
    }
}

/// The user is closing the window (not quitting): hand over to the menu bar once it's gone.
pub fn closing_to_menu_bar() {
    CLOSING.store(true, Ordering::Relaxed);
}

/// Shows the window, creating it (with its web view) if needed. In the menu-bar-only copy it
/// starts the full app instead and exits.
pub fn open_window(app: &AppHandle) {
    if MENU_BAR_ONLY.load(Ordering::Relaxed) {
        if platform::start_self(&[]).is_ok() {
            app.exit(0);
        }
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let Some(config) = app.config().app.windows.first() else {
        return;
    };
    if let Ok(window) = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
        let _ = window.set_focus();
    }
}

/// The window is gone. If the user closed it, hand over to the menu-bar-only copy once no scan
/// or cleanup is running, then exit.
pub fn window_closed(app: &AppHandle) {
    if !CLOSING.swap(false, Ordering::Relaxed) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let handle = app.clone();
    let _ = std::thread::Builder::new()
        .name("wrasse-handover".into())
        .spawn(move || {
            while handle.state::<ScanState>().busy() || wrasse_core::safety::busy() {
                std::thread::sleep(Duration::from_millis(500));
            }
            // Reopened meanwhile: stay.
            if handle.get_webview_window("main").is_some() {
                return;
            }
            if platform::start_self(&[MENU_BAR_ARG]).is_ok() {
                handle.exit(0);
            }
        });
}

/// Scan now: the open window runs its own scan (so the map updates); with the window closed the
/// core scans in the background and saves the result for next time.
fn scan_now(app: &AppHandle) {
    if app.get_webview_window("main").is_some() {
        let _ = app.emit("menu-scan", ());
        return;
    }
    if SCANNING.swap(true, Ordering::Relaxed) {
        return;
    }
    refresh(app);
    let handle = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wrasse-menubar-scan".into())
        .spawn(move || {
            let dir = cache::data_dir(&handle.config().identifier);
            let control = ScanControl::default();
            let _ = session::scan_volume(&control, dir.as_deref());
            SCANNING.store(false, Ordering::Relaxed);
            refresh(&handle);
        });
    if spawned.is_err() {
        SCANNING.store(false, Ordering::Relaxed);
    }
}

/// Empties tray batches older than the keep period (also done at every launch).
fn purge_expired(app: &AppHandle) {
    let state = app.state::<CleanupState>();
    let _ = state.with(|ctx| {
        let days = wrasse_core::settings::load(&ctx.data_dir).keep_days;
        ctx.quarantine
            .purge_older_than(days)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
}

fn quit(app: &AppHandle) {
    if MENU_BAR_ONLY.load(Ordering::Relaxed) {
        if let Some(dir) = cache::data_dir(&app.config().identifier) {
            let _ = std::fs::write(dir.join(PID_FILE), "");
        }
    }
    app.exit(0);
}
