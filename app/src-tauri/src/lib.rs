//! Thin Tauri wrapper around wrasse-core. All logic lives in the core crate.

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
    }
}

#[tauri::command]
fn system_volume() -> Option<VolumeSpace> {
    platform::system_volume()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch focuses the existing window instead of starting another instance.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![app_info, system_volume])
        .run(tauri::generate_context!())
        .expect("error while running Wrasse");
}
