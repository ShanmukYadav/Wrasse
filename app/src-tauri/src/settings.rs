//! Settings commands for the UI (`app/src/api/tauri.ts`): tray keep days, the login item, the
//! custom rules folder, revealing a file, and "Remove all Wrasse data".

use crate::cleanup::{self, CleanupState};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use wrasse_core::platform;
use wrasse_core::safety::{self, OwnData};
use wrasse_core::settings::{self, Settings};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    keep_days: u64,
    /// `None` when macOS can't say (a build that isn't an app bundle).
    login_item: Option<bool>,
    /// Where custom junk rules go (`*.toml`).
    rules_folder: PathBuf,
    data_folder: PathBuf,
}

#[tauri::command]
pub fn settings_get(state: State<'_, CleanupState>) -> Result<SettingsView, String> {
    state.with(|ctx| {
        Ok(SettingsView {
            keep_days: settings::load(&ctx.data_dir).keep_days,
            login_item: platform::login_item_enabled(),
            rules_folder: ctx.data_dir.join("rules").join("custom"),
            data_folder: ctx.data_dir.clone(),
        })
    })
}

/// Saves how long batches stay in the tray (clamped to 1–30). Older batches are emptied when
/// Wrasse next starts.
#[tauri::command]
pub fn set_keep_days(state: State<'_, CleanupState>, days: u64) -> Result<u64, String> {
    state.with(|ctx| {
        settings::save(&ctx.data_dir, Settings { keep_days: days })
            .map(|s| s.keep_days)
            .map_err(|e| e.to_string())
    })
}

/// Turns Wrasse's login item on or off and returns the state macOS now reports.
#[tauri::command]
pub fn set_login_item(on: bool) -> Result<Option<bool>, String> {
    platform::set_login_item(on).map_err(|e| e.to_string())?;
    Ok(platform::login_item_enabled())
}

/// Shows a file or folder selected in Finder. Read-only.
#[tauri::command]
pub fn reveal(path: PathBuf) -> Result<(), String> {
    platform::reveal(&path).map_err(|e| e.to_string())
}

/// Quick Look preview of a file (Space). Read-only.
#[tauri::command]
pub fn quick_look(path: PathBuf) -> Result<(), String> {
    platform::quick_look(&path).map_err(|e| e.to_string())
}

/// Creates the custom rules folder if needed and shows it in Finder.
#[tauri::command]
pub fn open_rules_folder(state: State<'_, CleanupState>) -> Result<(), String> {
    let dir = state.with(|ctx| Ok(ctx.data_dir.join("rules").join("custom")))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    platform::reveal(&dir).map_err(|e| e.to_string())
}

/// What "Remove all Wrasse data" would remove. Read-only.
#[tauri::command]
pub fn own_data_preview(state: State<'_, CleanupState>) -> Result<OwnData, String> {
    state.with(|ctx| safety::remove_own_data(ctx, true).map_err(|e| e.to_string()))
}

/// Removes Wrasse's own data (not undoable; the UI confirms first), then sets the safety
/// context up again on the now-empty folder.
#[tauri::command]
pub fn remove_own_data(app: AppHandle, state: State<'_, CleanupState>) -> Result<OwnData, String> {
    let dry_run = state.dry_run();
    let out = state.with(|ctx| safety::remove_own_data(ctx, dry_run).map_err(|e| e.to_string()))?;
    if !dry_run {
        cleanup::init(&app);
    }
    Ok(out)
}
