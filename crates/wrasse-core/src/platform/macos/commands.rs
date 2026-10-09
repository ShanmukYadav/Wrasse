//! The only external programs Wrasse runs on macOS (PLAN.md §4.1). Read-only programs go through
//! `run_readonly`, which refuses anything not on its allowlist. The two that change state
//! (Time Machine exclusion of Wrasse's own folders, emptying the Trash through Finder) have their
//! own narrowly named functions; emptying the Trash is reachable only through `safety::delegate`.

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
