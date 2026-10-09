//! Uninstall tests (docs/phase3-plan.md §7), on dummy apps from `wrasse_fixtures::mac_apps`,
//! never real apps. They live under `safety/` because they spawn a process and move fixture files.

use crate::apps::uninstall::*;
use crate::apps::App;
use crate::platform::macos::apps::{discover, fake::Fake, Places};
use crate::safety::delegate_with;
use crate::safety::{self, Context, Delegated, Reason};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Setup {
    base: PathBuf,
    ctx: Context,
    places: Places,
    fake: Fake,
}

impl Drop for Setup {
    fn drop(&mut self) {
        crate::platform::remove_test_dir(&self.base);
    }
}

/// The dummy-app fixture, with the "/Applications" stand-in moved to ~/Applications so the
/// delegated actions' "must be in an Applications folder" check sees a real one.
fn setup(tag: &str) -> Setup {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-uninstall-{tag}-{nanos}"));
    fs::create_dir_all(&base).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    let f = wrasse_fixtures::mac_apps(&base).unwrap();
    let apps_dir = f.home.join("Applications");
    for e in fs::read_dir(&f.applications).unwrap().flatten() {
        let to = apps_dir.join(e.file_name());
        if !to.exists() {
            fs::rename(e.path(), to).unwrap();
        }
    }
    let places = Places {
        home: f.home.clone(),
        app_dirs: vec![apps_dir.clone()],
        caskrooms: vec![f.caskroom.clone()],
        system_library: f.system_library.clone(),
    };
    let fake = Fake {
        root_owned: vec![
            apps_dir.join("WrasseStore.app"),
            apps_dir.join("WrasseSystem.app"),
        ],
        ..Default::default()
    };
    let ctx = Context::new(f.home.clone(), base.join("data")).unwrap();
    Setup {
        base,
        ctx,
        places,
        fake,
    }
}

fn app(s: &Setup, file: &str) -> App {
    discover(&s.places, &s.fake)
        .into_iter()
        .find(|a| a.bundle.file_name().unwrap() == file)
        .unwrap()
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).unwrap().flatten() {
            let md = fs::symlink_metadata(e.path()).unwrap();
            if md.is_dir() {
                stack.push(e.path());
            } else if md.is_file() {
                out.push((e.path(), fs::read(e.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn cask_of(app: &Path) -> Option<String> {
    (app.file_name()? == "WrasseCask.app").then(|| "wrasse-dummy-cask".to_string())
}

#[test]
fn user_owned_app_and_ticked_leftovers_go_to_the_tray_and_come_back() {
    let s = setup("tray");
    let dummy = app(&s, "WrasseDummy.app");
    assert_eq!(route(&dummy), Route::Tray);
    let before = snapshot(&s.base.join("home"));
    let ticked: Vec<PathBuf> = dummy
        .leftovers
        .iter()
        .filter(|l| l.ticked_by_default())
        .map(|l| l.path.clone())
        .collect();
    assert_eq!(ticked.len(), 9);
    let mut asked = ticked.clone();
    // Not this app's, and one needing admin: both ignored.
    asked.push(
        s.places
            .home
            .join("Library/Caches/com.wrasse-test.dummyother"),
    );
    asked.push(
        s.places
            .system_library
            .join("LaunchDaemons/com.wrasse-test.dummy.daemon.plist"),
    );

    // Dry run: nothing moves.
    let dry = uninstall(&s.ctx, &dummy, &asked, false, true).unwrap();
    assert!(dry.bundle_removed && dry.dry_run);
    assert!(dummy.bundle.exists());

    let out = uninstall(&s.ctx, &dummy, &asked, false, false).unwrap();
    assert!(out.bundle_removed && out.bundle_entry.is_some());
    assert_eq!(out.leftovers.done.len(), 9, "{:?}", out.leftovers.skipped);
    assert!(!dummy.bundle.exists());
    for p in &ticked {
        assert!(!p.exists(), "{}", p.display());
    }
    // Unticked (matched by name), not-its-own and admin items stay.
    assert!(s
        .places
        .home
        .join("Library/Application Support/WrasseDummy")
        .exists());
    assert!(s
        .places
        .home
        .join("Library/Caches/com.wrasse-test.dummyother")
        .exists());
    assert!(s
        .places
        .system_library
        .join("LaunchDaemons/com.wrasse-test.dummy.daemon.plist")
        .exists());

    let mut ids: Vec<String> = out
        .leftovers
        .done
        .iter()
        .filter_map(|d| d.entry.clone())
        .collect();
    ids.push(out.bundle_entry.unwrap());
    assert_eq!(safety::restore(&s.ctx, &ids).unwrap().len(), 10);
    assert_eq!(
        snapshot(&s.base.join("home")),
        before,
        "byte-identical after restore"
    );
}

#[test]
fn a_running_app_is_never_removed() {
    let s = setup("running");
    let dummy = app(&s, "WrasseDummy.app");
    // A real process running from inside the dummy bundle (a copy of /bin/sleep).
    let exe = dummy.bundle.join("Contents/MacOS/WrasseDummy");
    fs::copy("/bin/sleep", &exe).unwrap();
    let mut child = std::process::Command::new(&exe).arg("30").spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let running = crate::platform::running_executables();
    assert!(
        runs_from(&dummy.bundle, &running),
        "the dummy shows as running"
    );
    assert!(uninstall(&s.ctx, &dummy, &[], true, false).is_err());
    // Even if the caller got it wrong, safety refuses at the last moment.
    let out = uninstall(&s.ctx, &dummy, &[], false, false).unwrap();
    assert!(!out.bundle_removed);
    assert_eq!(out.bundle_skipped, Some(Reason::AppRunning));
    // Delegated routes refuse it too.
    let finder = Delegated::FinderMoveToTrash {
        path: dummy.bundle.clone(),
    };
    assert!(delegate_with(&s.ctx, finder, true, &cask_of).is_err());
    let _ = child.kill();
    let _ = child.wait();
    assert!(dummy.bundle.exists());
}

#[test]
fn homebrew_route_is_validated_and_never_runs_in_tests() {
    let s = setup("brew");
    let cask = app(&s, "WrasseCask.app");
    let Route::Homebrew { token } = route(&cask) else {
        panic!("cask route")
    };
    assert_eq!(token, "wrasse-dummy-cask");
    let ok = Delegated::BrewUninstallCask {
        token: token.clone(),
        app: cask.bundle.clone(),
    };
    assert!(
        delegate_with(&s.ctx, ok.clone(), true, &cask_of).unwrap() > 0,
        "dry run reports"
    );
    // Past validation, the test build stops before running anything.
    if let Err(e) = delegate_with(&s.ctx, ok, false, &cask_of) {
        assert_ne!(e.kind(), io::ErrorKind::PermissionDenied, "{e}");
    }
    assert!(cask.bundle.exists());
    let dummy = app(&s, "WrasseDummy.app");
    for (token, path) in [
        ("foo;rm -rf ~", cask.bundle.clone()),
        ("--zap", cask.bundle.clone()),
        ("../wrasse-dummy-cask", cask.bundle.clone()),
        ("Wrasse-Dummy-Cask", cask.bundle.clone()),
        ("wrasse-dummy-cask", dummy.bundle.clone()), // that cask didn't install this app
        ("wrasse-other-cask", cask.bundle.clone()),
    ] {
        let bad = Delegated::BrewUninstallCask {
            token: token.into(),
            app: path,
        };
        let err = delegate_with(&s.ctx, bad, true, &cask_of).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{token}");
    }
    assert!(s
        .ctx
        .audit
        .recent(20)
        .iter()
        .any(|r| r["result"].as_str().unwrap_or("").starts_with("refused")));
}

#[test]
fn finder_route_only_for_apps_in_an_applications_folder() {
    let mut s = setup("finder");
    let store = app(&s, "WrasseStore.app");
    assert_eq!(route(&store), Route::FinderTrash);
    let ok = Delegated::FinderMoveToTrash {
        path: store.bundle.clone(),
    };
    assert!(delegate_with(&s.ctx, ok.clone(), true, &cask_of).unwrap() > 0);
    if let Err(e) = delegate_with(&s.ctx, ok, false, &cask_of) {
        assert_ne!(e.kind(), io::ErrorKind::PermissionDenied, "{e}");
    }
    assert!(store.bundle.exists());

    let elsewhere = s.base.join("home/Downloads/Loose.app");
    wrasse_fixtures::app_bundle(&elsewhere, "com.wrasse-test.loose", "1").unwrap();
    let deep = s.base.join("home/Applications/a/b/Deep.app");
    wrasse_fixtures::app_bundle(&deep, "com.wrasse-test.deep", "1").unwrap();
    let system = app(&s, "WrasseSystem.app");
    assert_eq!(route(&system), Route::NotRemovable);
    assert!(uninstall(&s.ctx, &system, &[], false, true).is_err());
    let not_app = s.base.join("home/Applications/Wrasse Suite");
    let link = s.base.join("home/Applications/Link to Dummy.app");
    for path in [
        elsewhere,
        deep,
        system.bundle.clone(),
        not_app,
        link,
        PathBuf::from("/System/Applications/Calculator.app"),
        PathBuf::from("/Applications/Safari.app"),
    ] {
        let bad = Delegated::FinderMoveToTrash { path: path.clone() };
        assert!(
            delegate_with(&s.ctx, bad, true, &cask_of).is_err(),
            "{}",
            path.display()
        );
    }
    // On the never-touch list: refused.
    s.ctx.exclusions.add(store.bundle.clone()).unwrap();
    let err = delegate_with(
        &s.ctx,
        Delegated::FinderMoveToTrash {
            path: store.bundle.clone(),
        },
        true,
        &cask_of,
    )
    .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
}
