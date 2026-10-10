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
    let out = run_osascript(
        &["-e", "tell application \"Finder\" to empty trash"],
        FINDER_ASK_TIMEOUT,
        "couldn't ask Finder to empty the Trash",
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
pub fn finder_move_to_trash(path: &Path) -> io::Result<()> {
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
        "couldn't ask Finder to move it to the Trash",
    )?;
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

/// Asks an app to quit the way the Dock's Quit does (it may ask to save documents).
///
/// Uses `NSRunningApplication::terminate`, which sends `kAEQuitApplication` and returns once
/// that request is sent. It does not call `forceTerminate` and does not signal the process.
/// A call that hasn't returned after 10 s is reported as "couldn't ask it to quit"; the app is
/// left running. Already-quit apps succeed without being signalled.
pub fn quit_app(bundle_id: &str) -> io::Result<()> {
    if !valid_quit_bundle_id(bundle_id) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let id = bundle_id.to_owned();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("wrasse-quit".into())
        .spawn(move || {
            let _ = tx.send(request_quit(&id));
        })
        .map_err(|e| io::Error::other(e.to_string()))?;
    match rx.recv_timeout(QUIT_ASK_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(io::Error::other(COULDNT_ASK_QUIT)),
    }
}

fn valid_quit_bundle_id(bundle_id: &str) -> bool {
    if bundle_id.is_empty()
        || (!crate::apps::is_bundle_id(bundle_id) && bundle_id.split('.').count() < 2)
    {
        return false;
    }
    bundle_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
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

fn request_quit(bundle_id: &str) -> io::Result<()> {
    let id = CString::new(bundle_id).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `id` lives until the pool is popped. Only `terminate` is sent.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = request_quit_in_pool(id.as_ptr());
        objc_autoreleasePoolPop(pool);
        result
    }
}

/// `NSRunningApplication` polite quit. `forceTerminate` is intentionally not bound here.
///
/// # Safety
/// `bundle_id` is a valid C string. The caller holds an autorelease pool.
unsafe fn request_quit_in_pool(bundle_id: *const c_char) -> io::Result<()> {
    let class = objc_getClass(c"NSRunningApplication".as_ptr());
    let nsstring = objc_getClass(c"NSString".as_ptr());
    if class.is_null() || nsstring.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let id_str = msg_cstr(nsstring, sel(c"stringWithUTF8String:"), bundle_id);
    if id_str.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let apps = msg_id(
        class,
        sel(c"runningApplicationsWithBundleIdentifier:"),
        id_str,
    );
    if apps.is_null() {
        return Err(io::Error::other(COULDNT_ASK_QUIT));
    }
    let n = msg_usize(apps, sel(c"count"));
    if n == 0 {
        return Ok(());
    }
    let mut asked = false;
    for i in 0..n {
        let app = msg_index(apps, sel(c"objectAtIndex:"), i);
        if app.is_null() || msg_bool(app, sel(c"isTerminated")) {
            continue;
        }
        // Polite quit. Returns once the request is sent; the app may still show a save dialog.
        if msg_bool(app, sel(c"terminate")) {
            asked = true;
        }
    }
    if asked {
        Ok(())
    } else {
        Err(io::Error::other(COULDNT_ASK_QUIT))
    }
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

fn sel(name: &CStr) -> *mut c_void {
    // SAFETY: `name` is a static C string.
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `objc_msgSend` is one C symbol with several signatures. The cast matches the call.
unsafe fn msg_id(recv: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, arg)
}

unsafe fn msg_cstr(recv: *mut c_void, sel: *mut c_void, arg: *const c_char) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, arg)
}

unsafe fn msg_usize(recv: *mut c_void, sel: *mut c_void) -> usize {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel)
}

unsafe fn msg_index(recv: *mut c_void, sel: *mut c_void, index: usize) -> *mut c_void {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel, index)
}

/// `BOOL` is a byte on both Apple architectures. Non-zero is true.
unsafe fn msg_bool(recv: *mut c_void, sel: *mut c_void) -> bool {
    type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u8;
    let f: F = std::mem::transmute(objc_msgSend as *const ());
    f(recv, sel) != 0
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

    /// A stalled AppleScript is stopped, and only that process. Nothing waits out `delay`.
    #[test]
    fn stalled_osascript_is_stopped() {
        let started = Instant::now();
        let err = run_osascript(
            &["-e", "delay 30"],
            Duration::from_millis(500),
            COULDNT_ASK_QUIT,
        )
        .unwrap_err();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "osascript was not stopped: {:?}",
            started.elapsed()
        );
        assert_eq!(err.to_string(), COULDNT_ASK_QUIT);
        let still = Command::new("/bin/ps")
            .args(["-ax", "-o", "command="])
            .output()
            .unwrap();
        let lingering = String::from_utf8_lossy(&still.stdout)
            .lines()
            .any(|line| line.contains("osascript") && line.contains("delay 30"));
        assert!(!lingering, "osascript was still running");
    }

    #[test]
    fn osascript_that_finishes_returns_its_output() {
        let out = run_osascript(&["-e", "return 1"], Duration::from_secs(10), "timed out").unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains('1'));
    }

    #[test]
    fn quit_refuses_a_bad_bundle_id_without_asking() {
        for id in ["", "hello", "foo; rm", "../x", "com.foo bar", "a/b"] {
            let err = quit_app(id).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{id}");
        }
    }

    /// No process is signalled when nothing with that id is running, and the call can't hang.
    #[test]
    fn quit_of_an_app_that_is_not_running_returns() {
        let started = Instant::now();
        quit_app("com.wrasse-test.quit-timeout-absent").unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
