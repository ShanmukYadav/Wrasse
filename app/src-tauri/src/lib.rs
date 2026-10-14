//! Thin Tauri wrapper around wrasse-core. All logic lives in the core crate.

mod cleanup;
mod files;
mod menubar;
mod restart;
mod scanning;
mod settings;
mod startup;
mod updater;

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
    /// `WRASSE_SCRIPTED_USE` (memory checks only): after the scan, the UI drills into two tiles,
    /// opens the tray and Settings, changes one setting, and closes everything again. `1` is all
    /// of it; a comma list (`drill`, `tray`, `settings`) picks steps.
    scripted_use: Option<String>,
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
        scripted_use: std::env::var("WRASSE_SCRIPTED_USE")
            .ok()
            .filter(|v| !v.is_empty()),
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
    // Menu-bar-only Wrasse: no window and no web view (so no WebKit processes), only the menu.
    let menu_bar_only = std::env::args().any(|a| a == menubar::MENU_BAR_ARG)
        || std::env::var("WRASSE_MENU_BAR_ONLY").is_ok_and(|v| v == "1");
    // A quiet restart (restart.rs): wait for the previous copy to exit, then reopen its window.
    let restarted = restart::restarted();
    let frame = restarted.as_ref().map(|r| r.frame);
    let restarted_dry_run = restarted.as_ref().is_some_and(|r| r.dry_run);
    let mut builder = tauri::Builder::default();
    if !menu_bar_only {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch opens the window instead of starting another instance.
            menubar::open_window(app);
        }));
    }
    if let Some(updater) = updater::plugin() {
        builder = builder.plugin(updater);
    }
    let result = builder
        .manage(updater::Pending::default())
        .manage(scanning::ScanState::default())
        .manage(cleanup::CleanupState::default())
        .manage(files::DupesState::default())
        .setup(move |app| {
            cleanup::init(app.handle());
            menubar::install(app.handle())?;
            if menu_bar_only {
                menubar::start_menu_bar_only(app.handle());
            } else {
                menubar::stop_menu_bar_copy(app.handle());
                if restarted_dry_run {
                    app.state::<cleanup::CleanupState>().set_dry_run(true);
                }
                match frame {
                    Some(frame) => menubar::reopen_window(app.handle(), frame),
                    None => menubar::open_window(app.handle()),
                }
                restart::watch(app.handle());
                updater::check_on_launch(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { .. } => menubar::closing_to_menu_bar(),
            tauri::WindowEvent::Focused(focused) => restart::focus_changed(*focused),
            tauri::WindowEvent::Destroyed => menubar::window_closed(window.app_handle()),
            _ => {}
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
            files::remove_duplicates,
            settings::settings_get,
            settings::set_keep_days,
            settings::set_login_item,
            settings::reveal,
            settings::quick_look,
            settings::export_table,
            settings::open_rules_folder,
            settings::own_data_preview,
            settings::remove_own_data,
            startup::startup_items,
            startup::disable_startup_item,
            restart::ui_resting,
            settings::set_check_updates,
            updater::install_update
        ])
        .build(tauri::generate_context!());
    let app = match result {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Wrasse couldn't start: {e}");
            std::process::exit(1);
        }
    };
    app.run(|app, event| match event {
        // The last window closing doesn't quit: Wrasse hands over to its menu-bar-only copy
        // (menubar.rs) and exits itself. Quit (menu or ⌘Q) quits.
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => menubar::open_window(app),
        _ => {}
    });
}
