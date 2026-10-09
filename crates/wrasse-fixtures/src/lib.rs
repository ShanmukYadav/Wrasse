//! Generates fake home folders (macOS and Windows layouts) in temp dirs for tests.
//! Destructive code is never tested against the real home folder (PLAN.md §4.3 rule 13).

use std::fs;
use std::io;
use std::path::Path;

/// A wide, deep tree of `files` empty files for scan benchmarks (PLAN.md §5.3: 2M files under
/// 300 MB). Files are spread 100 per folder, folders 100 per parent, so the shape resembles a
/// real disk (many folders, few huge ones). Empty files take no blocks, so the fixture needs
/// metadata space only. Returns the number of folders created.
pub fn many_files(root: &Path, files: u64) -> io::Result<u64> {
    const PER_DIR: u64 = 100;
    const FANOUT: u64 = 100;
    let dirs = files.div_ceil(PER_DIR);
    let mut made = 0;
    for d in 0..dirs {
        let dir = root
            .join(format!("g{:03}", d / (FANOUT * FANOUT)))
            .join(format!("p{:03}", (d / FANOUT) % FANOUT))
            .join(format!("d{:03}", d % FANOUT));
        fs::create_dir_all(&dir)?;
        made += 1;
        let start = d * PER_DIR;
        for f in start..(start + PER_DIR).min(files) {
            fs::File::create(dir.join(format!("f{f:07}.txt")))?;
        }
    }
    Ok(made)
}

/// A fake macOS home with junk for every bundled rule, plus personal files that must never be
/// touched (PLAN.md Phase 2 checkpoint: clean it, restore from quarantine, compare byte for byte).
/// Contents are deterministic; sizes are small so the fixture is fast.
pub fn mac_home(home: &Path) -> io::Result<()> {
    let files: [(&str, usize); 18] = [
        ("Library/Caches/com.spotify.client/Storage/a.file", 40_000),
        ("Library/Caches/com.spotify.client/fsCachedData/b", 12_000),
        ("Library/Caches/Google/Chrome/Default/Cache/data_0", 30_000),
        ("Library/Caches/Firefox/Profiles/x.default/cache2/c", 8_000),
        ("Library/Logs/Spotify/spotify.log", 5_000),
        ("Library/Logs/DiagnosticReports/Spotify-2025.ips", 3_000),
        (
            "Library/Developer/Xcode/DerivedData/MyApp-abc/Build/Intermediates/o",
            25_000,
        ),
        (
            "Library/Developer/Xcode/Archives/2024-03-01/MyApp.xcarchive/Info.plist",
            2_000,
        ),
        (
            "Library/Containers/com.apple.mail/Data/Library/Mail Downloads/1F2/invoice.pdf",
            6_000,
        ),
        ("Downloads/OldInstaller.dmg", 20_000),
        ("Downloads/Recent.dmg", 20_000),
        ("Downloads/notes.txt", 500),
        ("Documents/thesis.docx", 9_000),
        ("Desktop/todo.txt", 300),
        ("Library/Keychains/login.keychain-db", 4_000),
        ("Library/Mail/V10/mailbox", 7_000),
        (
            "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
            5_000,
        ),
        (".Trash/deleted-file.txt", 1_000),
    ];
    for (i, (rel, size)) in files.iter().enumerate() {
        let path = home.join(rel);
        fs::create_dir_all(path.parent().unwrap_or(home))?;
        let byte = (i as u8).wrapping_mul(37).wrapping_add(11);
        fs::write(
            &path,
            (0..*size)
                .map(|n| byte.wrapping_add((n % 251) as u8))
                .collect::<Vec<u8>>(),
        )?;
    }
    // Ages: the old installer and the archive are past their rules' minimum age.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
    fs::File::options()
        .write(true)
        .open(home.join("Downloads/OldInstaller.dmg"))?
        .set_modified(old)?;
    fs::File::open(home.join("Library/Developer/Xcode/Archives/2024-03-01"))?.set_modified(old)?;
    Ok(())
}

/// Where [`mac_apps`] put things.
#[derive(Debug, Clone)]
pub struct AppsFixture {
    /// Stands in for `/Applications`.
    pub applications: std::path::PathBuf,
    pub home: std::path::PathBuf,
    /// Stands in for `/opt/homebrew/Caskroom`.
    pub caskroom: std::path::PathBuf,
    /// Stands in for `/Library`.
    pub system_library: std::path::PathBuf,
}

/// A minimal app bundle: Info.plist with id, name and version, and a stub executable.
pub fn app_bundle(path: &Path, id: &str, version: &str) -> io::Result<()> {
    let name = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let contents = path.join("Contents");
    fs::create_dir_all(contents.join("MacOS"))?;
    fs::write(
        contents.join("Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
</dict>
</plist>
"#
        ),
    )?;
    fs::write(contents.join("MacOS").join(&name), b"#!/bin/sh\nexit 0\n")?;
    fs::write(contents.join("Resources.bin"), vec![1u8; 200_000])
}

fn put(path: &Path, bytes: usize) -> io::Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, vec![3u8; bytes])
}

/// Sets a file's or folder's modification time `days` ago.
pub fn age(path: &Path, days: u64) -> io::Result<()> {
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    fs::File::open(path)?.set_modified(t)
}

/// Dummy apps for Phase 3 tests (never real apps). Everything lives under `root`:
/// - `WrasseDummy.app` (`com.wrasse-test.dummy`), copied in by the user, with leftovers in every
///   Library place, one matched only by name, and a root-level launch daemon (needs admin);
/// - `WrasseStore.app` with an App Store receipt; `WrasseCask.app` from a Homebrew cask (JSON
///   metadata) and `WrasseRbCask.app` (older `.rb` metadata);
/// - `WrasseSystem.app` (`com.apple.wrasse-test`), standing in for an app that's part of macOS;
/// - two different apps both named `Wrasse Twin` (one per Applications folder), sharing a folder;
/// - `Wrasse Suite/WrasseSuite A.app`, an app inside a plain folder; a link to an app (ignored);
/// - leftovers of removed apps: `com.wrasse-gone.app` (60 days old: an orphan),
///   `com.wrasse-fresh.app` (touched today: not yet), `com.apple.wrasse-cache` (never).
pub fn mac_apps(root: &Path) -> io::Result<AppsFixture> {
    let applications = root.join("Applications");
    let home = root.join("home");
    let caskroom = root.join("Caskroom");
    let system_library = root.join("Library");
    let lib = home.join("Library");
    fs::create_dir_all(&applications)?;
    fs::create_dir_all(home.join("Applications"))?;

    app_bundle(
        &applications.join("WrasseDummy.app"),
        "com.wrasse-test.dummy",
        "1.0",
    )?;
    put(&lib.join("Caches/com.wrasse-test.dummy/Cache.db"), 300_000)?;
    put(
        &lib.join("Application Support/WrasseDummy/state.json"),
        120_000,
    )?;
    put(&lib.join("Containers/com.wrasse-test.dummy/Data/x"), 50_000)?;
    put(
        &lib.join("Group Containers/ABCDE12345.com.wrasse-test.dummy/shared.db"),
        40_000,
    )?;
    put(&lib.join("Preferences/com.wrasse-test.dummy.plist"), 2_000)?;
    put(
        &lib.join("Saved Application State/com.wrasse-test.dummy.savedState/w.data"),
        8_000,
    )?;
    put(&lib.join("Logs/com.wrasse-test.dummy/today.log"), 9_000)?;
    put(&lib.join("HTTPStorages/com.wrasse-test.dummy/c"), 1_000)?;
    put(&lib.join("WebKit/com.wrasse-test.dummy/w"), 1_000)?;
    put(
        &lib.join("LaunchAgents/com.wrasse-test.dummy.helper.plist"),
        500,
    )?;
    put(
        &system_library.join("LaunchDaemons/com.wrasse-test.dummy.daemon.plist"),
        500,
    )?;
    // Not the dummy's, despite the similar names.
    put(&lib.join("Caches/com.wrasse-test.dummyother/x"), 1_000)?;
    put(&lib.join("Application Support/Data/x"), 1_000)?;

    let store = applications.join("WrasseStore.app");
    app_bundle(&store, "com.wrasse-test.store", "2.0")?;
    put(&store.join("Contents/_MASReceipt/receipt"), 100)?;
    put(&lib.join("Containers/com.wrasse-test.store/Data/y"), 30_000)?;

    app_bundle(
        &applications.join("WrasseCask.app"),
        "com.wrasse-test.cask",
        "3.1",
    )?;
    let meta = caskroom.join("wrasse-dummy-cask/.metadata/3.1/20260101000000.000/Casks");
    fs::create_dir_all(&meta)?;
    fs::write(
        meta.join("wrasse-dummy-cask.json"),
        r#"{"token":"wrasse-dummy-cask","artifacts":[{"uninstall":[{"quit":"com.wrasse-test.cask"}]},{"app":["WrasseCask.app"]}]}"#,
    )?;
    fs::create_dir_all(caskroom.join("wrasse-dummy-cask/3.1"))?;
    app_bundle(
        &applications.join("WrasseRbCask.app"),
        "com.wrasse-test.rbcask",
        "0.9",
    )?;
    let meta = caskroom.join("wrasse-rb-cask/.metadata/0.9/20250101000000.000/Casks");
    fs::create_dir_all(&meta)?;
    fs::write(
        meta.join("wrasse-rb-cask.rb"),
        "cask \"wrasse-rb-cask\" do\n  version \"0.9\"\n  app \"WrasseRbCask.app\"\nend\n",
    )?;

    app_bundle(
        &applications.join("WrasseSystem.app"),
        "com.apple.wrasse-test",
        "26.0",
    )?;

    app_bundle(
        &applications.join("Wrasse Twin.app"),
        "com.wrasse-test.twin-a",
        "1.0",
    )?;
    app_bundle(
        &home.join("Applications/Wrasse Twin.app"),
        "com.wrasse-test.twin-b",
        "1.0",
    )?;
    put(
        &lib.join("Application Support/Wrasse Twin/shared.db"),
        10_000,
    )?;

    app_bundle(
        &applications.join("Wrasse Suite/WrasseSuite A.app"),
        "com.wrasse-suite.a",
        "1.0",
    )?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        applications.join("WrasseDummy.app"),
        applications.join("Link to Dummy.app"),
    )?;

    put(&lib.join("Caches/com.wrasse-gone.app/blob"), 70_000)?;
    put(
        &lib.join("Application Support/com.wrasse-gone.app/data"),
        20_000,
    )?;
    put(&lib.join("Preferences/com.wrasse-gone.app.plist"), 1_000)?;
    for p in [
        "Caches/com.wrasse-gone.app/blob",
        "Caches/com.wrasse-gone.app",
        "Application Support/com.wrasse-gone.app/data",
        "Application Support/com.wrasse-gone.app",
        "Preferences/com.wrasse-gone.app.plist",
    ] {
        age(&lib.join(p), 60)?;
    }
    put(&lib.join("Caches/com.wrasse-fresh.app/blob"), 5_000)?;
    put(&lib.join("Caches/com.apple.wrasse-cache/blob"), 5_000)?;
    age(&lib.join("Caches/com.apple.wrasse-cache/blob"), 90)?;
    age(&lib.join("Caches/com.apple.wrasse-cache"), 90)?;

    Ok(AppsFixture {
        applications,
        home,
        caskroom,
        system_library,
    })
}
