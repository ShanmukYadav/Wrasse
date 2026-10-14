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

/// Whether launchd has this job loaded in your session. Read-only.
pub fn launch_job_loaded(label: &str) -> bool {
    #[cfg(target_os = "macos")]
    return macos::commands::launch_job_loaded(label);
    #[allow(unreachable_code)]
    {
        let _ = label;
        false
    }
}

/// Whether a program has a valid code signature; `None` if it's missing or can't be checked.
pub fn code_signed(program: &Path) -> Option<bool> {
    #[cfg(target_os = "macos")]
    return macos::commands::code_signed(program);
    #[allow(unreachable_code)]
    {
        let _ = program;
        None
    }
}

/// The `Label` of a launchd property list.
pub fn launch_agent_label(plist: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    return plist::Value::from_file(plist)
        .ok()?
        .as_dictionary()?
        .get("Label")?
        .as_string()
        .map(str::to_string);
    #[allow(unreachable_code)]
    {
        let _ = plist;
        None
    }
}

/// Starts another copy of Wrasse itself (this executable) with `args`, detached: the app hands
/// over to its menu-bar-only mode this way, and back. Only ever Wrasse's own executable.
pub fn start_self(args: &[&str]) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(args)
        // Measurement hooks belong to the copy a check started, not to the next one.
        .env_remove("WRASSE_AUTOSCAN")
        .env_remove("WRASSE_SCRIPTED_USE")
        .env_remove("WRASSE_RESTART_AFTER_SECS")
        .env_remove("WRASSE_BENCH")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

/// Stops Wrasse's own menu-bar-only copy (pid from its pid file), after checking that the
/// process really is this executable. Never signals anything else.
pub fn stop_own_copy(pid: u32) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    if pid == std::process::id() {
        return false;
    }
    let mut sys = sysinfo::System::new();
    let target = sysinfo::Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[target]),
        true,
        sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always),
    );
    let ours = sys
        .process(target)
        .and_then(|p| p.exe())
        .is_some_and(|e| e == exe);
    #[cfg(unix)]
    if ours {
        // SAFETY: a plain kill(2) with SIGTERM on a pid just checked to be our own executable.
        return unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0;
    }
    let _ = ours;
    false
}

/// Waits until process `pid` has exited, at most `timeout`. Used when Wrasse restarts itself.
pub fn wait_for_exit(pid: u32, timeout: std::time::Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        let mut sys = sysinfo::System::new();
        let target = sysinfo::Pid::from_u32(pid);
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[target]), true);
        if sys.process(target).is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
