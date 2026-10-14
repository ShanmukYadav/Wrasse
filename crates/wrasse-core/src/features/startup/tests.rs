use super::*;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fake {
    loaded: HashSet<String>,
}

impl Probe for Fake {
    fn loaded(&self, label: &str) -> bool {
        self.loaded.contains(label)
    }
    fn signed(&self, program: &Path) -> Option<bool> {
        Some(program.to_string_lossy().contains("Signed"))
    }
}

fn agent(dir: &Path, label: &str, program: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{label}.plist"));
    fs::write(
        &path,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{label}</string>
  <key>ProgramArguments</key><array><string>{program}</string><string>--login</string></array>
  <key>RunAtLoad</key><true/>
</dict></plist>"#
        ),
    )
    .unwrap();
    path
}

struct Setup {
    base: PathBuf,
    ctx: Context,
    places: Vec<(PathBuf, Scope)>,
}

impl Drop for Setup {
    fn drop(&mut self) {
        crate::platform::remove_test_dir(&self.base);
    }
}

fn setup(tag: &str) -> Setup {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-startup-{tag}-{nanos}"));
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    let home = base.join("home");
    let mine = home.join("Library/LaunchAgents");
    agent(
        &mine,
        "com.wrasse-test.helper",
        "/Applications/Signed.app/Contents/MacOS/helper",
    );
    agent(&mine, "com.apple.fake-in-home", "/usr/libexec/thing");
    let all = base.join("Library/LaunchAgents");
    agent(&all, "com.wrasse-test.updater", "/Library/Unsigned/updater");
    let daemons = base.join("Library/LaunchDaemons");
    agent(
        &daemons,
        "com.wrasse-test.daemon",
        "/Library/Unsigned/daemon",
    );
    let ctx = Context::new(home, base.join("data")).unwrap();
    Setup {
        places: vec![
            (mine, Scope::User),
            (all, Scope::AllUsers),
            (daemons, Scope::System),
        ],
        base,
        ctx,
    }
}

#[test]
fn lists_every_place_and_only_your_own_can_be_turned_off() {
    let s = setup("list");
    let probe = Fake {
        loaded: ["com.wrasse-test.helper".to_string()].into(),
    };
    let items = list(&s.ctx.home, &s.places, &probe);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "com.apple.fake-in-home",
            "com.wrasse-test.helper",
            "com.wrasse-test.daemon",
            "com.wrasse-test.updater"
        ]
    );
    let helper = &items[1];
    assert!(helper.enabled && helper.can_disable);
    assert_eq!(helper.app.as_deref(), Some("Signed"));
    assert_eq!(helper.signed, Some(true));
    assert!(!items[0].can_disable, "Apple's are never touched");
    assert!(items[2..]
        .iter()
        .all(|i| !i.can_disable && i.signed == Some(false)));
}

#[test]
fn turning_off_moves_the_plist_to_the_tray_and_restore_brings_it_back() {
    let s = setup("disable");
    let probe = Fake {
        loaded: HashSet::new(),
    };
    let items = list(&s.ctx.home, &s.places, &probe);
    let helper = items
        .iter()
        .find(|i| i.label == "com.wrasse-test.helper")
        .unwrap();
    let before = fs::read(&helper.plist).unwrap();
    // Dry run changes nothing.
    disable(&s.ctx, helper, true).unwrap();
    assert!(helper.plist.exists());
    // Not loaded, so nothing to unload: the plist goes to the tray.
    let report = disable(&s.ctx, helper, false).unwrap();
    assert_eq!(report.done.len(), 1, "{report:?}");
    assert!(!helper.plist.exists());
    let entry = report.done[0].entry.clone().unwrap();
    safety::restore(&s.ctx, &[entry]).unwrap();
    assert_eq!(fs::read(&helper.plist).unwrap(), before);
}

#[test]
fn others_are_refused() {
    let s = setup("refuse");
    let probe = Fake {
        loaded: ["com.wrasse-test.daemon".to_string()].into(),
    };
    let items = list(&s.ctx.home, &s.places, &probe);
    for item in &items {
        if item.can_disable {
            continue;
        }
        let err = disable(&s.ctx, item, false).unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::PermissionDenied,
            "{}",
            item.label
        );
        assert!(item.plist.exists());
    }
    // The delegate refuses on its own too, whatever the caller says.
    for plist in [
        s.places[2].0.join("com.wrasse-test.daemon.plist"),
        s.places[0].0.join("com.apple.fake-in-home.plist"),
    ] {
        let err =
            safety::delegate(&s.ctx, Delegated::UnloadLaunchAgent { plist }, true).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
    // A loaded agent of yours passes the checks in dry run (the test build never runs launchctl).
    let helper = s.places[0].0.join("com.wrasse-test.helper.plist");
    safety::delegate(&s.ctx, Delegated::UnloadLaunchAgent { plist: helper }, true).unwrap();
}
