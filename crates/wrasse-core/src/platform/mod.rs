//! Platform abstraction (PLAN.md §3). All OS-specific code lives under this module.

#[cfg(target_os = "macos")]
pub(crate) mod macos;
mod volume;
#[cfg(windows)]
mod windows;

pub use volume::{system_volume, VolumeSpace};

use crate::format::UnitSystem;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub trait Platform: Send + Sync {
    /// Human-readable OS name for labels, e.g. "macOS".
    fn name(&self) -> &'static str;
    /// The file manager's name, used in "Reveal in …" labels.
    fn file_manager(&self) -> &'static str;
    fn units(&self) -> UnitSystem;
}

pub fn current() -> &'static dyn Platform {
    #[cfg(target_os = "macos")]
    {
        &macos::MacOs
    }
    #[cfg(windows)]
    {
        &windows::Windows
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        &Unsupported
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
struct Unsupported;

#[cfg(not(any(target_os = "macos", windows)))]
impl Platform for Unsupported {
    fn name(&self) -> &'static str {
        "Unsupported"
    }
    fn file_manager(&self) -> &'static str {
        "file manager"
    }
    fn units(&self) -> UnitSystem {
        UnitSystem::Decimal
    }
}

// ---- Scanner support -------------------------------------------------------------------------

/// Where a whole-volume scan starts, and the prefix to strip so paths read as the user sees them.
/// macOS scans the Data volume (`/System/Volumes/Data`): the sealed system volume is shown as one
/// block, and firmlinked folders (`/Users`, `/Applications`, …) are counted once.
pub fn scan_root() -> (PathBuf, &'static str) {
    #[cfg(target_os = "macos")]
    {
        const DATA: &str = "/System/Volumes/Data";
        if Path::new(DATA).is_dir() {
            return (PathBuf::from(DATA), DATA);
        }
    }
    #[cfg(windows)]
    {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        return (PathBuf::from(format!("{drive}\\")), "");
    }
    #[allow(unreachable_code)]
    (PathBuf::from("/"), "")
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Space on the system volume (or APFS container), and how much of it the OS itself uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volumes {
    pub capacity: u64,
    pub available: u64,
    /// The sealed, read-only OS volume (macOS); 0 where the OS lives in the scanned volume.
    pub os_used: u64,
}

impl Volumes {
    pub fn used(&self) -> u64 {
        self.capacity.saturating_sub(self.available)
    }
}

pub fn volumes() -> Option<Volumes> {
    #[cfg(target_os = "macos")]
    {
        let (capacity, available) = macos::container_space(Path::new("/"))?;
        let os_used = macos::volume_space_used(Path::new("/")).unwrap_or(0);
        return Some(Volumes {
            capacity,
            available,
            os_used,
        });
    }
    #[allow(unreachable_code)]
    system_volume().map(|v| Volumes {
        capacity: v.total,
        available: v.available,
        os_used: 0,
    })
}

/// Runs the calling thread at background priority (scan workers).
pub fn lower_thread_priority() {
    #[cfg(target_os = "macos")]
    macos::lower_thread_priority();
}

/// An iCloud (or other cloud) placeholder with no local data.
pub fn is_dataless(md: &fs::Metadata) -> bool {
    #[cfg(target_os = "macos")]
    return macos::is_dataless(md);
    #[allow(unreachable_code)]
    {
        let _ = md;
        false
    }
}

/// Bytes a file shares with APFS clones, if any.
pub fn clone_shared_bytes(path: &Path, allocated: u64) -> Option<u64> {
    #[cfg(target_os = "macos")]
    return macos::clone_shared_bytes(path, allocated);
    #[allow(unreachable_code)]
    {
        let _ = (path, allocated);
        None
    }
}

/// Whether Wrasse can read everything it needs (macOS Full Disk Access). Always true elsewhere.
pub fn full_disk_access() -> bool {
    #[cfg(target_os = "macos")]
    return home_dir().map_or(true, |h| macos::full_disk_access(&h));
    #[allow(unreachable_code)]
    true
}

/// Opens the OS page where the user grants Full Disk Access.
pub fn open_full_disk_access_settings() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open_full_disk_access_settings();
    #[allow(unreachable_code)]
    Ok(())
}

/// The system volume's name as the file manager shows it ("Macintosh HD").
pub fn volume_name() -> String {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let root = if cfg!(windows) {
        std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()) + "\\"
    } else {
        "/".into()
    };
    disks
        .list()
        .iter()
        .find(|d| d.mount_point() == Path::new(&root))
        .map(|d| d.name().to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "Local Disk".into()
            } else {
                "Macintosh HD".into()
            }
        })
}

/// Executable paths of running processes (to mark apps as running). Read-only.
pub fn running_executables() -> Vec<PathBuf> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .values()
        .filter_map(|p| p.exe().map(Path::to_path_buf))
        .collect()
}

/// An app bundle's identifier (`CFBundleIdentifier`), e.g. "com.apple.dt.Xcode".
pub fn bundle_id(app: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
        return info
            .as_dictionary()?
            .get("CFBundleIdentifier")?
            .as_string()
            .map(str::to_string);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        None
    }
}

/// Tests only: removes a scratch folder a test created under the OS temp dir. Refuses anything
/// else, so it can't be pointed at real data.
#[cfg(test)]
pub fn remove_test_dir(path: &Path) {
    let tmp = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    assert!(
        path.starts_with(&tmp) && name.starts_with("wrasse-"),
        "not a test folder: {}",
        path.display()
    );
    let _ = fs::remove_dir_all(path);
}

/// Tests only: removes one file inside a test scratch folder (same guard as `remove_test_dir`).
#[cfg(test)]
pub fn remove_test_file(path: &Path) {
    let tmp = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    assert!(
        path.starts_with(&tmp) && path.to_string_lossy().contains("/wrasse-"),
        "not a test file: {}",
        path.display()
    );
    let _ = fs::remove_file(path);
}

/// Folders that changed under `watch` since a stored change-log position, for incremental
/// rescans: `Some(changes)`, or `None` when only a full scan is reliable (no log, log reset,
/// events dropped). macOS: FSEvents. Elsewhere: always `None` for now.
pub fn changes_since(
    watch: &Path,
    since: u64,
    log_id: Option<&str>,
) -> Option<Vec<(PathBuf, bool)>> {
    #[cfg(target_os = "macos")]
    {
        let dev = fs::symlink_metadata(watch)
            .ok()
            .map(|m| std::os::unix::fs::MetadataExt::dev(&m))?;
        if macos::fsevents::volume_uuid(dev).as_deref() != log_id {
            return None;
        }
        return macos::fsevents::changes_since(watch, since, std::time::Duration::from_secs(15));
    }
    #[allow(unreachable_code)]
    {
        let _ = (watch, since, log_id);
        None
    }
}

/// The change log's current position and identity for the volume holding `path`.
pub fn change_log_position(path: &Path) -> (u64, Option<String>) {
    #[cfg(target_os = "macos")]
    {
        let dev = fs::symlink_metadata(path)
            .ok()
            .map(|m| std::os::unix::fs::MetadataExt::dev(&m));
        return (
            macos::fsevents::current_event_id(),
            dev.and_then(macos::fsevents::volume_uuid),
        );
    }
    #[allow(unreachable_code)]
    {
        let _ = path;
        (0, None)
    }
}

/// Power source, for pausing scans on low battery (PLAN.md Phase 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    pub on_battery: bool,
    /// Battery charge, 0–100 (100 where there is no battery).
    pub percent: u8,
    pub low_power: bool,
}

pub fn power_state() -> Option<PowerState> {
    #[cfg(target_os = "macos")]
    return macos::power::power_state();
    #[allow(unreachable_code)]
    None
}

/// macOS keeps freed allocations in a cache, and that cache is what a scan leaves behind.
/// `MallocSpaceEfficient=1` makes libmalloc return those pages, but the allocator reads the
/// variable when its zone is created, which is before `main`. Re-exec once so the flag is
/// already set. The web view is started later and inherits it.
pub fn prefer_small_heap() {
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("WRASSE_SMALL_HEAP").is_some() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.env("MallocSpaceEfficient", "1");
        cmd.env("WRASSE_SMALL_HEAP", "1");
        cmd.args(std::env::args_os().skip(1));
        // Replaces this process. If it fails, keep going without the flag.
        let _ = std::os::unix::process::CommandExt::exec(&mut cmd);
    }
}

/// Hands memory freed after a scan back to the OS (macOS keeps freed heap pages otherwise, which
/// shows as idle memory).
pub fn release_free_memory() {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        // SAFETY: a null zone means all zones; goal 0 means release as much as possible.
        unsafe {
            malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
}

// ---- Safety support (Phase 2) -----------------------------------------------------------------

/// How a protected location may be touched (PLAN.md §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// Nothing at or under this path is ever removed.
    Hard,
    /// The folder itself is never removed; items inside only when the user picks them by hand,
    /// only to quarantine, never pre-selected (Documents, Desktop, iCloud Drive).
    HandPickOnly,
    /// The folder itself is never removed; what's inside follows the normal rules (Downloads, …).
    FolderOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protected {
    pub path: PathBuf,
    pub protection: Protection,
}

/// The hardcoded protected locations for this OS. Rules and settings can't override them.
pub fn protected_paths(home: &Path, wrasse_data: &Path) -> Vec<Protected> {
    #[allow(unused_mut)] // extended per OS below
    let mut out = vec![
        Protected {
            path: PathBuf::from("/"),
            protection: Protection::FolderOnly,
        },
        Protected {
            path: home.to_path_buf(),
            protection: Protection::FolderOnly,
        },
        Protected {
            path: wrasse_data.to_path_buf(),
            protection: Protection::Hard,
        },
    ];
    #[cfg(target_os = "macos")]
    out.extend(macos::protect::protected(home));
    out
}

/// Path parts that make everything under them protected wherever they appear (app-managed
/// libraries: Photos, Music, TV).
pub fn protected_name_suffixes() -> &'static [&'static str] {
    &[
        ".photoslibrary",
        ".musiclibrary",
        ".tvlibrary",
        ".imovielibrary",
    ]
}

/// Folders synced to other devices through iCloud (removing an item there removes it everywhere).
pub fn synced_folders(home: &Path) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    return macos::protect::synced(home);
    #[allow(unreachable_code)]
    {
        let _ = home;
        Vec::new()
    }
}

/// The current user's id; items owned by anyone else are never removed.
pub fn current_uid() -> Option<u32> {
    #[cfg(unix)]
    // SAFETY: getuid has no preconditions.
    return Some(unsafe { libc::getuid() });
    #[allow(unreachable_code)]
    None
}

pub fn owner_uid(md: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    return Some(std::os::unix::fs::MetadataExt::uid(md));
    #[allow(unreachable_code)]
    {
        let _ = md;
        None
    }
}

/// Device and file id, to re-check that an item is still the same thing right before acting.
pub fn file_identity(md: &fs::Metadata) -> (u64, u64) {
    #[cfg(unix)]
    return (
        std::os::unix::fs::MetadataExt::dev(md),
        std::os::unix::fs::MetadataExt::ino(md),
    );
    #[allow(unreachable_code)]
    {
        let _ = md;
        (0, 0)
    }
}

/// Whether a process has the file open.
pub fn is_open(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    return macos::commands::is_open(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        false
    }
}

/// Keeps the machine awake while the returned guard lives.
pub struct Awake {
    #[cfg(target_os = "macos")]
    _inner: macos::awake::Awake,
}

#[cfg(target_os = "macos")]
pub fn keep_awake(reason: &str) -> Awake {
    Awake {
        _inner: macos::awake::keep_awake(reason),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn keep_awake(_reason: &str) -> Awake {
    Awake {}
}

/// Keeps one of Wrasse's own folders out of backups and search indexing.
pub fn exclude_from_backups(dir: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::exclude_from_backups(dir);
    #[allow(unreachable_code)]
    {
        let _ = dir;
        Ok(())
    }
}

pub fn is_excluded_from_backups(dir: &Path) -> bool {
    #[cfg(target_os = "macos")]
    return macos::commands::is_excluded_from_backups(dir);
    #[allow(unreachable_code)]
    {
        let _ = dir;
        false
    }
}

/// Delegated OS actions (PLAN.md §4.1). Only `safety::delegate` calls this, after validating the
/// action (`brew` must be one of [`brew_binaries`] and owned by the user, and so on).
pub fn run_delegated(action: &crate::safety::Delegated, brew: Option<&Path>) -> io::Result<()> {
    // Tests exercise `safety::delegate` with temporary homes; they must never reach a real OS
    // action (emptying the user's actual Trash, uninstalling a real app).
    #[cfg(test)]
    {
        let _ = (action, brew);
        return Err(io::Error::other("delegated actions don't run in tests"));
    }
    #[allow(unreachable_code)]
    {
        #[cfg(target_os = "macos")]
        use crate::safety::Delegated;
        #[cfg(target_os = "macos")]
        return match action {
            Delegated::EmptyTrash => macos::commands::empty_trash_via_finder(),
            Delegated::BrewUninstallCask { token, .. } => match brew {
                Some(b) => macos::commands::brew_uninstall_cask(b, token),
                None => Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Homebrew isn't installed",
                )),
            },
            Delegated::FinderMoveToTrash { path } => macos::commands::finder_move_to_trash(path),
            Delegated::OllamaRm { model } => ollama_delete(model),
            Delegated::OllamaStop { model } => ollama_stop(model),
        };
        #[allow(unreachable_code)]
        {
            let _ = (action, brew);
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }
    }
}

/// Whether a Homebrew cask token is well formed (lowercase letters, digits, `-@._+`).
pub fn valid_cask_token(token: &str) -> bool {
    #[cfg(target_os = "macos")]
    return macos::apps::valid_token(token);
    #[allow(unreachable_code)]
    {
        let _ = token;
        false
    }
}

/// Where Homebrew may live; only these are ever run, and only when owned by the current user.
pub fn brew_binaries() -> &'static [&'static str] {
    &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
}

/// Homebrew's Caskrooms, next to the binaries above.
pub fn caskrooms() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/Caskroom"),
        PathBuf::from("/usr/local/Caskroom"),
    ]
}

/// Folders apps are installed into: (`/Applications`, `~/Applications`).
pub fn app_dirs(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Applications"), home.join("Applications")]
    } else {
        Vec::new()
    }
}

/// Installed apps with their leftovers (PLAN.md §6.4). Read-only.
pub fn installed_apps(home: &Path) -> Vec<crate::apps::App> {
    #[cfg(target_os = "macos")]
    return macos::apps::discover(&macos::apps::Places::system(home), &macos::apps::System);
    #[allow(unreachable_code)]
    {
        let _ = home;
        Vec::new()
    }
}

/// The app at `bundle`, discovered fresh.
pub fn app_at(home: &Path, bundle: &Path) -> Option<crate::apps::App> {
    #[cfg(target_os = "macos")]
    return macos::apps::app_at(
        &macos::apps::Places::system(home),
        &macos::apps::System,
        bundle,
    );
    #[allow(unreachable_code)]
    {
        let _ = (home, bundle);
        None
    }
}

/// Data folders whose app is gone (docs/phase3-plan.md decision 5).
pub fn orphaned_leftovers(home: &Path, apps: &[crate::apps::App]) -> Vec<crate::apps::Orphan> {
    #[cfg(target_os = "macos")]
    return macos::apps::orphans(
        &macos::apps::Places::system(home),
        &macos::apps::System,
        apps,
        &running_bundle_ids(),
    );
    #[allow(unreachable_code)]
    {
        let _ = (home, apps);
        Vec::new()
    }
}

/// The cask token that installed this app bundle, if any.
pub fn cask_of(app: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let name = app.file_name()?.to_string_lossy().into_owned();
        return macos::apps::cask_apps(&caskrooms()).remove(&name);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        None
    }
}

/// When a file or app was last opened (Unix seconds), from Spotlight.
pub fn last_used(path: &Path) -> Option<i64> {
    #[cfg(target_os = "macos")]
    return macos::spotlight::last_used(path);
    #[allow(unreachable_code)]
    {
        let _ = path;
        None
    }
}

/// Politely asks an app to quit (never forces it). Returns once the request is sent, or after
/// 10 s if it couldn't be sent. The caller waits for the app to exit.
pub fn quit_app(bundle_id: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::commands::quit_app(bundle_id);
    #[allow(unreachable_code)]
    {
        let _ = bundle_id;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Where the Trash is, to show its size before emptying.
pub fn trash_dir(home: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(home.join(".Trash"))
    } else {
        None
    }
}

/// Bundle ids of running apps (macOS), for rules that need their app closed.
pub fn running_bundle_ids() -> Vec<String> {
    let mut ids: Vec<String> = running_executables()
        .iter()
        .filter_map(|exe| {
            let s = exe.to_string_lossy();
            let end = s.find(".app/Contents/MacOS/")?;
            bundle_id(Path::new(&s[..end + 4]))
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

const OLLAMA_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
    11434,
);

/// Model names the local Ollama API accepts. Letters, digits, and `: / . _ - @` only.
/// No `..`, so a name can't be a path.
pub fn valid_ollama_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 || name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '_' | '-' | '@'))
}

/// True when something accepts TCP on 127.0.0.1:11434. The connection is closed immediately.
pub fn ollama_reachable() -> bool {
    TcpStream::connect_timeout(&OLLAMA_ADDR, Duration::from_millis(300)).is_ok()
}

/// GET one allowlisted path. Read-only.
pub fn ollama_get(path: &str) -> io::Result<String> {
    let (status, body) = ollama_http("GET", path, None, 800)?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(io::Error::other(format!("ollama replied {status}")))
    }
}

/// POST `/api/delete`, the request `ollama rm` sends. The name is checked first.
pub fn ollama_delete(name: &str) -> io::Result<()> {
    ollama_post("/api/delete", &format!(r#"{{"name":"{name}"}}"#), name)
}

/// POST `/api/generate` with `keep_alive: 0`, the request `ollama stop` sends.
pub fn ollama_stop(name: &str) -> io::Result<()> {
    ollama_post(
        "/api/generate",
        &format!(r#"{{"model":"{name}","keep_alive":0}}"#),
        name,
    )
}

fn ollama_post(path: &str, body: &str, name: &str) -> io::Result<()> {
    if !valid_ollama_name(name) {
        return Err(io::Error::other("that isn't a model name"));
    }
    let (status, text) = ollama_http("POST", path, Some(body.as_bytes()), 20_000)?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(io::Error::other(format!("ollama replied {status}: {text}")))
    }
}

fn ollama_http(
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    read_ms: u64,
) -> io::Result<(u16, String)> {
    if method != "GET" && method != "POST" {
        return Err(io::Error::other("that request isn't allowed"));
    }
    if !matches!(
        path,
        "/api/ps" | "/api/delete" | "/api/generate" | "/api/version"
    ) {
        return Err(io::Error::other("that request isn't allowed"));
    }
    let mut stream = TcpStream::connect_timeout(&OLLAMA_ADDR, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_millis(read_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(300)))?;
    let mut req =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nConnection: close\r\n");
    if let Some(bytes) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", bytes.len()));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    if let Some(bytes) = body {
        stream.write_all(bytes)?;
    }
    let raw = read_http(&mut stream)?;
    let (head, rest) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::other("ollama sent no headers"))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    Ok((status, rest.to_string()))
}

/// Reads until the server closes or the timeout. A timeout after headers is the reply.
fn read_http(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() > 8_000_000 {
            break;
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                let headers = buf.windows(4).any(|w| w == b"\r\n\r\n");
                if headers {
                    break;
                }
                if buf.is_empty() {
                    return Err(e);
                }
                break;
            }
        }
    }
    if buf.is_empty() {
        return Err(io::Error::other("empty reply"));
    }
    String::from_utf8(buf).map_err(|_| io::Error::other("ollama sent bytes that aren't text"))
}

/// Sum of resident memory for processes whose name contains one of `needles`. Read-only.
pub fn process_memory(needles: &[&str]) -> u64 {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet),
    );
    let mut total = 0u64;
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if needles.iter().any(|n| name.contains(n) || exe.contains(n)) {
            total += process.memory();
        }
    }
    total
}
