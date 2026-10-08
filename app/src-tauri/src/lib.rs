//! Thin Tauri wrapper around wrasse-core. All logic lives in the core crate.

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    platform: &'static str,
    file_manager: &'static str,
    window_effects: bool,
    dev: bool,
}

#[tauri::command]
fn app_info() -> AppInfo {
    let platform = wrasse_core::platform::current();
    AppInfo {
        version: wrasse_core::VERSION,
        platform: platform.name(),
        file_manager: platform.file_manager(),
        window_effects: platform.window_effects(),
        dev: cfg!(debug_assertions),
    }
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
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window is defined in tauri.conf.json");
            if wrasse_core::platform::current().window_effects() {
                apply_window_effects(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Wrasse");
}

/// Sidebar vibrancy on macOS, Mica on Windows 11 (PLAN.md §7.5).
/// Only called when the platform supports it; otherwise the UI paints a solid sidebar.
fn apply_window_effects(window: &tauri::WebviewWindow) {
    use tauri::window::{Effect, EffectState, EffectsBuilder};

    #[cfg(target_os = "macos")]
    let effects = EffectsBuilder::new()
        .effect(Effect::Sidebar)
        .state(EffectState::FollowsWindowActiveState)
        .build();
    #[cfg(windows)]
    let effects = EffectsBuilder::new().effect(Effect::Mica).build();
    #[cfg(not(any(target_os = "macos", windows)))]
    let effects = EffectsBuilder::new().build();

    let _ = window.set_effects(effects);
}
