use super::*;
use crate::apps::uninstall::{route, Route};
use fake::Fake;

fn setup(tag: &str) -> (PathBuf, wrasse_fixtures::AppsFixture, Places, Fake) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("wrasse-apps-{tag}-{nanos}"));
    fs::create_dir_all(&base).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    let f = wrasse_fixtures::mac_apps(&base).unwrap();
    let places = Places {
        home: f.home.clone(),
        app_dirs: vec![f.applications.clone(), f.home.join("Applications")],
        caskrooms: vec![f.caskroom.clone()],
        system_library: f.system_library.clone(),
    };
    let fake = Fake {
        root_owned: vec![
            f.applications.join("WrasseStore.app"),
            f.applications.join("WrasseSystem.app"),
        ],
        usage: HashMap::from([
            ("WrasseDummy.app".to_string(), (Some(40), Some(100))),
            ("WrasseCask.app".to_string(), (None, Some(200))),
            ("WrasseStore.app".to_string(), (Some(1), Some(300))),
        ]),
        registered: vec!["com.wrasse-elsewhere.app".into()],
    };
    (base, f, places, fake)
}

fn by_name<'a>(apps: &'a [App], name: &str) -> &'a App {
    apps.iter()
        .find(|a| a.bundle.file_name().unwrap() == name)
        .unwrap_or_else(|| panic!("{name}"))
}

#[test]
fn discovers_sources_owners_usage_and_folders() {
    let (base, f, places, fake) = setup("discover");
    let apps = discover(&places, &fake);
    let mut names: Vec<String> = apps
        .iter()
        .map(|a| a.bundle.strip_prefix(&base).unwrap().display().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Applications/Wrasse Suite/WrasseSuite A.app",
            "Applications/Wrasse Twin.app",
            "Applications/WrasseCask.app",
            "Applications/WrasseDummy.app",
            "Applications/WrasseRbCask.app",
            "Applications/WrasseStore.app",
            "Applications/WrasseSystem.app",
            "home/Applications/Wrasse Twin.app",
        ],
        "links to apps are not installs"
    );
    let dummy = by_name(&apps, "WrasseDummy.app");
    assert_eq!(
        (dummy.source.clone(), dummy.owner),
        (Source::Folder, Owner::User)
    );
    assert_eq!(dummy.id.as_deref(), Some("com.wrasse-test.dummy"));
    assert_eq!(dummy.version.as_deref(), Some("1.0"));
    assert_eq!(
        (dummy.last_used_days, dummy.never_opened()),
        (Some(40), false)
    );
    assert_eq!(route(dummy), Route::Tray);

    let store = by_name(&apps, "WrasseStore.app");
    assert_eq!(
        (store.source.clone(), store.owner),
        (Source::AppStore, Owner::Root)
    );
    assert_eq!(route(store), Route::FinderTrash);

    let cask = by_name(&apps, "WrasseCask.app");
    assert_eq!(
        cask.source,
        Source::Cask {
            token: "wrasse-dummy-cask".into()
        }
    );
    assert!(cask.never_opened(), "no last-used date, added 200 days ago");
    assert_eq!(
        route(cask),
        Route::Homebrew {
            token: "wrasse-dummy-cask".into()
        }
    );
    assert_eq!(
        by_name(&apps, "WrasseRbCask.app").source,
        Source::Cask {
            token: "wrasse-rb-cask".into()
        }
    );

    let system = by_name(&apps, "WrasseSystem.app");
    assert_eq!(system.source, Source::System);
    assert!(!system.removable() && system.leftovers.is_empty());
    assert_eq!(route(system), Route::NotRemovable);

    let suite = by_name(&apps, "WrasseSuite A.app");
    assert_eq!(
        suite.folder.as_deref(),
        Some(f.applications.join("Wrasse Suite").as_path())
    );
    crate::platform::remove_test_dir(&base);
}

#[test]
fn leftovers_by_id_by_name_shared_and_admin() {
    let (base, f, places, fake) = setup("leftovers");
    let apps = discover(&places, &fake);
    let dummy = by_name(&apps, "WrasseDummy.app");
    let mut got: Vec<(String, LeftoverKind, MatchedBy, bool, bool)> = dummy
        .leftovers
        .iter()
        .map(|l| {
            (
                l.path.strip_prefix(&base).unwrap().display().to_string(),
                l.kind,
                l.matched_by,
                l.needs_admin,
                l.ticked_by_default(),
            )
        })
        .collect();
    got.sort();
    use LeftoverKind as K;
    use MatchedBy::{BundleId as Id, Name};
    let lib = "home/Library";
    let mut want = vec![
        (
            format!("{lib}/Application Support/WrasseDummy"),
            K::Support,
            Name,
            false,
            false,
        ),
        (
            format!("{lib}/Caches/com.wrasse-test.dummy"),
            K::Caches,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/Containers/com.wrasse-test.dummy"),
            K::Containers,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/Group Containers/ABCDE12345.com.wrasse-test.dummy"),
            K::GroupContainers,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/HTTPStorages/com.wrasse-test.dummy"),
            K::HttpStorages,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/LaunchAgents/com.wrasse-test.dummy.helper.plist"),
            K::LaunchAgents,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/Logs/com.wrasse-test.dummy"),
            K::Logs,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/Preferences/com.wrasse-test.dummy.plist"),
            K::Preferences,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/Saved Application State/com.wrasse-test.dummy.savedState"),
            K::SavedState,
            Id,
            false,
            true,
        ),
        (
            format!("{lib}/WebKit/com.wrasse-test.dummy"),
            K::WebKit,
            Id,
            false,
            true,
        ),
        (
            "Library/LaunchDaemons/com.wrasse-test.dummy.daemon.plist".to_string(),
            K::LaunchDaemons,
            Id,
            true,
            false,
        ),
    ];
    want.sort();
    assert_eq!(
        got, want,
        "not com.wrasse-test.dummyother, not the generic \"Data\""
    );

    // Two different apps named "Wrasse Twin": their shared folder is never ticked.
    let twins: Vec<&App> = apps.iter().filter(|a| a.name == "Wrasse Twin").collect();
    assert_eq!(twins.len(), 2);
    for t in twins {
        let shared = t
            .leftovers
            .iter()
            .find(|l| l.path.ends_with("Application Support/Wrasse Twin"))
            .unwrap();
        assert_eq!(shared.shared_with, ["Wrasse Twin"]);
        assert!(!shared.ticked_by_default());
    }
    let _ = f;
    crate::platform::remove_test_dir(&base);
}

#[test]
fn orphans_are_old_unregistered_and_never_apple_or_a_known_vendor() {
    let (base, f, places, fake) = setup("orphans");
    let lib = f.home.join("Library");
    // On another disk (LaunchServices knows it): not an orphan.
    fs::create_dir_all(lib.join("Caches/com.wrasse-elsewhere.app")).unwrap();
    wrasse_fixtures::age(&lib.join("Caches/com.wrasse-elsewhere.app"), 90).unwrap();
    // Same vendor as an installed app (a helper of it): not an orphan.
    fs::create_dir_all(lib.join("Caches/com.wrasse-test.updater")).unwrap();
    wrasse_fixtures::age(&lib.join("Caches/com.wrasse-test.updater"), 90).unwrap();
    let apps = discover(&places, &fake);
    let found = orphans(&places, &fake, &apps, &[]);
    assert_eq!(found.len(), 1, "{found:#?}");
    let o = &found[0];
    assert_eq!(o.id, "com.wrasse-gone.app");
    assert!(o.untouched_days >= 59);
    let mut items: Vec<String> = o
        .items
        .iter()
        .map(|l| l.path.strip_prefix(&lib).unwrap().display().to_string())
        .collect();
    items.sort();
    assert_eq!(
        items,
        [
            "Application Support/com.wrasse-gone.app",
            "Caches/com.wrasse-gone.app",
            "Preferences/com.wrasse-gone.app.plist"
        ]
    );
    // A running app's vendor counts as installed too.
    assert!(orphans(&places, &fake, &apps, &["com.wrasse-gone.other".into()]).is_empty());
    crate::platform::remove_test_dir(&base);
}

#[test]
fn cask_tokens_and_metadata() {
    for ok in [
        "firefox",
        "visual-studio-code",
        "temurin@17",
        "1password",
        "foo.bar_baz+x",
    ] {
        assert!(valid_token(ok), "{ok}");
    }
    for bad in [
        "", "Firefox", "-rf", "foo bar", "foo;rm", "../x", "a..b", "foo/bar", "--zap", "café",
    ] {
        assert!(!valid_token(bad), "{bad}");
    }
    let (base, f, _, _) = setup("casks");
    let casks = cask_apps(std::slice::from_ref(&f.caskroom));
    assert_eq!(
        casks.get("WrasseCask.app").map(String::as_str),
        Some("wrasse-dummy-cask")
    );
    assert_eq!(
        casks.get("WrasseRbCask.app").map(String::as_str),
        Some("wrasse-rb-cask")
    );
    assert_eq!(casks.len(), 2);
    crate::platform::remove_test_dir(&base);
}
