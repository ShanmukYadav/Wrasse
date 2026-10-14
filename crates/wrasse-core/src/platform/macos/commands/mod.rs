//! The only external programs Wrasse runs on macOS (PLAN.md §4.1). Read-only programs go through
//! `run_readonly`, which refuses anything not on its allowlist. Those that change state have their
//! own narrowly named functions, with arguments passed as argv (never through a shell):
//! - Time Machine exclusion of Wrasse's own folders;
//! - asking an app to quit (before uninstalling it);
//! - delegated removals, reachable only through `safety::delegate`, which validates them first:
//!   emptying the Trash and moving an app to the Trash (Finder), `brew uninstall --cask`.
//!
//! AppleScript can sit forever on an Automation prompt. Every `osascript` call is spawned and
//! given up after a deadline; only that `osascript` process is killed. Quit itself does not use
//! AppleScript: it sends the Dock's polite quit and never signals the app.

mod appkit;
use appkit::*;
pub use appkit::{login_item_enabled, quit_app, reveal, set_login_item};

use std::ffi::{c_char, c_void, CStr, CString};
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How long a quit request may take to be delivered. The call returns as soon as the request is
/// sent; this only bounds a stuck one. The app itself is never signalled.
const QUIT_ASK_TIMEOUT: Duration = Duration::from_secs(10);

/// Finder scripts wait on a confirmation or a password dialog, so they get longer than quit.
/// AppleScript's own default command timeout is two minutes. Past this, only `osascript` stops.
const FINDER_ASK_TIMEOUT: Duration = Duration::from_secs(120);

const COULDNT_ASK_QUIT: &str = "couldn't ask it to quit";

/// Stopping `osascript` does not roll back what Finder already did. These are errors, never a
/// successful empty or move.
const TRASH_PARTLY_DONE: &str =
    "Emptying the Trash stopped after 2 minutes and may be partly done.";
const FINDER_PARTLY_DONE: &str =
    "Moving it to the Trash stopped after 2 minutes and may be partly done.";

/// Programs that only read state.
const READ_ONLY: [&str; 5] = [
    "/usr/bin/pmset",
    "/usr/sbin/lsof",
    "/usr/bin/tmutil",
    "/bin/launchctl",
    "/usr/bin/codesign",
];

/// Runs an allowlisted, read-only program. `tmutil` is allowed only for `isexcluded` and for
/// `listlocalsnapshots` of `/` or `/System/Volumes/Data`; `launchctl` only for `list <label>`;
/// `codesign` only for `--verify -- <path>`.
pub(crate) fn run_readonly(program: &str, args: &[&str]) -> io::Result<Output> {
    let args_ok = match program {
        "/usr/bin/tmutil" => matches!(
            args,
            ["isexcluded", _]
                | ["listlocalsnapshots", "/"]
                | ["listlocalsnapshots", "/System/Volumes/Data"]
        ),
        "/bin/launchctl" => matches!(args, ["list", label] if valid_label(label)),
        "/usr/bin/codesign" => matches!(args, ["--verify", "--", _]),
        _ => true,
    };
    let allowed = READ_ONLY.contains(&program) && args_ok;
    if !allowed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("not an allowed read-only program: {program}"),
        ));
    }
    Command::new(program).args(args).output()
}

/// Shows a Quick Look preview of `path` (`qlmanage -p`), like Space in Finder. Read-only. The
/// preview runs in its own process, which Wrasse doesn't wait for; a thread reaps it.
pub fn quick_look(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || !path.exists() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    let mut child = Command::new("/usr/bin/qlmanage")
        .arg("-p")
        .arg("--")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// A launchd label: letters, digits and `.-_`, not starting with `-`.
pub(crate) fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('-')
        && label.len() <= 255
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Whether launchd has a job with this label loaded in your session (`launchctl list <label>`).
pub(crate) fn launch_job_loaded(label: &str) -> bool {
    run_readonly("/bin/launchctl", &["list", label]).is_ok_and(|o| o.status.success())
}

/// Whether a program has a valid code signature (`codesign --verify`). `None` if it's missing.
pub(crate) fn code_signed(program: &Path) -> Option<bool> {
    if !program.is_file() {
        return None;
    }
    let p = program.to_str()?;
    run_readonly("/usr/bin/codesign", &["--verify", "--", p])
        .ok()
        .map(|o| o.status.success())
}

/// `launchctl bootout gui/<uid> <plist>`: unloads one of your own agents (delegated; validated in
/// `safety::delegate`). It stays installed until its plist goes to the tray.
pub(crate) fn launchctl_bootout(uid: u32, plist: &Path) -> io::Result<()> {
    let out = Command::new("/bin/launchctl")
        .arg("bootout")
        .arg(format!("gui/{uid}"))
        .arg(plist)
        .stdin(Stdio::null())
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "launchctl couldn't unload it: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// Whether any process has the file open (`lsof -t`). Errors count as "can't tell" = not open.
pub(crate) fn is_open(path: &Path) -> bool {
    let Some(p) = path.to_str() else { return false };
    run_readonly("/usr/sbin/lsof", &["-t", "--", p]).is_ok_and(|o| !o.stdout.is_empty())
}

/// Keeps Wrasse's own folder (quarantine, scan cache) out of Time Machine and Spotlight.
/// `tmutil addexclusion` without `-p` sets a sticky exclusion on the folder and needs no admin
/// rights; `.metadata_never_index` asks Spotlight to skip it.
pub fn exclude_from_backups(dir: &Path) -> io::Result<()> {
    std::fs::write(dir.join(".metadata_never_index"), b"")?;
    let Some(p) = dir.to_str() else { return Ok(()) };
    let out = Command::new("/usr/bin/tmutil")
        .args(["addexclusion", p])
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}

/// Empties the Trash through Finder, so macOS handles items the user's account can't delete
/// directly. Not undoable. Called only from `safety::delegate`.
///
/// If Finder hasn't finished after 2 minutes, this returns an error
/// ([`TRASH_PARTLY_DONE`]): the empty may already have started, so it is not success.
pub(crate) fn empty_trash_via_finder() -> io::Result<()> {
    let out = run_osascript(
        &["-e", "tell application \"Finder\" to empty trash"],
        FINDER_ASK_TIMEOUT,
        TRASH_PARTLY_DONE,
    )?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// Moves one item to the Trash through Finder, which asks for an administrator password when the
/// item is root-owned (App Store and package installs). Put Back works. The path goes in as an
/// argument to the script, so it can't change the script.
///
/// If Finder hasn't finished after 2 minutes, this returns an error
/// ([`FINDER_PARTLY_DONE`]): the move may already have started, so it is not success.
pub(crate) fn finder_move_to_trash(path: &Path) -> io::Result<()> {
    let Some(p) = path.to_str() else {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    };
    let out = run_osascript(
        &[
            "-e",
            "on run argv",
            "-e",
            "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
            "-e",
            "end run",
            "--",
            p,
        ],
        FINDER_ASK_TIMEOUT,
        FINDER_PARTLY_DONE,
    )?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// Local Time Machine snapshot names for the two volumes that can hold them.
pub(crate) fn snapshot_list_text() -> String {
    let mut out = String::new();
    for mount in ["/", "/System/Volumes/Data"] {
        if let Ok(got) = run_readonly("/usr/bin/tmutil", &["listlocalsnapshots", mount]) {
            out.push_str(&String::from_utf8_lossy(&got.stdout));
            out.push('\n');
        }
    }
    out
}

/// Free space, and free space including what macOS can reclaim for an important allocation.
pub(crate) fn volume_capacities() -> Option<(u64, u64)> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let got = volume_capacities_in_pool();
        objc_autoreleasePoolPop(pool);
        got
    }
}

/// AppleScript that asks for an administrator password and thins snapshots.
/// `bytes` is a number, so the script text cannot be changed by it.
pub(crate) fn thin_admin_source(bytes: u64) -> String {
    format!(
        "do shell script \"/usr/bin/tmutil thinlocalsnapshots / {bytes} 4\" with administrator privileges"
    )
}

/// Thins local snapshots. Tries without a password first. If that fails, macOS asks
/// through its own administrator prompt. Called only from `safety::delegate`.
pub(crate) fn thin_local_snapshots(bytes: u64) -> io::Result<()> {
    if bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no space held by snapshots",
        ));
    }
    let amount = bytes.to_string();
    let out = Command::new("/usr/bin/tmutil")
        .args(["thinlocalsnapshots", "/", &amount, "4"])
        .output()?;
    if out.status.success() {
        return Ok(());
    }
    let source = thin_admin_source(bytes);
    let admin = run_osascript(
        &["-e", &source],
        FINDER_ASK_TIMEOUT,
        "Thinning snapshots stopped after 2 minutes and may be partly done.",
    )?;
    if admin.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&admin.stderr).trim().to_string(),
        ))
    }
}

/// `brew cleanup -s --dry-run`. Nothing is removed. `None` when Homebrew isn't installed.
pub(crate) fn brew_cleanup_dry_run() -> Option<String> {
    let brew = crate::platform::brew_binaries()
        .iter()
        .copied()
        .find(|b| Path::new(b).is_file())?;
    let out = Command::new(brew)
        .args(["cleanup", "-s", "--dry-run"])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ANALYTICS", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .output()
        .ok()?;
    if out.stdout.is_empty() && !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `brew cleanup -s`. Called only from `safety::delegate`.
pub(crate) fn brew_cleanup(brew: &Path) -> io::Result<()> {
    let out = Command::new(brew)
        .args(["cleanup", "-s"])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ANALYTICS", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// `docker system df`, after `docker info` succeeds. `None` when Docker isn't running.
pub(crate) fn docker_df_text() -> Option<String> {
    let bin = crate::platform::docker_bin()?;
    let info = Command::new(&bin).arg("info").output().ok()?;
    if !info.status.success() {
        return None;
    }
    let df = Command::new(&bin).args(["system", "df"]).output().ok()?;
    if !df.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&df.stdout).into_owned())
}

/// `docker system prune -f`. Volumes are not removed. Called only from `safety::delegate`.
pub(crate) fn docker_prune() -> io::Result<()> {
    let Some(bin) = crate::platform::docker_bin() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Docker isn't running",
        ));
    };
    let info = Command::new(&bin).arg("info").output()?;
    if !info.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Docker isn't running",
        ));
    }
    let out = Command::new(&bin)
        .args(["system", "prune", "-f"])
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// Unavailable simulator device ids, and how much their device folders hold. Listing only.
pub(crate) fn unavailable_simulators(home: &Path) -> Option<(u64, u32)> {
    let out = Command::new("/usr/bin/xcrun")
        .args(["simctl", "list", "devices", "unavailable"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let ids = crate::devjunk::parse_unavailable_simulators(&text);
    let mut bytes = 0u64;
    let mut count = 0u32;
    for id in ids {
        if !valid_sim_id(&id) {
            continue;
        }
        let path = home
            .join("Library/Developer/CoreSimulator/Devices")
            .join(&id);
        bytes += crate::quarantine::size_on_disk(&path);
        count += 1;
    }
    Some((bytes, count))
}

fn valid_sim_id(id: &str) -> bool {
    (8..=64).contains(&id.len())
        && id.contains('-')
        && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// `xcrun simctl delete unavailable`. Called only from `safety::delegate`.
pub(crate) fn simctl_delete_unavailable() -> io::Result<()> {
    let out = Command::new("/usr/bin/xcrun")
        .args(["simctl", "delete", "unavailable"])
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// `brew uninstall --cask <token>` with a validated brew binary and token. No `--zap`: Wrasse
/// sends the leftovers to the tray itself, so they stay restorable (docs/history/phase3-plan.md decision 2).
pub(crate) fn brew_uninstall_cask(brew: &Path, token: &str) -> io::Result<()> {
    let out = Command::new(brew)
        .args(["uninstall", "--cask", "--", token])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ANALYTICS", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// Runs `/usr/bin/osascript` and gives up after `timeout`. On timeout only that process is
/// killed (not Finder, not the app the script was talking to).
fn run_osascript(args: &[&str], timeout: Duration, on_timeout: &str) -> io::Result<Output> {
    let mut child = Command::new("/usr/bin/osascript")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout_thread = std::thread::spawn(read_pipe(child.stdout.take()));
    let stderr_thread = std::thread::spawn(read_pipe(child.stderr.take()));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                timed_out = true;
                // `Child::kill` signals this pid with SIGKILL. It is not a process group,
                // so Finder and the target app are not signalled.
                let _ = child.kill();
                break child.wait()?;
            }
            None => std::thread::sleep(Duration::from_millis(40)),
        }
    };
    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();
    if timed_out {
        return Err(io::Error::other(on_timeout));
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn read_pipe(pipe: Option<impl Read + Send + 'static>) -> impl FnOnce() -> Vec<u8> + Send {
    move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    }
}

#[cfg(test)]
mod tests;
