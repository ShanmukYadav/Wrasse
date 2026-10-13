//! Thin Tauri wrapper around wrasse-core. All logic lives in the core crate.

mod cleanup;
mod files;
mod scanning;

use serde::Serialize;
use tauri::Manager;
use wrasse_core::format::UnitSystem;
use wrasse_core::platform::{self, VolumeSpace};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    platform: &'static str,
    file_manager: &'static str,
    units: UnitSystem,
    dev: bool,
    /// `WRASSE_BENCH=1` asks the UI to run its fps benchmark after the first scan.
    bench: bool,
    /// Phase 1 only reads: cleanup actions are not available yet.
    read_only: bool,
    /// `WRASSE_AUTOSCAN=1` skips onboarding and scans at launch (test runs and benchmarks).
    auto_scan: bool,
}

#[tauri::command]
fn app_info() -> AppInfo {
    let platform = platform::current();
    AppInfo {
        version: wrasse_core::VERSION,
        platform: platform.name(),
        file_manager: platform.file_manager(),
        units: platform.units(),
        dev: cfg!(debug_assertions),
        bench: std::env::var("WRASSE_BENCH").is_ok_and(|v| v == "1"),
        read_only: false,
        // Set only by test runs (dev or release benchmarks); users never see it.
        auto_scan: std::env::var("WRASSE_AUTOSCAN").is_ok_and(|v| v == "1"),
    }
}

#[tauri::command]
fn system_volume() -> Option<VolumeSpace> {
    platform::system_volume()
}

/// Dev builds only: lets the UI print benchmark results in the terminal running `tauri dev`.
#[tauri::command]
fn dev_log(message: String) {
    if cfg!(debug_assertions) {
        println!("{message}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Every audit-log line from this process says it came from the app (a click).
    wrasse_core::safety::audit::set_origin(wrasse_core::safety::audit::Origin::Ui);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch focuses the existing window instead of starting another instance.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(scanning::ScanState::default())
        .manage(cleanup::CleanupState::default())
        .manage(files::DupesState::default())
        .setup(|app| {
            cleanup::init(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            system_volume,
            dev_log,
            scanning::scan,
            scanning::cancel_scan,
            scanning::pause_scan,
            scanning::cached_scan,
            scanning::full_disk_access,
            scanning::open_full_disk_access_settings,
            cleanup::cleanup_status,
            cleanup::set_dry_run,
            cleanup::preview_removal,
            cleanup::preview_models,
            cleanup::remove_items,
            cleanup::remove_models,
            cleanup::loaded_models,
            cleanup::unload_model,
            cleanup::restore_items,
            cleanup::purge_items,
            cleanup::empty_trash,
            cleanup::space_report,
            cleanup::run_delegate,
            cleanup::junk_report,
            cleanup::never_touch,
            cleanup::never_touch_list,
            cleanup::audit_recent,
            cleanup::quit_app,
            cleanup::running_apps,
            cleanup::uninstall_app,
            cleanup::plan_free,
            files::large_files,
            files::find_duplicates,
            files::cancel_duplicates,
            files::remove_duplicates
        ])
        .run(tauri::generate_context!())
        .expect("error while running Wrasse");
}
