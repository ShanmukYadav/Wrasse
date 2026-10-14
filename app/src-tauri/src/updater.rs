//! Opt-in updates (docs/phase6-plan.md §3). Off until the owner turns it on in Settings: then,
//! at launch, Wrasse asks GitHub Releases for the newest version on its channel (beta builds:
//! pre-releases; stable builds: releases). An update is offered in the window and installs only
//! after a click; every download is checked against the update key's public half built into the
//! app, and a bad signature is refused.
//!
//! The public key comes from the build environment (`WRASSE_UPDATER_PUBKEY`, a repository
//! variable in the release workflow). A build without it can't check for updates.

use crate::cleanup::CleanupState;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const PUBKEY: Option<&str> = option_env!("WRASSE_UPDATER_PUBKEY");
/// "beta" (the default for 0.x builds) or "stable", set by the release workflow.
const CHANNEL: Option<&str> = option_env!("WRASSE_CHANNEL");
const CHANNEL_BASE: &str = "https://github.com/ShanmukYadav/Wrasse/releases/download/updates";

/// The update found at launch, waiting for the click.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Offer {
    version: String,
    notes: Option<String>,
}

/// This build can check for updates.
pub fn available() -> bool {
    PUBKEY.is_some_and(|k| !k.trim().is_empty())
}

/// The updater plugin, when this build has a key.
pub fn plugin() -> Option<tauri::plugin::TauriPlugin<tauri::Wry, tauri_plugin_updater::Config>> {
    let key = PUBKEY.filter(|k| !k.trim().is_empty())?;
    Some(tauri_plugin_updater::Builder::new().pubkey(key).build())
}

fn endpoint() -> String {
    let channel = if CHANNEL == Some("stable") {
        "stable"
    } else {
        "beta"
    };
    format!("{CHANNEL_BASE}/{channel}.json")
}

/// At launch: if the owner turned the check on, look for an update and offer it.
pub fn check_on_launch(app: &AppHandle) {
    if !available() {
        return;
    }
    let on = app
        .state::<CleanupState>()
        .with(|ctx| Ok(wrasse_core::settings::load(&ctx.data_dir).check_updates))
        .unwrap_or(false);
    if !on {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(url) = endpoint().parse() else { return };
        let Ok(updater) = handle
            .updater_builder()
            .endpoints(vec![url])
            .and_then(|b| b.build())
        else {
            return;
        };
        if let Ok(Some(update)) = updater.check().await {
            let offer = Offer {
                version: update.version.clone(),
                notes: update.body.clone(),
            };
            if let Ok(mut slot) = handle.state::<Pending>().0.lock() {
                *slot = Some(update);
            }
            let _ = handle.emit("update-available", offer);
        }
    });
}

/// Downloads the offered update, checks its signature, installs it and restarts Wrasse.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: State<'_, Pending>) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("no update is waiting")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
