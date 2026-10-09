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
