//! Native glass behind transparent regions of the webview (PLAN.md §7.5, Phase 0.5).
//!
//! The web UI reports the rects of its persistent floating controls; each platform backend
//! places native glass views under the webview at those rects, plus one full-window glass
//! background. The canvas cuts matching transparent holes so the native glass shows through.
//!
//! - macOS 26+: `NSGlassEffectView` (Liquid Glass).
//! - macOS 13–15: `NSVisualEffectView` materials.
//! - Windows (planned for Windows parity): Mica background, Acrylic panels, solid on Windows 10.
//!   Until then it reports `none` and the UI uses CSS glass.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    Top,
    Bottom,
}

/// A rect in CSS pixels (= points) relative to the webview's top-left corner.
#[derive(Debug, Clone, Deserialize)]
pub struct Region {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub radius: f64,
    pub anchor: Anchor,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlassKind {
    /// macOS 26+ Liquid Glass.
    Liquid,
    /// macOS 13–15 visual effect materials.
    Vibrancy,
    /// No native glass; the UI draws CSS glass.
    None,
}

#[derive(Serialize)]
pub struct Support {
    pub kind: GlassKind,
}

/// `WRASSE_GLASS=vibrancy|off` forces a fallback, for testing older-macOS behavior.
fn forced() -> Option<GlassKind> {
    match std::env::var("WRASSE_GLASS").ok()?.as_str() {
        "vibrancy" => Some(GlassKind::Vibrancy),
        "off" | "none" => Some(GlassKind::None),
        _ => None,
    }
}

pub fn support() -> Support {
    #[cfg(target_os = "macos")]
    let detected = macos::detect();
    #[cfg(not(target_os = "macos"))]
    let detected = GlassKind::None;
    let kind = match forced() {
        Some(GlassKind::None) => GlassKind::None,
        Some(GlassKind::Vibrancy) if detected != GlassKind::None => GlassKind::Vibrancy,
        _ => detected,
    };
    Support { kind }
}

/// Adds the full-window glass background. Call once from setup.
pub fn install(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        let kind = support().kind;
        if kind != GlassKind::None {
            let _ = window.with_webview(move |webview| macos::install(webview.inner(), kind));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Replaces the set of glass regions under the webview.
pub fn set_regions(window: &tauri::WebviewWindow, regions: Vec<Region>) {
    #[cfg(target_os = "macos")]
    {
        let kind = support().kind;
        if kind != GlassKind::None {
            let _ = window
                .with_webview(move |webview| macos::set_regions(webview.inner(), kind, &regions));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, regions);
}
