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
        (
            "/usr/bin/tmutil",
            &["thinlocalsnapshots", "/", "1", "4"][..],
        ),
        ("/usr/bin/tmutil", &["listlocalsnapshots", "relative"][..]),
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
    assert!(run_readonly("/usr/bin/tmutil", &["listlocalsnapshots", "/"]).is_ok());
    let script = thin_admin_source(1_000);
    assert_eq!(
        script,
        "do shell script \"/usr/bin/tmutil thinlocalsnapshots / 1000 4\" with administrator privileges"
    );
    assert!(!script.contains("thinlocalsnapshots / ;"));
}

/// A stalled AppleScript is stopped, and only that process. Nothing waits out `delay`.
#[test]
fn stalled_osascript_is_stopped() {
    let started = Instant::now();
    let err = run_osascript(
        &["-e", "delay 30"],
        Duration::from_millis(500),
        TRASH_PARTLY_DONE,
    )
    .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "osascript was not stopped: {:?}",
        started.elapsed()
    );
    assert_eq!(err.to_string(), TRASH_PARTLY_DONE);
    assert!(TRASH_PARTLY_DONE.contains("may be partly done"));
    assert!(FINDER_PARTLY_DONE.contains("may be partly done"));
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
