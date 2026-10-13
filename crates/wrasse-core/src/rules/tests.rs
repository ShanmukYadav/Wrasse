use super::*;
use crate::platform;

struct Fixture {
    base: PathBuf,
    home: PathBuf,
    protected: Vec<Protected>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        platform::remove_test_dir(&self.base);
    }
}

fn fixture(tag: &str) -> Fixture {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-rules-{tag}-{nanos}"));
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    let home = fs::canonicalize(&home).unwrap();
    let protected = platform::protected_paths(&home, &base.join("data"));
    Fixture {
        base: fs::canonicalize(&base).unwrap(),
        home,
        protected,
    }
}

fn put(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; bytes]).unwrap();
}

fn age(path: &Path, days: u64) {
    let t = SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn rule_file(body: &str) -> String {
    format!("schema_version = 1\n{body}")
}

fn one_rule(roots: &str, include: &str, extra: &str) -> String {
    rule_file(&format!(
        "[[rule]]\nid = \"t\"\nname = \"T\"\nplatform = \"macos\"\ncategory = \"temp\"\nroots = [{roots}]\ninclude = [{include}]\nsafety = \"safe\"\nexplanation = \"x\"\n{extra}"
    ))
}

#[test]
fn bundled_pack_is_valid_on_this_mac() {
    let home = platform::home_dir().unwrap();
    let protected = platform::protected_paths(&home, Path::new("/nonexistent-wrasse-data"));
    let (rules, errors) = load(bundled_text(), false, &home, &protected);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(rules.len(), 16);
}

#[test]
fn each_macos_rule_matches_its_fixture() {
    let f = fixture("pack");
    let h = &f.home;
    put(&h.join("Library/Caches/com.spotify.client/data"), 3000);
    put(&h.join("Library/Caches/com.apple.bird/x"), 10); // excluded (iCloud)
    put(
        &h.join("Library/Caches/Google/Chrome/Default/Cache/a"),
        2000,
    );
    put(&h.join("Library/Caches/Firefox/Profiles/a"), 100);
    put(&h.join("Library/Caches/com.apple.Safari/Cache.db"), 100);
    put(&h.join("Library/Caches/Arc/a"), 100);
    put(&h.join("Library/Caches/BraveSoftware/a"), 100);
    put(&h.join("Library/Caches/Microsoft Edge/a"), 100);
    put(&h.join("Library/Logs/Spotify/log.txt"), 100);
    put(&h.join("Library/Logs/DiagnosticReports/App.ips"), 100);
    put(
        &h.join("Library/Developer/Xcode/DerivedData/MyApp-abc/Build/x"),
        100,
    );
    put(
        &h.join("Library/Developer/Xcode/Archives/2025-01-01/a.xcarchive"),
        100,
    );
    age(
        &h.join("Library/Developer/Xcode/Archives/2025-01-01/a.xcarchive"),
        200,
    );
    put(
        &h.join("Library/Developer/Xcode/iOS DeviceSupport/17.0/x"),
        100,
    );
    put(
        &h.join("Library/Application Support/MobileSync/Backup/abc/Manifest.db"),
        100,
    );
    put(
        &h.join("Library/Containers/com.apple.mail/Data/Library/Mail Downloads/1/a.pdf"),
        100,
    );
    put(&h.join("Downloads/OldApp.dmg"), 100);
    put(&h.join("Downloads/NewApp.dmg"), 100);
    put(&h.join("Downloads/notes.txt"), 100);
    put(&h.join(".Trash/old.txt"), 100);
    age(&h.join("Downloads/OldApp.dmg"), 45);
    // Age the folders whose rules have a minimum age (a folder's own date is what counts).
    for d in [
        "Library/Developer/Xcode/Archives/2025-01-01",
        "Library/Developer/Xcode/iOS DeviceSupport/17.0",
    ] {
        let t = SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
        fs::File::open(h.join(d)).unwrap().set_modified(t).unwrap();
    }

    let (rules, errors) = load(bundled_text(), false, h, &f.protected);
    assert!(errors.is_empty(), "{errors:?}");
    let (found, held) = find(&rules, &["com.apple.Safari".to_string()], &|p| {
        crate::quarantine::size_on_disk(p)
    });
    let by = |id: &str| {
        found
            .iter()
            .filter(|m| m.rule_id == id)
            .map(|m| m.path.strip_prefix(h).unwrap().to_path_buf())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        by("app-caches"),
        [PathBuf::from("Library/Caches/com.spotify.client")]
    );
    assert_eq!(
        by("chrome-cache"),
        [PathBuf::from("Library/Caches/Google/Chrome/Default")]
    );
    assert_eq!(
        by("firefox-cache"),
        [PathBuf::from("Library/Caches/Firefox/Profiles")]
    );
    assert!(
        by("safari-cache").is_empty(),
        "Safari is open, so its rule is held"
    );
    assert_eq!(held[0].rule_id, "safari-cache");
    assert_eq!(by("arc-cache").len(), 1);
    assert_eq!(by("brave-cache").len(), 1);
    assert_eq!(by("edge-cache").len(), 1);
    assert_eq!(by("app-logs"), [PathBuf::from("Library/Logs/Spotify")]);
    assert_eq!(
        by("crash-reports"),
        [PathBuf::from("Library/Logs/DiagnosticReports/App.ips")]
    );
    assert_eq!(
        by("xcode-derived-data"),
        [PathBuf::from(
            "Library/Developer/Xcode/DerivedData/MyApp-abc"
        )]
    );
    assert_eq!(
        by("xcode-old-archives"),
        [PathBuf::from("Library/Developer/Xcode/Archives/2025-01-01")]
    );
    assert_eq!(
        by("xcode-device-support"),
        [PathBuf::from(
            "Library/Developer/Xcode/iOS DeviceSupport/17.0"
        )]
    );
    assert_eq!(
        by("ios-backups"),
        [PathBuf::from(
            "Library/Application Support/MobileSync/Backup/abc"
        )]
    );
    assert_eq!(by("mail-downloads").len(), 1);
    assert_eq!(
        by("old-installers"),
        [PathBuf::from("Downloads/OldApp.dmg")],
        "only installers older than 30 days"
    );
    assert_eq!(by("trash"), [PathBuf::from(".Trash")]);
    assert!(found
        .iter()
        .find(|m| m.rule_id == "trash")
        .unwrap()
        .delegate
        .is_some());
}

#[test]
fn malicious_rules_are_rejected() {
    let f = fixture("evil");
    let h = &f.home;
    fs::create_dir_all(h.join("Library/Keychains")).unwrap();
    fs::create_dir_all(h.join("Library/Caches")).unwrap();
    std::os::unix::fs::symlink(h.join("Library/Keychains"), h.join("Library/Caches/evil")).unwrap();
    let cases = [
        (
            "root inside Documents",
            one_rule("\"~/Documents\"", "\"**\"", ""),
        ),
        ("root is home", one_rule("\"~\"", "\"**\"", "")),
        (
            "root contains protected",
            one_rule("\"~/Library\"", "\"**\"", ""),
        ),
        ("system root", one_rule("\"/System/Library\"", "\"**\"", "")),
        ("whole disk", one_rule("\"/\"", "\"**\"", "")),
        (
            "escape with ..",
            one_rule("\"~/Library/Caches/../Keychains\"", "\"**\"", ""),
        ),
        (
            "symlink escape",
            one_rule("\"~/Library/Caches/evil\"", "\"**\"", ""),
        ),
        (
            "relative root",
            one_rule("\"Library/Caches\"", "\"**\"", ""),
        ),
        (
            "glob escape ..",
            one_rule("\"~/Library/Caches\"", "\"../*\"", ""),
        ),
        (
            "glob absolute",
            one_rule("\"~/Library/Caches\"", "\"/etc/*\"", ""),
        ),
        (
            "glob nested path",
            one_rule("\"~/Library/Caches\"", "\"a/b\"", ""),
        ),
        (
            "keep rule",
            one_rule("\"~/Library/Caches\"", "\"**\"", "")
                .replace("safety = \"safe\"", "safety = \"keep\""),
        ),
        (
            "unknown delegate",
            one_rule("\"~/Library/Caches\"", "\"**\"", "delegate = \"rm-rf\""),
        ),
        (
            "photos library",
            one_rule("\"~/Pictures/Photos Library.photoslibrary\"", "\"**\"", ""),
        ),
    ];
    for (what, text) in cases {
        let (rules, errors) = load(&text, false, h, &f.protected);
        assert!(
            rules.is_empty() && !errors.is_empty(),
            "{what} should be rejected"
        );
    }
    // Wrong schema version: the whole file is refused.
    let (rules, errors) = load(
        &one_rule("\"~/Library/Caches\"", "\"**\"", "")
            .replace("schema_version = 1", "schema_version = 2"),
        false,
        h,
        &f.protected,
    );
    assert!(rules.is_empty() && errors[0].why.contains("schema_version"));
    // Custom rules can't be safe and can't delegate.
    let (rules, _) = load(
        &one_rule("\"~/Library/Caches\"", "\"**\"", ""),
        true,
        h,
        &f.protected,
    );
    assert_eq!(rules[0].safety, Safety::YourCall);
    let (rules, errors) = load(
        &one_rule("\"~/.Trash\"", "\"**\"", "delegate = \"empty-trash\""),
        true,
        h,
        &f.protected,
    );
    assert!(rules.is_empty() && errors[0].why.contains("delegate"));
}

/// One bundled rule, on its own: it matches exactly `expect` (paths relative to home) and
/// none of `misses`; it never matches a protected place, not even an app library planted
/// inside its root; and sending everything it matched through `safety` (to quarantine)
/// leaves every protected file byte-identical.
fn check_rule(id: &str, setup: &dyn Fn(&Path), expect: &[&str], misses: &[&str]) {
    let f = fixture(id);
    let h = &f.home;
    let traps = [
        "Library/Keychains/login.keychain-db",
        "Library/Mail/V10/mailbox",
        "Library/Messages/chat.db",
        "Library/Cookies/Cookies.binarycookies",
        "Documents/thesis.docx",
        "Desktop/todo.txt",
        "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
    ];
    for t in traps {
        put(&h.join(t), 64);
    }
    setup(h);
    let (rules, errors) = load(bundled_text(), false, h, &f.protected);
    assert!(errors.is_empty(), "{errors:?}");
    let rule = rules.iter().find(|r| r.id == id).expect("rule exists");
    for root in &rule.roots {
        put(
            &root.join("Planted.photoslibrary/database/Photos.sqlite"),
            64,
        );
        put(&root.join("Planted.musiclibrary/Library.musicdb"), 64);
    }
    let (found, _) = find(&rules, &[], &|p| crate::quarantine::size_on_disk(p));
    let mine: Vec<&Match> = found.iter().filter(|m| m.rule_id == id).collect();
    let mut got: Vec<PathBuf> = mine
        .iter()
        .map(|m| m.path.strip_prefix(h).unwrap().to_path_buf())
        .collect();
    got.sort();
    let mut want: Vec<PathBuf> = expect.iter().map(PathBuf::from).collect();
    want.sort();
    assert_eq!(got, want, "{id} matched the wrong things");
    for miss in misses {
        assert!(
            !got.contains(&PathBuf::from(miss)),
            "{id} must not match {miss}"
        );
    }

    // Never a protected path, by the protected list itself.
    for m in &mine {
        assert!(
            !m.path
                .components()
                .any(|c| platform::protected_name_suffixes()
                    .iter()
                    .any(|s| c.as_os_str().to_string_lossy().ends_with(s))),
            "{id} matched an app library: {}",
            m.path.display()
        );
        for p in &f.protected {
            let hit = match p.protection {
                Protection::FolderOnly => m.path == p.path,
                _ => m.path.starts_with(&p.path),
            };
            assert!(!hit, "{id} matched protected {}", m.path.display());
        }
    }

    // And `safety` agrees: nothing is refused, and quarantining it all leaves the traps alone.
    let ctx = crate::safety::Context::new(h.clone(), f.base.join("data")).unwrap();
    let requests: Vec<crate::safety::Request> = mine
        .iter()
        .filter(|m| m.delegate.is_none())
        .map(|m| crate::safety::Request {
            path: m.path.clone(),
            safety: m.safety,
            rule_id: Some(m.rule_id.clone()),
            root: Some(m.root.clone()),
            hand_picked: false,
            scanned: None,
        })
        .collect();
    for m in mine.iter().filter(|m| m.delegate.is_some()) {
        assert_eq!(m.path, m.root, "a delegated rule offers only its root");
    }
    let plan = crate::safety::prepare(&ctx, &requests, crate::safety::Mode::Quarantine);
    assert!(plan.skipped.is_empty(), "{id}: {:?}", plan.skipped);
    let report = crate::safety::remove(&ctx, &plan, false).unwrap();
    assert_eq!(report.done.len(), requests.len());
    for t in traps {
        assert_eq!(
            fs::read(h.join(t)).unwrap(),
            vec![7u8; 64],
            "{t} was touched"
        );
    }
    for root in &rule.roots {
        assert!(root
            .join("Planted.photoslibrary/database/Photos.sqlite")
            .exists());
    }
}

fn age_dir(path: &Path, days: u64) {
    let t = SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    fs::File::open(path).unwrap().set_modified(t).unwrap();
}

#[test]
fn rule_app_caches() {
    check_rule(
        "app-caches",
        &|h| {
            put(&h.join("Library/Caches/com.spotify.client/data"), 3000);
            put(&h.join("Library/Caches/com.apple.bird/x"), 10);
            put(&h.join("Library/Caches/CloudKit/x"), 10);
            put(&h.join("Library/Caches/com.apple.akd/x"), 10);
            put(&h.join("Library/Caches/Google/Chrome/Default/a"), 10);
            put(&h.join("Library/Caches/com.apple.Safari/Cache.db"), 10);
        },
        &["Library/Caches/com.spotify.client"],
        &[
            "Library/Caches/com.apple.bird",
            "Library/Caches/CloudKit",
            "Library/Caches/com.apple.akd",
            "Library/Caches/Google",
            "Library/Caches/com.apple.Safari",
        ],
    );
}

#[test]
fn rule_app_logs() {
    check_rule(
        "app-logs",
        &|h| {
            put(&h.join("Library/Logs/Spotify/log.txt"), 100);
            put(&h.join("Library/Logs/DiagnosticReports/App.ips"), 100);
        },
        &["Library/Logs/Spotify"],
        &["Library/Logs/DiagnosticReports"],
    );
}

#[test]
fn rule_crash_reports() {
    check_rule(
        "crash-reports",
        &|h| {
            put(&h.join("Library/Logs/DiagnosticReports/App.ips"), 100);
            put(&h.join("Library/Logs/Spotify/log.txt"), 100);
        },
        &["Library/Logs/DiagnosticReports/App.ips"],
        &["Library/Logs/Spotify"],
    );
}

#[test]
fn rule_chrome_cache() {
    check_rule(
        "chrome-cache",
        &|h| {
            put(
                &h.join("Library/Caches/Google/Chrome/Default/Cache/a"),
                2000,
            )
        },
        &["Library/Caches/Google/Chrome/Default"],
        &["Library/Caches/Google"],
    );
}

#[test]
fn rule_firefox_cache() {
    check_rule(
        "firefox-cache",
        &|h| put(&h.join("Library/Caches/Firefox/Profiles/a"), 100),
        &["Library/Caches/Firefox/Profiles"],
        &[],
    );
}

#[test]
fn rule_safari_cache() {
    check_rule(
        "safari-cache",
        &|h| put(&h.join("Library/Caches/com.apple.Safari/Cache.db"), 100),
        &["Library/Caches/com.apple.Safari/Cache.db"],
        &[],
    );
}

#[test]
fn rule_arc_cache() {
    check_rule(
        "arc-cache",
        &|h| put(&h.join("Library/Caches/Arc/a"), 100),
        &["Library/Caches/Arc/a"],
        &[],
    );
}

#[test]
fn rule_brave_cache() {
    check_rule(
        "brave-cache",
        &|h| put(&h.join("Library/Caches/BraveSoftware/Brave-Browser/a"), 100),
        &["Library/Caches/BraveSoftware/Brave-Browser"],
        &[],
    );
}

#[test]
fn rule_edge_cache() {
    check_rule(
        "edge-cache",
        &|h| put(&h.join("Library/Caches/Microsoft Edge/Default/a"), 100),
        &["Library/Caches/Microsoft Edge/Default"],
        &[],
    );
}

#[test]
fn rule_xcode_derived_data() {
    check_rule(
        "xcode-derived-data",
        &|h| {
            put(
                &h.join("Library/Developer/Xcode/DerivedData/MyApp-abc/Build/x"),
                100,
            )
        },
        &["Library/Developer/Xcode/DerivedData/MyApp-abc"],
        &[],
    );
}

#[test]
fn rule_xcode_old_archives() {
    check_rule(
        "xcode-old-archives",
        &|h| {
            let a = h.join("Library/Developer/Xcode/Archives");
            put(&a.join("2025-01-01/Old.xcarchive/Info.plist"), 100);
            put(&a.join("2026-10-01/New.xcarchive/Info.plist"), 100);
            age_dir(&a.join("2025-01-01"), 200);
        },
        &["Library/Developer/Xcode/Archives/2025-01-01"],
        &["Library/Developer/Xcode/Archives/2026-10-01"],
    );
}

#[test]
fn rule_xcode_device_support() {
    check_rule(
        "xcode-device-support",
        &|h| {
            let d = h.join("Library/Developer/Xcode/iOS DeviceSupport");
            put(&d.join("16.0/Symbols/x"), 100);
            put(&d.join("18.0/Symbols/x"), 100);
            age_dir(&d.join("16.0"), 400);
        },
        &["Library/Developer/Xcode/iOS DeviceSupport/16.0"],
        &["Library/Developer/Xcode/iOS DeviceSupport/18.0"],
    );
}

#[test]
fn rule_ios_backups() {
    check_rule(
        "ios-backups",
        &|h| {
            put(
                &h.join("Library/Application Support/MobileSync/Backup/abc/Manifest.db"),
                100,
            )
        },
        &["Library/Application Support/MobileSync/Backup/abc"],
        &[],
    );
}

#[test]
fn rule_mail_downloads() {
    check_rule(
        "mail-downloads",
        &|h| {
            put(
                &h.join("Library/Containers/com.apple.mail/Data/Library/Mail Downloads/1/a.pdf"),
                100,
            );
        },
        &["Library/Containers/com.apple.mail/Data/Library/Mail Downloads/1"],
        &["Library/Mail/V10"],
    );
}

#[test]
fn rule_old_installers() {
    check_rule(
        "old-installers",
        &|h| {
            for (name, days) in [
                ("Old.dmg", 45),
                ("Old.pkg", 45),
                ("Old.zip", 45),
                ("Xcode.xip", 45),
                ("New.dmg", 2),
                ("notes.txt", 45),
                ("photo.jpg", 45),
            ] {
                put(&h.join("Downloads").join(name), 100);
                age(&h.join("Downloads").join(name), days);
            }
        },
        &[
            "Downloads/Old.dmg",
            "Downloads/Old.pkg",
            "Downloads/Old.zip",
            "Downloads/Xcode.xip",
        ],
        &[
            "Downloads/New.dmg",
            "Downloads/notes.txt",
            "Downloads/photo.jpg",
            "Downloads",
        ],
    );
}

#[test]
fn rule_trash() {
    check_rule(
        "trash",
        &|h| put(&h.join(".Trash/old.txt"), 100),
        &[".Trash"],
        &[".Trash/old.txt"],
    );
}

/// Every rule with `requires_app_closed` is held (not matched) while its app runs and matches
/// again once it's closed; other rules are unaffected.
#[test]
fn cache_rules_wait_for_their_app_to_close() {
    let f = fixture("appclosed");
    let h = &f.home;
    let (rules, errors) = load(bundled_text(), false, h, &f.protected);
    assert!(errors.is_empty(), "{errors:?}");
    let needing: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.requires_app_closed.is_some())
        .collect();
    assert_eq!(needing.len(), 7, "six browsers and Xcode");
    for r in &needing {
        put(&r.roots[0].join("entry/data"), 100);
    }
    put(&h.join("Library/Caches/com.spotify.client/data"), 100);
    let size = |p: &Path| crate::quarantine::size_on_disk(p);
    for r in &needing {
        let app = r.requires_app_closed.clone().unwrap();
        let (found, held) = find(&rules, std::slice::from_ref(&app), &size);
        assert!(
            !found.iter().any(|m| m.rule_id == r.id),
            "{} must wait while {app} runs",
            r.id
        );
        assert_eq!(held.len(), 1);
        assert_eq!(
            (held[0].rule_id.as_str(), held[0].app.as_str()),
            (r.id.as_str(), app.as_str())
        );
        assert!(
            found.iter().any(|m| m.rule_id == "app-caches"),
            "other rules still run"
        );
        for other in needing.iter().filter(|o| o.id != r.id) {
            assert!(found.iter().any(|m| m.rule_id == other.id));
        }
    }
    let (found, held) = find(&rules, &[], &size);
    assert!(held.is_empty());
    for r in &needing {
        assert!(
            found.iter().any(|m| m.rule_id == r.id),
            "{} once closed",
            r.id
        );
    }
}

/// Only "empty-trash" may be delegated, only by a bundled rule, spelled exactly.
#[test]
fn delegates_off_the_allowlist_are_refused() {
    let f = fixture("delegate");
    let h = &f.home;
    for bad in [
        "rm-rf",
        "empty-trash; rm -rf ~",
        "Empty-Trash",
        "emptyTrash",
        "empty-trash ",
        "",
        "shell",
        "osascript",
    ] {
        let text = one_rule("\"~/.Trash\"", "\"**\"", &format!("delegate = {bad:?}"));
        let (rules, errors) = load(&text, false, h, &f.protected);
        assert!(rules.is_empty(), "delegate {bad:?} must be refused");
        assert!(errors[0].why.contains("delegate"), "{errors:?}");
    }
    let ok = one_rule("\"~/.Trash\"", "\"**\"", "delegate = \"empty-trash\"");
    let (rules, _) = load(&ok, false, h, &f.protected);
    assert_eq!(rules[0].delegate, Some(Delegated::EmptyTrash));
    let (rules, errors) = load(&ok, true, h, &f.protected);
    assert!(
        rules.is_empty() && errors[0].why.contains("delegate"),
        "custom rules can't"
    );
    // The UI names delegated actions by their serialized name; nothing else deserializes.
    assert_eq!(
        serde_json::from_str::<Delegated>("\"emptyTrash\"").unwrap(),
        Delegated::EmptyTrash
    );
    for bad in ["\"empty-trash\"", "\"rmRf\"", "\"runShell\"", "\"\"", "0"] {
        assert!(serde_json::from_str::<Delegated>(bad).is_err(), "{bad}");
    }
}
