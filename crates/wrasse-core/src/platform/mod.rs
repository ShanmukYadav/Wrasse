//! Platform abstraction (PLAN.md §3). All OS-specific code lives under this module.

#[cfg(target_os = "macos")]
mod macos;
mod volume;
#[cfg(windows)]
mod windows;

pub use volume::{system_volume, VolumeSpace};

use crate::format::UnitSystem;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub trait Platform: Send + Sync {
    /// Human-readable OS name for labels, e.g. "macOS".
    fn name(&self) -> &'static str;
    /// The file manager's name, used in "Reveal in …" labels.
    fn file_manager(&self) -> &'static str;
    fn units(&self) -> UnitSystem;
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
}

// ---- Scanner support -------------------------------------------------------------------------

/// Where a whole-volume scan starts, and the prefix to strip so paths read as the user sees them.
/// macOS scans the Data volume (`/System/Volumes/Data`): the sealed system volume is shown as one
/// block, and firmlinked folders (`/Users`, `/Applications`, …) are counted once.
pub fn scan_root() -> (PathBuf, &'static str) {
    #[cfg(target_os = "macos")]
    {
        const DATA: &str = "/System/Volumes/Data";
        if Path::new(DATA).is_dir() {
            return (PathBuf::from(DATA), DATA);
        }
    }
    #[cfg(windows)]
    {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        return (PathBuf::from(format!("{drive}\\")), "");
    }
    #[allow(unreachable_code)]
    (PathBuf::from("/"), "")
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Space on the system volume (or APFS container), and how much of it the OS itself uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volumes {
    pub capacity: u64,
    pub available: u64,
    /// The sealed, read-only OS volume (macOS); 0 where the OS lives in the scanned volume.
    pub os_used: u64,
}

impl Volumes {
    pub fn used(&self) -> u64 {
        self.capacity.saturating_sub(self.available)
    }
}

pub fn volumes() -> Option<Volumes> {
    #[cfg(target_os = "macos")]
    {
        let (capacity, available) = macos::container_space(Path::new("/"))?;
        let os_used = macos::volume_space_used(Path::new("/")).unwrap_or(0);
        return Some(Volumes {
            capacity,
            available,
            os_used,
        });
    }
    #[allow(unreachable_code)]
    system_volume().map(|v| Volumes {
        capacity: v.total,
        available: v.available,
        os_used: 0,
    })
}

/// Runs the calling thread at background priority (scan workers).
pub fn lower_thread_priority() {
    #[cfg(target_os = "macos")]
    macos::lower_thread_priority();
}

/// An iCloud (or other cloud) placeholder with no local data.
pub fn is_dataless(md: &fs::Metadata) -> bool {
    #[cfg(target_os = "macos")]
    return macos::is_dataless(md);
    #[allow(unreachable_code)]
    {
        let _ = md;
        false
    }
}

/// Bytes a file shares with APFS clones, if any.
pub fn clone_shared_bytes(path: &Path, allocated: u64) -> Option<u64> {
    #[cfg(target_os = "macos")]
    return macos::clone_shared_bytes(path, allocated);
    #[allow(unreachable_code)]
    {
        let _ = (path, allocated);
        None
    }
}

/// Whether Wrasse can read everything it needs (macOS Full Disk Access). Always true elsewhere.
pub fn full_disk_access() -> bool {
    #[cfg(target_os = "macos")]
    return home_dir().map_or(true, |h| macos::full_disk_access(&h));
    #[allow(unreachable_code)]
    true
}

/// Opens the OS page where the user grants Full Disk Access.
pub fn open_full_disk_access_settings() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open_full_disk_access_settings();
    #[allow(unreachable_code)]
    Ok(())
}

/// The system volume's name as the file manager shows it ("Macintosh HD").
pub fn volume_name() -> String {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let root = if cfg!(windows) {
        std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()) + "\\"
    } else {
        "/".into()
    };
    disks
        .list()
        .iter()
        .find(|d| d.mount_point() == Path::new(&root))
        .map(|d| d.name().to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "Local Disk".into()
            } else {
                "Macintosh HD".into()
            }
        })
}

/// Executable paths of running processes (to mark apps as running). Read-only.
pub fn running_executables() -> Vec<PathBuf> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .values()
        .filter_map(|p| p.exe().map(Path::to_path_buf))
        .collect()
}

/// An app bundle's identifier (`CFBundleIdentifier`), e.g. "com.apple.dt.Xcode".
pub fn bundle_id(app: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
        return info
            .as_dictionary()?
            .get("CFBundleIdentifier")?
            .as_string()
            .map(str::to_string);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        None
    }
}

/// Tests only: removes a scratch folder a test created under the OS temp dir. Refuses anything
/// else, so it can't be pointed at real data.
#[cfg(test)]
pub fn remove_test_dir(path: &Path) {
    let tmp = std::env::temp_dir();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    assert!(
        path.starts_with(&tmp) && name.starts_with("wrasse-"),
        "not a test folder: {}",
        path.display()
    );
    let _ = fs::remove_dir_all(path);
}

/// Tests only: removes one file inside a test scratch folder (same guard as `remove_test_dir`).
#[cfg(test)]
pub fn remove_test_file(path: &Path) {
    let tmp = std::env::temp_dir();
    assert!(
        path.starts_with(&tmp) && path.to_string_lossy().contains("/wrasse-"),
        "not a test file: {}",
        path.display()
    );
    let _ = fs::remove_file(path);
}

/// Folders that changed under `watch` since a stored change-log position, for incremental
/// rescans: `Some(changes)`, or `None` when only a full scan is reliable (no log, log reset,
/// events dropped). macOS: FSEvents. Elsewhere: always `None` for now.
pub fn changes_since(
    watch: &Path,
    since: u64,
    log_id: Option<&str>,
) -> Option<Vec<(PathBuf, bool)>> {
    #[cfg(target_os = "macos")]
    {
        let dev = fs::symlink_metadata(watch)
            .ok()
            .map(|m| std::os::unix::fs::MetadataExt::dev(&m))?;
        if macos::fsevents::volume_uuid(dev).as_deref() != log_id {
            return None;
        }
        return macos::fsevents::changes_since(watch, since, std::time::Duration::from_secs(15));
    }
    #[allow(unreachable_code)]
    {
        let _ = (watch, since, log_id);
        None
    }
}

/// The change log's current position and identity for the volume holding `path`.
pub fn change_log_position(path: &Path) -> (u64, Option<String>) {
    #[cfg(target_os = "macos")]
    {
        let dev = fs::symlink_metadata(path)
            .ok()
            .map(|m| std::os::unix::fs::MetadataExt::dev(&m));
        return (
            macos::fsevents::current_event_id(),
            dev.and_then(macos::fsevents::volume_uuid),
        );
    }
    #[allow(unreachable_code)]
    {
        let _ = path;
        (0, None)
    }
}

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
