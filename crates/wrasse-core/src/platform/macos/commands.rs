//! The only external programs Wrasse runs on macOS (PLAN.md §4.1). Read-only programs go through
//! `run_readonly`, which refuses anything not on its allowlist. Those that change state have their
//! own narrowly named functions, with arguments passed as argv (never through a shell):
//! - Time Machine exclusion of Wrasse's own folders;
//! - asking an app to quit (before uninstalling it);
//! - delegated removals, reachable only through `safety::delegate`, which validates them first:
//!   emptying the Trash and moving an app to the Trash (Finder), `brew uninstall --cask`.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

/// Programs that only read state.
const READ_ONLY: [&str; 3] = ["/usr/bin/pmset", "/usr/sbin/lsof", "/usr/bin/tmutil"];

/// Runs an allowlisted, read-only program. `tmutil` is allowed only for `isexcluded`.
pub fn run_readonly(program: &str, args: &[&str]) -> io::Result<Output> {
    let allowed = READ_ONLY.contains(&program)
        && (program != "/usr/bin/tmutil" || args.first() == Some(&"isexcluded"));
    if !allowed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("not an allowed read-only program: {program}"),
        ));
    }
    Command::new(program).args(args).output()
}

/// Whether any process has the file open (`lsof -t`). Errors count as "can't tell" = not open.
pub fn is_open(path: &Path) -> bool {
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

pub fn is_excluded_from_backups(dir: &Path) -> bool {
    let Some(p) = dir.to_str() else { return false };
    run_readonly("/usr/bin/tmutil", &["isexcluded", p])
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("[Excluded]"))
}

/// Empties the Trash through Finder, so macOS handles items the user's account can't delete
/// directly. Not undoable. Called only from `safety::delegate`.
pub fn empty_trash_via_finder() -> io::Result<()> {
    let out = Command::new("/usr/bin/osascript")
        .args(["-e", "tell application \"Finder\" to empty trash"])
        .output()?;
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
pub fn finder_move_to_trash(path: &Path) -> io::Result<()> {
    let Some(p) = path.to_str() else {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    };
    let out = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
            "-e",
            "end run",
            "--",
            p,
        ])
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
/// sends the leftovers to the tray itself, so they stay restorable (docs/phase3-plan.md decision 2).
pub fn brew_uninstall_cask(brew: &Path, token: &str) -> io::Result<()> {
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

/// Asks an app to quit, the way the Dock's Quit does (it may ask to save documents). Never forces.
pub fn quit_app(bundle_id: &str) -> io::Result<()> {
    if !crate::apps::is_bundle_id(bundle_id) && bundle_id.split('.').count() < 2 {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    if !bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let out = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "tell application id (item 1 of argv) to quit",
            "-e",
            "end run",
            "--",
            bundle_id,
        ])
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anything off the read-only allowlist is refused before a process starts.
    #[test]
    fn programs_off_the_allowlist_are_refused() {
        for (program, args) in [
            ("/bin/sh", &["-c", "true"][..]),
            ("/usr/bin/osascript", &["-e", "return 1"][..]),
            (
                "/usr/bin/tmutil",
                &["addexclusion", "/nonexistent-wrasse"][..],
            ),
            ("/usr/bin/tmutil", &[][..]),
            ("pmset", &["-g", "batt"][..]),
            ("/usr/bin/../bin/pmset", &["-g", "batt"][..]),
        ] {
            let err = run_readonly(program, args).unwrap_err();
            assert_eq!(
                err.kind(),
                io::ErrorKind::PermissionDenied,
                "{program} {args:?}"
            );
        }
        assert!(run_readonly("/usr/bin/pmset", &["-g", "batt"]).is_ok());
    }
}
