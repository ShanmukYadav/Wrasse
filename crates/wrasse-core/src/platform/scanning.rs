//! Scanner support: where a scan starts, volumes, dataless and cloned files, the change log, test folders.

use super::*;

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

pub(crate) fn volumes() -> Option<Volumes> {
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

/// Folders the OS guards with a per-folder prompt until Full Disk Access is on (macOS only).
pub fn consent_folders() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    return home_dir().map_or_else(Vec::new, |h| macos::consent_folders(&h));
    #[allow(unreachable_code)]
    Vec::new()
}

/// Opens the OS page where the user grants Full Disk Access.
pub fn open_full_disk_access_settings() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open_full_disk_access_settings();
    #[allow(unreachable_code)]
    Ok(())
}

/// The system volume's name as the file manager shows it ("Macintosh HD").
pub(crate) fn volume_name() -> String {
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

/// Tests only: removes a scratch folder a test created under the OS temp dir. Refuses anything
/// else, so it can't be pointed at real data.
#[cfg(test)]
pub(crate) fn remove_test_dir(path: &Path) {
    let tmp = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
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
pub(crate) fn remove_test_file(path: &Path) {
    let tmp = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
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
pub(crate) fn changes_since(
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
pub(crate) fn change_log_position(path: &Path) -> (u64, Option<String>) {
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

/// Where apps keep their data on this OS (~/Library/Application Support, %APPDATA%).
pub fn app_data_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library/Application Support"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        home_dir().map(|h| h.join(".local/share"))
    }
}
