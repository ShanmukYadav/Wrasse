//! Power and memory: battery and Low Power Mode, keeping the Mac awake, heap hints, process memory.

#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use super::*;

/// Power source, for pausing scans on low battery (PLAN.md Phase 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    pub on_battery: bool,
    /// Battery charge, 0–100 (100 where there is no battery).
    pub percent: u8,
    pub low_power: bool,
}

pub fn power_state() -> Option<PowerState> {
    #[cfg(target_os = "macos")]
    return macos::power::power_state();
    #[allow(unreachable_code)]
    None
}

/// macOS keeps freed allocations in a cache, and that cache is what a scan leaves behind.
/// `MallocSpaceEfficient=1` makes libmalloc return those pages, but the allocator reads the
/// variable when its zone is created, which is before `main`. Re-exec once so the flag is
/// already set. The web view is started later and inherits it.
pub fn prefer_small_heap() {
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("WRASSE_SMALL_HEAP").is_some() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.env("MallocSpaceEfficient", "1");
        cmd.env("WRASSE_SMALL_HEAP", "1");
        cmd.args(std::env::args_os().skip(1));
        // Replaces this process. If it fails, keep going without the flag.
        let _ = std::os::unix::process::CommandExt::exec(&mut cmd);
    }
}

/// Hands memory freed after a scan back to the OS (macOS keeps freed heap pages otherwise, which
/// shows as idle memory).
pub fn release_free_memory() {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        // SAFETY: a null zone means all zones; goal 0 means release as much as possible.
        unsafe {
            malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
}

/// Keeps the machine awake while the returned guard lives.
pub(crate) struct Awake {
    #[cfg(target_os = "macos")]
    _inner: macos::awake::Awake,
}

#[cfg(target_os = "macos")]
pub(crate) fn keep_awake(reason: &str) -> Awake {
    Awake {
        _inner: macos::awake::keep_awake(reason),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn keep_awake(_reason: &str) -> Awake {
    Awake {}
}

/// Sum of resident memory for processes whose name contains one of `needles`. Read-only.
pub(crate) fn process_memory(needles: &[&str]) -> u64 {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet),
    );
    let mut total = 0u64;
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if needles.iter().any(|n| name.contains(n) || exe.contains(n)) {
            total += process.memory();
        }
    }
    total
}
