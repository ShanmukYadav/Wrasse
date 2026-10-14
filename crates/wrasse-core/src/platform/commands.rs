//! Delegated and read-only tools: backups exclusion, Homebrew, Docker, snapshots, simulators, the Trash.

use super::*;

/// Keeps one of Wrasse's own folders out of backups and search indexing.
pub fn exclude_from_backups(dir: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::exclude_from_backups(dir);
    #[allow(unreachable_code)]
    {
        let _ = dir;
        Ok(())
    }
}

/// Delegated OS actions (PLAN.md §4.1). Only `safety::delegate` calls this, after validating the
/// action (`brew` must be one of [`brew_binaries`] and owned by the user, and so on).
pub(crate) fn run_delegated(
    action: &crate::safety::Delegated,
    brew: Option<&Path>,
) -> io::Result<()> {
    // Tests exercise `safety::delegate` with temporary homes; they must never reach a real OS
    // action (emptying the user's actual Trash, uninstalling a real app).
    #[cfg(test)]
    {
        let _ = (action, brew);
        return Err(io::Error::other("delegated actions don't run in tests"));
    }
    #[allow(unreachable_code)]
    {
        #[cfg(target_os = "macos")]
        use crate::safety::Delegated;
        #[cfg(target_os = "macos")]
        return match action {
            Delegated::EmptyTrash => macos::commands::empty_trash_via_finder(),
            Delegated::BrewUninstallCask { token, .. } => match brew {
                Some(b) => macos::commands::brew_uninstall_cask(b, token),
                None => Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Homebrew isn't installed",
                )),
            },
            Delegated::FinderMoveToTrash { path } => macos::commands::finder_move_to_trash(path),
            Delegated::OllamaRm { model } => ollama_delete(model),
            Delegated::OllamaStop { model } => ollama_stop(model),
            Delegated::ThinLocalSnapshots { bytes } => {
                macos::commands::thin_local_snapshots(*bytes)
            }
            Delegated::BrewCleanup => match brew {
                Some(b) => macos::commands::brew_cleanup(b),
                None => Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Homebrew isn't installed",
                )),
            },
            Delegated::DockerPrune => macos::commands::docker_prune(),
            Delegated::SimctlDeleteUnavailable => macos::commands::simctl_delete_unavailable(),
            Delegated::UnloadLaunchAgent { plist } => match current_uid() {
                Some(uid) => macos::commands::launchctl_bootout(uid, plist),
                None => Err(io::Error::other("couldn't tell who you are")),
            },
        };
        #[allow(unreachable_code)]
        {
            let _ = (action, brew);
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }
    }
}

/// Whether a Homebrew cask token is well formed (lowercase letters, digits, `-@._+`).
pub(crate) fn valid_cask_token(token: &str) -> bool {
    #[cfg(target_os = "macos")]
    return macos::apps::valid_token(token);
    #[allow(unreachable_code)]
    {
        let _ = token;
        false
    }
}

/// Where Homebrew may live; only these are ever run, and only when owned by the current user.
pub(crate) fn brew_binaries() -> &'static [&'static str] {
    &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
}

/// Docker's own binary. Wrasse never opens Docker's disk image; it only runs docker.
pub(crate) fn docker_bin() -> Option<PathBuf> {
    [
        "/usr/local/bin/docker",
        "/opt/homebrew/bin/docker",
        "/Applications/Docker.app/Contents/Resources/bin/docker",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

/// `tmutil listlocalsnapshots` for `/` and the Data volume. Empty when it can't be read.
pub(crate) fn snapshot_list_text() -> String {
    #[cfg(target_os = "macos")]
    {
        return macos::commands::snapshot_list_text();
    }
    #[allow(unreachable_code)]
    String::new()
}

/// `(available, important_available)` from NSURL. `None` when the keys can't be read.
pub(crate) fn volume_capacities() -> Option<(u64, u64)> {
    #[cfg(target_os = "macos")]
    {
        return macos::commands::volume_capacities();
    }
    #[allow(unreachable_code)]
    None
}

/// Bytes `brew cleanup -s --dry-run` would free. `None` when Homebrew isn't there.
pub(crate) fn brew_cleanup_preview() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let text = macos::commands::brew_cleanup_dry_run()?;
        return Some(crate::devjunk::parse_brew_cleanup(&text));
    }
    #[allow(unreachable_code)]
    None
}

/// Reclaimable images, containers and build cache from `docker system df`.
/// `None` when Docker isn't running. Volumes are not included.
pub(crate) fn docker_reclaimable() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let text = macos::commands::docker_df_text()?;
        return Some(crate::devjunk::parse_docker_df(&text));
    }
    #[allow(unreachable_code)]
    None
}

/// Unavailable simulators: `(bytes on disk, count)`. Listing only.
pub(crate) fn unavailable_simulators(home: &Path) -> Option<(u64, u32)> {
    #[cfg(target_os = "macos")]
    {
        return macos::commands::unavailable_simulators(home);
    }
    #[allow(unreachable_code)]
    {
        let _ = home;
        None
    }
}

/// Where the Trash is, to show its size before emptying.
pub(crate) fn trash_dir(home: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(home.join(".Trash"))
    } else {
        None
    }
}

/// Shows `path` selected in the file manager. Read-only.
pub fn reveal(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::reveal(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Whether Wrasse opens at login; `None` when the OS can't say (or doesn't support it).
pub fn login_item_enabled() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return macos::commands::login_item_enabled();
    #[allow(unreachable_code)]
    None
}

/// Turns Wrasse's own login item on or off (Settings switch only; off by default).
pub fn set_login_item(on: bool) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::set_login_item(on);
    #[allow(unreachable_code)]
    {
        let _ = on;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Shows a Quick Look preview of a file (Space in the app). Read-only.
pub fn quick_look(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::quick_look(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// A well-formed launchd label (letters, digits, `.-_`).
pub fn valid_launch_label(label: &str) -> bool {
    #[cfg(target_os = "macos")]
    return macos::commands::valid_label(label);
    #[allow(unreachable_code)]
    {
        let _ = label;
        false
    }
}
