//! Safety support: protected places, ownership, file identity, open files.

use super::*;

// ---- Safety support (Phase 2) -----------------------------------------------------------------

/// How a protected location may be touched (PLAN.md §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// Nothing at or under this path is ever removed.
    Hard,
    /// The folder itself is never removed; items inside only when the user picks them by hand,
    /// only to quarantine, never pre-selected (Documents, Desktop, iCloud Drive).
    HandPickOnly,
    /// The folder itself is never removed; what's inside follows the normal rules (Downloads, …).
    FolderOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protected {
    pub path: PathBuf,
    pub protection: Protection,
}

/// Documents, Desktop and iCloud Drive: places whose items are only ever picked by hand.
pub fn hand_pick_only(home: &Path, wrasse_data: &Path) -> Vec<PathBuf> {
    protected_paths(home, wrasse_data)
        .into_iter()
        .filter(|p| p.protection == Protection::HandPickOnly)
        .map(|p| p.path)
        .collect()
}

/// The hardcoded protected locations for this OS. Rules and settings can't override them.
pub(crate) fn protected_paths(home: &Path, wrasse_data: &Path) -> Vec<Protected> {
    #[allow(unused_mut)] // extended per OS below
    let mut out = vec![
        Protected {
            path: PathBuf::from("/"),
            protection: Protection::FolderOnly,
        },
        Protected {
            path: home.to_path_buf(),
            protection: Protection::FolderOnly,
        },
        Protected {
            path: wrasse_data.to_path_buf(),
            protection: Protection::Hard,
        },
    ];
    #[cfg(target_os = "macos")]
    out.extend(macos::protect::protected(home));
    out
}

/// Path parts that make everything under them protected wherever they appear (app-managed
/// libraries: Photos, Music, TV).
pub(crate) fn protected_name_suffixes() -> &'static [&'static str] {
    &[
        ".photoslibrary",
        ".musiclibrary",
        ".tvlibrary",
        ".imovielibrary",
    ]
}

/// Folders synced to other devices through iCloud (removing an item there removes it everywhere).
pub(crate) fn synced_folders(home: &Path) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    return macos::protect::synced(home);
    #[allow(unreachable_code)]
    {
        let _ = home;
        Vec::new()
    }
}

/// The current user's id; items owned by anyone else are never removed.
pub(crate) fn current_uid() -> Option<u32> {
    #[cfg(unix)]
    // SAFETY: getuid has no preconditions.
    return Some(unsafe { libc::getuid() });
    #[allow(unreachable_code)]
    None
}

pub(crate) fn owner_uid(md: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    return Some(std::os::unix::fs::MetadataExt::uid(md));
    #[allow(unreachable_code)]
    {
        let _ = md;
        None
    }
}

/// Device and file id, to re-check that an item is still the same thing right before acting.
pub(crate) fn file_identity(md: &fs::Metadata) -> (u64, u64) {
    #[cfg(unix)]
    return (
        std::os::unix::fs::MetadataExt::dev(md),
        std::os::unix::fs::MetadataExt::ino(md),
    );
    #[allow(unreachable_code)]
    {
        let _ = md;
        (0, 0)
    }
}

/// Whether a process has the file open.
pub(crate) fn is_open(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    return macos::commands::is_open(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        false
    }
}
