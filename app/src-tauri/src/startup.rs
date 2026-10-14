//! Startup Items commands for the UI: the list, and turning off one of your own agents.

use crate::cleanup::CleanupState;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use wrasse_core::platform;
use wrasse_core::startup::{self, StartupItem, System};

/// Everything that starts at login or with the Mac. Read-only.
#[tauri::command]
pub async fn startup_items() -> Result<Vec<StartupItem>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let home = platform::home_dir().ok_or("no home folder")?;
        Ok(startup::list(&home, &startup::places(&home), &System))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Turns off one of your own agents (unload, then its plist to the tray). The item is looked up
/// again here, never trusted from the UI. Dry run reports without acting.
#[tauri::command]
pub async fn disable_startup_item(
    app: AppHandle,
    plist: PathBuf,
) -> Result<wrasse_core::safety::Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let home = platform::home_dir().ok_or("no home folder")?;
        let item = startup::list(&home, &startup::places(&home), &System)
            .into_iter()
            .find(|i| i.plist == plist)
            .ok_or("that startup item isn't there any more")?;
        let state = app.state::<CleanupState>();
        let dry_run = state.dry_run();
        state.with(|ctx| startup::disable(ctx, &item, dry_run).map_err(|e| e.to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
