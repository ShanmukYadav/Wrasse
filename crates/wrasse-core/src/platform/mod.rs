//! Platform abstraction (PLAN.md §3). All OS-specific code lives under this module.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use crate::format::UnitSystem;

pub trait Platform: Send + Sync {
    /// Human-readable OS name for labels, e.g. "macOS".
    fn name(&self) -> &'static str;
    /// The file manager's name, used in "Reveal in …" labels.
    fn file_manager(&self) -> &'static str;
    fn units(&self) -> UnitSystem;
    /// Whether the native window backdrop (vibrancy on macOS, Mica on Windows 11) is available.
    /// Without it the UI paints a solid sidebar.
    fn window_effects(&self) -> bool;
}

pub fn current() -> &'static dyn Platform {
    #[cfg(target_os = "macos")]
    {
        &macos::MacOs
    }
    #[cfg(windows)]
    {
        &windows::Windows
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        &Unsupported
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
struct Unsupported;

#[cfg(not(any(target_os = "macos", windows)))]
impl Platform for Unsupported {
    fn name(&self) -> &'static str {
        "Unsupported"
    }
    fn file_manager(&self) -> &'static str {
        "file manager"
    }
    fn units(&self) -> UnitSystem {
        UnitSystem::Decimal
    }
    fn window_effects(&self) -> bool {
        false
    }
}
