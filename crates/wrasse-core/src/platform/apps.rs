//! Apps: running processes, bundle ids, where apps live, last use, quitting.

use super::*;

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

/// Whether `exe` is the app at `bundle` itself: its executable sits in `Contents/MacOS`. Helpers
/// that outlive the app (share and notification extensions in `PlugIns`, XPC services, login
/// items) don't count, so an app that was quit isn't reported as running.
pub(crate) fn is_app_process(bundle: &Path, exe: &Path) -> bool {
    exe.parent() == Some(bundle.join("Contents/MacOS").as_path())
}

/// An app bundle's identifier (`CFBundleIdentifier`), e.g. "com.apple.dt.Xcode".
pub(crate) fn bundle_id(app: &Path) -> Option<String> {
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

/// Homebrew's Caskrooms, next to the binaries above.
pub fn caskrooms() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/Caskroom"),
        PathBuf::from("/usr/local/Caskroom"),
    ]
}

/// Folders apps are installed into: (`/Applications`, `~/Applications`).
pub(crate) fn app_dirs(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Applications"), home.join("Applications")]
    } else {
        Vec::new()
    }
}

/// Installed apps with their leftovers (PLAN.md §6.4). Read-only.
pub fn installed_apps(home: &Path) -> Vec<crate::apps::App> {
    #[cfg(target_os = "macos")]
    return macos::apps::discover(&macos::apps::Places::system(home), &macos::apps::System);
    #[allow(unreachable_code)]
    {
        let _ = home;
        Vec::new()
    }
}

/// The app at `bundle`, discovered fresh.
pub fn app_at(home: &Path, bundle: &Path) -> Option<crate::apps::App> {
    #[cfg(target_os = "macos")]
    return macos::apps::app_at(
        &macos::apps::Places::system(home),
        &macos::apps::System,
        bundle,
    );
    #[allow(unreachable_code)]
    {
        let _ = (home, bundle);
        None
    }
}

/// Data folders whose app is gone (docs/history/phase3-plan.md decision 5).
pub fn orphaned_leftovers(home: &Path, apps: &[crate::apps::App]) -> Vec<crate::apps::Orphan> {
    #[cfg(target_os = "macos")]
    return macos::apps::orphans(
        &macos::apps::Places::system(home),
        &macos::apps::System,
        apps,
        &running_bundle_ids(),
    );
    #[allow(unreachable_code)]
    {
        let _ = (home, apps);
        Vec::new()
    }
}

/// The cask token that installed this app bundle, if any.
pub(crate) fn cask_of(app: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let name = app.file_name()?.to_string_lossy().into_owned();
        return macos::apps::cask_apps(&caskrooms()).remove(&name);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        None
    }
}

/// When a file or app was last opened (Unix seconds), from Spotlight.
pub fn last_used(path: &Path) -> Option<i64> {
    #[cfg(target_os = "macos")]
    return macos::spotlight::last_used(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        None
    }
}

/// Politely asks an app to quit (never forces it). Returns once the request is sent, or after
/// 10 s if it couldn't be sent. The caller waits for the app to exit.
pub fn quit_app(bundle_id: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::quit_app(bundle_id);
    #[allow(unreachable_code)]
    {
        let _ = bundle_id;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Bundle ids of running apps (macOS), for rules that need their app closed.
pub(crate) fn running_bundle_ids() -> Vec<String> {
    let mut ids: Vec<String> = running_executables()
        .iter()
        .filter_map(|exe| {
            let s = exe.to_string_lossy();
            let end = s.find(".app/Contents/MacOS/")?;
            bundle_id(Path::new(&s[..end + 4]))
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}
