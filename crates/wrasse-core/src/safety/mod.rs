//! The only destructive entrypoints in Wrasse: [`remove`] and [`delegate`] (PLAN.md §4.1).
//! Nothing outside `safety/`, `quarantine/` and `platform/` may delete, rename or spawn
//! processes; `scripts/check-forbidden-apis.sh` enforces this (CI and the pre-commit hook).
//!
//! Removal is two steps, so the user always sees what will happen before it does:
//! 1. [`prepare`] checks every item against the rules below and returns a [`Plan`]: what would be
//!    removed, how many bytes, what was skipped and why, and whether it can be undone.
//! 2. [`remove`] re-checks each planned item right before acting (same file, not swapped for a
//!    link, and for "your call" items unchanged) and then moves it to quarantine or deletes it,
//!    or, in dry-run, only reports what it would do. Every outcome goes to the audit log.
//!
//! Checks (PLAN.md §4.2–4.3): protected locations (hardcoded per OS in `platform`), the user's
//! never-touch list, the startup disk only, the current user's files only, no iCloud-only
//! (dataless) items, no symlink escapes out of a rule's root, open files left alone, "Delete now"
//! only for items labeled Safe, items in Documents/Desktop/iCloud Drive only when picked by hand
//! and only to quarantine. Platform-neutral: OS specifics come from `platform`.

pub mod audit;
pub mod exclusions;

use crate::platform::{self, Protected, Protection};
use crate::quarantine::Quarantine;
use audit::{AuditLog, Record};
use exclusions::Exclusions;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

/// One operation at a time (PLAN.md §4.3 rule 9).
static OPERATION: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Moved aside, restorable until the quarantine is emptied.
    Quarantine,
    /// Deleted for good. Only for items labeled Safe. Not undoable.
    DeleteNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    Safe,
    YourCall,
}

/// One thing the user (or a rule) wants removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub path: PathBuf,
    pub safety: Safety,
    #[serde(default)]
    pub rule_id: Option<String>,
    /// For rule items: the rule's root; the item must stay inside it after resolving links.
    #[serde(default)]
    pub root: Option<PathBuf>,
    /// The user picked this item themselves (needed for Documents, Desktop and iCloud Drive).
    #[serde(default)]
    pub hand_picked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Protected,
    /// In Documents, Desktop or iCloud Drive and not picked by hand (or asked for Delete now).
    HandPickOnly,
    Excluded,
    OtherVolume,
    OtherUser,
    InCloudOnly,
    OutsideRoot,
    NotFound,
    /// It changed between the preview and the action (different file, now a link, or edited).
    Changed,
    InUse,
    /// An app that's open (its bundle, while a process runs from it).
    AppRunning,
    /// "Delete now" is only for items labeled Safe.
    NotSafeForDeleteNow,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Warning {
    /// iCloud syncs this folder: removing it here removes it on the user's other devices too.
    SyncedToICloud,
}

/// What an item was when planned, to re-check right before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
    pub uid: Option<u32>,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub len: u64,
    pub mtime: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Planned {
    pub request: Request,
    pub identity: Identity,
    pub bytes: u64,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub mode: Mode,
    pub items: Vec<Planned>,
    pub skipped: Vec<Skipped>,
    pub bytes: u64,
    /// False for Delete now: shown as "Not undoable" before the user confirms.
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub path: PathBuf,
    pub bytes: u64,
    /// The quarantine entry, for restore.
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub done: Vec<Done>,
    pub skipped: Vec<Skipped>,
    pub bytes: u64,
    pub dry_run: bool,
}

/// Everything the checks need, gathered once.
pub struct Context {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub protected: Vec<Protected>,
    pub synced: Vec<PathBuf>,
    pub exclusions: Exclusions,
    pub quarantine: Quarantine,
    pub audit: AuditLog,
    /// Device of the startup (data) volume; v1 cleans only this disk.
    pub startup_dev: u64,
    pub uid: Option<u32>,
}

impl Context {
    pub fn new(home: PathBuf, data_dir: PathBuf) -> io::Result<Self> {
        // Compared against resolved item paths, so resolve these too.
        fs::create_dir_all(&data_dir)?;
        let home = resolve_dir(&home)?;
        let data_dir = resolve_dir(&data_dir)?;
        let quarantine = Quarantine::open(data_dir.join("Quarantine"))?;
        let startup_dev = fs::symlink_metadata(&home).map(|m| platform::file_identity(&m).0)?;
        Ok(Context {
            protected: platform::protected_paths(&home, &data_dir),
            synced: platform::synced_folders(&home),
            exclusions: Exclusions::load(&data_dir),
            audit: AuditLog::new(&data_dir),
            uid: platform::current_uid(),
            home,
            data_dir,
            quarantine,
            startup_dev,
        })
    }
}

fn identity(md: &fs::Metadata) -> Identity {
    let (dev, ino) = platform::file_identity(md);
    Identity {
        dev,
        ino,
        uid: platform::owner_uid(md),
        is_dir: md.is_dir(),
        is_symlink: md.file_type().is_symlink(),
        len: md.len(),
        mtime: md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs()),
    }
}

fn resolve_dir(dir: &Path) -> io::Result<PathBuf> {
    let d = fs::canonicalize(dir)?;
    Ok(d.strip_prefix("/System/Volumes/Data")
        .map(|p| Path::new("/").join(p))
        .unwrap_or(d))
}

/// The path with its parent folders' links resolved (the item itself is not followed, so a link
/// is removed as a link). Firmlinked data-volume paths read as the user sees them.
fn resolve(path: &Path) -> Option<PathBuf> {
    let parent = fs::canonicalize(path.parent()?).ok()?;
    let parent = parent
        .strip_prefix("/System/Volumes/Data")
        .map(|p| Path::new("/").join(p))
        .unwrap_or(parent);
    Some(parent.join(path.file_name()?))
}

/// Why a path can't be removed at all, or `None` when only the per-mode rules apply.
fn protection_of(ctx: &Context, path: &Path) -> Option<Protection> {
    if path.components().any(|c| {
        platform::protected_name_suffixes()
            .iter()
            .any(|s| c.as_os_str().to_string_lossy().ends_with(s))
    }) {
        return Some(Protection::Hard);
    }
    let mut strongest: Option<Protection> = None;
    for p in &ctx.protected {
        let hit = match p.protection {
            Protection::Hard => path.starts_with(&p.path),
            // The folder itself; its contents are fine (FolderOnly) or hand-pick-only.
            Protection::FolderOnly => path == p.path,
            Protection::HandPickOnly => path.starts_with(&p.path),
        };
        if !hit {
            continue;
        }
        let this = if p.protection == Protection::HandPickOnly && path == p.path {
            Protection::Hard
        } else {
            p.protection
        };
        strongest = Some(match (strongest, this) {
            (Some(Protection::Hard), _) | (_, Protection::Hard) => Protection::Hard,
            (Some(Protection::FolderOnly), _) | (_, Protection::FolderOnly) => {
                Protection::FolderOnly
            }
            _ => Protection::HandPickOnly,
        });
    }
    strongest
}

/// Checks every request and returns what would happen. Touches nothing.
pub fn prepare(ctx: &Context, requests: &[Request], mode: Mode) -> Plan {
    let mut plan = Plan {
        mode,
        items: Vec::new(),
        skipped: Vec::new(),
        bytes: 0,
        undoable: mode == Mode::Quarantine,
    };
    for req in requests {
        match check(ctx, req, mode) {
            Ok(planned) => {
                plan.bytes += planned.bytes;
                plan.items.push(planned);
            }
            Err(reason) => plan.skipped.push(Skipped {
                path: req.path.clone(),
                reason,
            }),
        }
    }
    plan
}

fn check(ctx: &Context, req: &Request, mode: Mode) -> Result<Planned, Reason> {
    let (identity, warnings) = vet(ctx, req, mode)?;
    Ok(Planned {
        request: req.clone(),
        identity,
        bytes: crate::quarantine::size_on_disk(&req.path),
        warnings,
    })
}

/// Every check except sizing; run when planning and again right before acting, so an item that
/// became protected, excluded, open or iCloud-only after the preview is still refused.
fn vet(ctx: &Context, req: &Request, mode: Mode) -> Result<(Identity, Vec<Warning>), Reason> {
    let md = fs::symlink_metadata(&req.path).map_err(|_| Reason::NotFound)?;
    let path = resolve(&req.path).ok_or(Reason::NotFound)?;
    match protection_of(ctx, &path) {
        Some(Protection::Hard) | Some(Protection::FolderOnly) => return Err(Reason::Protected),
        Some(Protection::HandPickOnly) if !req.hand_picked || mode == Mode::DeleteNow => {
            return Err(Reason::HandPickOnly)
        }
        _ => {}
    }
    if ctx.exclusions.contains(&path) || ctx.exclusions.contains(&req.path) {
        return Err(Reason::Excluded);
    }
    if let Some(root) = &req.root {
        let root = fs::canonicalize(root).map_err(|_| Reason::OutsideRoot)?;
        let root = root
            .strip_prefix("/System/Volumes/Data")
            .map(|p| Path::new("/").join(p))
            .unwrap_or(root);
        if !path.starts_with(&root) || path == root {
            return Err(Reason::OutsideRoot);
        }
    }
    let id = identity(&md);
    if id.dev != ctx.startup_dev {
        return Err(Reason::OtherVolume);
    }
    if ctx.uid.is_some() && id.uid != ctx.uid {
        return Err(Reason::OtherUser);
    }
    if platform::is_dataless(&md) {
        return Err(Reason::InCloudOnly);
    }
    if mode == Mode::DeleteNow && req.safety != Safety::Safe {
        return Err(Reason::NotSafeForDeleteNow);
    }
    if req.safety == Safety::YourCall && !id.is_dir && platform::is_open(&req.path) {
        return Err(Reason::InUse);
    }
    if is_app_bundle(&path) && app_running(&req.path, &path) {
        return Err(Reason::AppRunning);
    }
    let mut warnings = Vec::new();
    if ctx.synced.iter().any(|s| path.starts_with(s)) {
        warnings.push(Warning::SyncedToICloud);
    }
    Ok((id, warnings))
}

/// Same file as planned? Safe cache folders may change size; "your call" items must not change.
fn still_same(planned: &Planned) -> bool {
    let Ok(md) = fs::symlink_metadata(&planned.request.path) else {
        return false;
    };
    let now = identity(&md);
    let was = planned.identity;
    let same = now.dev == was.dev
        && now.ino == was.ino
        && now.uid == was.uid
        && now.is_dir == was.is_dir
        && now.is_symlink == was.is_symlink;
    let unchanged = planned.request.safety == Safety::Safe && was.is_dir
        || (now.len == was.len && now.mtime == was.mtime);
    same && unchanged
}

/// Acts on a prepared plan: re-checks each item, then quarantines or deletes it (or, in dry-run,
/// reports what it would do). One operation at a time; the machine stays awake meanwhile.
pub fn remove(ctx: &Context, plan: &Plan, dry_run: bool) -> io::Result<Report> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    let _awake = platform::keep_awake("Wrasse is cleaning up");
    let mut report = Report {
        dry_run,
        skipped: plan.skipped.clone(),
        ..Default::default()
    };
    let mut batch = if dry_run || plan.mode == Mode::DeleteNow {
        None
    } else {
        Some(ctx.quarantine.begin()?)
    };
    let action = match plan.mode {
        Mode::Quarantine => "quarantine",
        Mode::DeleteNow => "delete",
    };
    for item in &plan.items {
        let req = &item.request;
        let target = req.path.to_string_lossy();
        let refused = match vet(ctx, req, plan.mode) {
            Err(reason) => Some(reason),
            Ok(_) if !still_same(item) => Some(Reason::Changed),
            Ok(_) => None,
        };
        if let Some(reason) = refused {
            report.skipped.push(Skipped {
                path: req.path.clone(),
                reason,
            });
            log(
                ctx,
                "skip",
                &target,
                item.bytes,
                req.rule_id.as_deref(),
                dry_run,
                if reason == Reason::Changed {
                    "changed since preview"
                } else {
                    "refused at the last moment"
                },
            );
            continue;
        }
        if dry_run {
            report.done.push(Done {
                path: req.path.clone(),
                bytes: item.bytes,
                entry: None,
            });
            report.bytes += item.bytes;
            log(
                ctx,
                action,
                &target,
                item.bytes,
                req.rule_id.as_deref(),
                true,
                "ok",
            );
            continue;
        }
        let result = match (&mut batch, plan.mode) {
            (Some(w), Mode::Quarantine) => w
                .store(&req.path, item.bytes, req.rule_id.as_deref())
                .map(Some),
            _ => delete(&req.path, item.identity.is_dir && !item.identity.is_symlink).map(|_| None),
        };
        match result {
            Ok(entry) => {
                report.done.push(Done {
                    path: req.path.clone(),
                    bytes: item.bytes,
                    entry,
                });
                report.bytes += item.bytes;
                log(
                    ctx,
                    action,
                    &target,
                    item.bytes,
                    req.rule_id.as_deref(),
                    false,
                    "ok",
                );
            }
            Err(e) => {
                report.skipped.push(Skipped {
                    path: req.path.clone(),
                    reason: Reason::Failed,
                });
                log(
                    ctx,
                    action,
                    &target,
                    item.bytes,
                    req.rule_id.as_deref(),
                    false,
                    &e.to_string(),
                );
            }
        }
    }
    if let Some(w) = batch {
        w.finish()?;
    }
    Ok(report)
}

fn delete(path: &Path, is_dir: bool) -> io::Result<()> {
    if is_dir {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn log(
    ctx: &Context,
    action: &str,
    target: &str,
    bytes: u64,
    rule_id: Option<&str>,
    dry_run: bool,
    result: &str,
) {
    // An audit line that can't be written must not stop the cleanup; it's reported in the result.
    let _ = ctx.audit.write(Record {
        at: 0,
        action,
        target,
        bytes,
        rule_id,
        dry_run,
        result,
    });
}

/// Restores quarantined items (by entry id).
pub fn restore(ctx: &Context, ids: &[String]) -> io::Result<Vec<(String, PathBuf)>> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    let restored = ctx.quarantine.restore(ids)?;
    for (_, path) in &restored {
        log(
            ctx,
            "restore",
            &path.to_string_lossy(),
            0,
            None,
            false,
            "ok",
        );
    }
    Ok(restored)
}

/// Empties quarantined items for good (the tray's "Empty now"). Not undoable.
pub fn purge(ctx: &Context, ids: &[String], dry_run: bool) -> io::Result<u64> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    if dry_run {
        let bytes = ctx
            .quarantine
            .stored()
            .iter()
            .filter(|(_, e)| ids.contains(&e.id))
            .map(|(_, e)| e.bytes)
            .sum();
        log(
            ctx,
            "purge",
            &format!("{} quarantined items", ids.len()),
            bytes,
            None,
            true,
            "ok",
        );
        return Ok(bytes);
    }
    let purged = ctx.quarantine.purge(ids)?;
    let bytes = purged.iter().map(|(_, b)| b).sum();
    log(
        ctx,
        "purge",
        &format!("{} quarantined items", purged.len()),
        bytes,
        None,
        false,
        "ok",
    );
    Ok(bytes)
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "app")
}

fn app_running(given: &Path, resolved: &Path) -> bool {
    platform::running_executables()
        .iter()
        .any(|e| e.starts_with(given) || e.starts_with(resolved))
}

/// Delegated OS actions (PLAN.md §4.1): the OS or a tool deletes on Wrasse's behalf. This is the
/// whole allowlist; each is validated in [`delegate`] before anything runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Delegated {
    /// Empty the Trash (Finder on macOS). Not undoable.
    EmptyTrash,
    /// `brew uninstall --cask <token>` for the app that cask installed. Not undoable.
    #[serde(rename_all = "camelCase")]
    BrewUninstallCask { token: String, app: PathBuf },
    /// Move an app the user can't move themselves (root-owned) to their Trash through Finder,
    /// which asks for the password. Put Back works.
    #[serde(rename_all = "camelCase")]
    FinderMoveToTrash { path: PathBuf },
}

/// Why a delegated action was refused before running.
fn refuse(why: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, why.into())
}

/// Checks an app bundle a delegated action would remove: an `.app` directly in (or one folder
/// inside) an Applications folder, on the startup disk, not protected, not on the never-touch
/// list, not part of the OS, and not running. Returns its size.
fn check_app_target(ctx: &Context, app: &Path) -> io::Result<u64> {
    let md = fs::symlink_metadata(app).map_err(|_| refuse("the app isn't there"))?;
    if md.file_type().is_symlink() || !md.is_dir() || !is_app_bundle(app) {
        return Err(refuse("not an app bundle"));
    }
    let path = resolve(app).ok_or_else(|| refuse("the app isn't there"))?;
    let dirs: Vec<PathBuf> = platform::app_dirs(&ctx.home)
        .iter()
        .filter_map(|d| resolve_dir(d).ok())
        .collect();
    let parent = path.parent().unwrap_or(Path::new("/"));
    let in_dirs = dirs
        .iter()
        .any(|d| parent == d || parent.parent() == Some(d.as_path()));
    if !in_dirs {
        return Err(refuse("only apps in an Applications folder"));
    }
    if matches!(protection_of(ctx, &path), Some(Protection::Hard)) {
        return Err(refuse("protected"));
    }
    if ctx.exclusions.contains(&path) || ctx.exclusions.contains(app) {
        return Err(refuse(format!(
            "{} is on your never-touch list",
            app.display()
        )));
    }
    if platform::file_identity(&md).0 != ctx.startup_dev {
        return Err(refuse("not on the startup disk"));
    }
    let system = platform::bundle_id(app).is_some_and(|id| id.starts_with("com.apple."))
        && !app.join("Contents/_MASReceipt/receipt").is_file();
    if system {
        return Err(refuse("part of the system"));
    }
    if app_running(app, &path) {
        return Err(io::Error::other("the app is open; quit it first"));
    }
    Ok(crate::quarantine::size_on_disk(app))
}

/// The Homebrew binary to run: one of the fixed places, a file owned by the current user.
fn brew_binary(ctx: &Context) -> Option<PathBuf> {
    platform::brew_binaries()
        .iter()
        .map(PathBuf::from)
        .find(|b| {
            fs::metadata(b).is_ok_and(|m| {
                m.is_file() && (ctx.uid.is_none() || platform::owner_uid(&m) == ctx.uid)
            })
        })
}

/// A cask token and app pair is valid when the token is well formed and that cask's metadata
/// lists this app.
pub fn cask_matches(token: &str, app: &Path, cask_of: &dyn Fn(&Path) -> Option<String>) -> bool {
    platform::valid_cask_token(token) && cask_of(app).as_deref() == Some(token)
}

pub fn delegate(ctx: &Context, action: Delegated, dry_run: bool) -> io::Result<u64> {
    delegate_with(ctx, action, dry_run, &platform::cask_of)
}

/// [`delegate`] with the cask lookup injected (tests use a fixture Caskroom).
pub fn delegate_with(
    ctx: &Context,
    action: Delegated,
    dry_run: bool,
    cask_of: &dyn Fn(&Path) -> Option<String>,
) -> io::Result<u64> {
    let _one = OPERATION
        .lock()
        .map_err(|_| io::Error::other("another cleanup is running"))?;
    let _awake = platform::keep_awake("Wrasse is cleaning up");
    let checked: io::Result<(String, u64, Option<PathBuf>)> = match &action {
        Delegated::EmptyTrash => {
            let place = platform::trash_dir(&ctx.home);
            match &place {
                // The never-touch list applies here too: the OS would delete everything in there.
                Some(p) if ctx.exclusions.contains(p) => Err(refuse(format!(
                    "{} is on your never-touch list",
                    p.display()
                ))),
                _ => Ok((
                    "empty Trash".to_string(),
                    place.map_or(0, |t| crate::quarantine::size_on_disk(&t)),
                    None,
                )),
            }
        }
        Delegated::BrewUninstallCask { token, app } => {
            if !cask_matches(token, app, cask_of) {
                Err(refuse(format!(
                    "{} wasn't installed by the cask \"{token}\"",
                    app.display()
                )))
            } else {
                check_app_target(ctx, app).and_then(|bytes| {
                    let brew = brew_binary(ctx);
                    if brew.is_none() && !dry_run {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "Homebrew isn't installed",
                        ));
                    }
                    Ok((
                        format!("brew uninstall --cask {token} ({})", app.display()),
                        bytes,
                        brew,
                    ))
                })
            }
        }
        Delegated::FinderMoveToTrash { path } => check_app_target(ctx, path).map(|bytes| {
            (
                format!("Finder: move {} to the Trash", path.display()),
                bytes,
                None,
            )
        }),
    };
    let (target, bytes, brew) = match checked {
        Ok(c) => c,
        Err(e) => {
            log(
                ctx,
                "delegate",
                &format!("{action:?}"),
                0,
                None,
                dry_run,
                &format!("refused: {e}"),
            );
            return Err(e);
        }
    };
    if dry_run {
        log(ctx, "delegate", &target, bytes, None, true, "ok");
        return Ok(bytes);
    }
    let result = platform::run_delegated(&action, brew.as_deref());
    log(
        ctx,
        "delegate",
        &target,
        bytes,
        None,
        false,
        &result
            .as_ref()
            .map_or_else(|e| e.to_string(), |_| "ok".into()),
    );
    result.map(|_| bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::SystemTime;

    fn setup(tag: &str) -> (PathBuf, Context) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("wrasse-safety-{tag}-{nanos}"));
        let home = base.join("home");
        for d in [
            "Documents",
            "Desktop",
            "Downloads",
            "Library/Caches/app",
            "Library/Keychains",
        ] {
            fs::create_dir_all(home.join(d)).unwrap();
        }
        fs::write(home.join("Library/Caches/app/blob"), vec![1u8; 4000]).unwrap();
        fs::write(home.join("Documents/report.pdf"), b"report").unwrap();
        fs::write(home.join("Downloads/old.dmg"), vec![2u8; 3000]).unwrap();
        fs::write(home.join("Library/Keychains/login.keychain-db"), b"secret").unwrap();
        let ctx = Context::new(home, base.join("data")).unwrap();
        let base = fs::canonicalize(&base).unwrap();
        (base, ctx)
    }

    fn req(path: PathBuf, safety: Safety) -> Request {
        Request {
            path,
            safety,
            rule_id: None,
            root: None,
            hand_picked: false,
        }
    }

    #[test]
    fn protected_and_hand_pick_rules() {
        let (base, ctx) = setup("protect");
        let h = &ctx.home;
        let plan = prepare(
            &ctx,
            &[
                req(
                    h.join("Library/Keychains/login.keychain-db"),
                    Safety::YourCall,
                ),
                req(h.join("Documents"), Safety::YourCall),
                req(h.join("Documents/report.pdf"), Safety::YourCall),
                req(h.join("Downloads"), Safety::YourCall),
                req(h.join("Downloads/old.dmg"), Safety::YourCall),
                req(h.join("Library/Caches/app"), Safety::Safe),
                req(h.clone(), Safety::YourCall),
                req(ctx.data_dir.join("Quarantine"), Safety::YourCall),
            ],
            Mode::Quarantine,
        );
        let skipped: Vec<_> = plan
            .skipped
            .iter()
            .map(|s| (s.path.strip_prefix(&base).unwrap().to_path_buf(), s.reason))
            .collect();
        assert!(skipped.contains(&(
            PathBuf::from("home/Library/Keychains/login.keychain-db"),
            Reason::Protected
        )));
        assert!(skipped.contains(&(PathBuf::from("home/Documents"), Reason::Protected)));
        assert!(skipped.contains(&(
            PathBuf::from("home/Documents/report.pdf"),
            Reason::HandPickOnly
        )));
        assert!(skipped.contains(&(PathBuf::from("home/Downloads"), Reason::Protected)));
        assert!(skipped.contains(&(PathBuf::from("home"), Reason::Protected)));
        assert!(skipped.contains(&(PathBuf::from("data/Quarantine"), Reason::Protected)));
        assert_eq!(plan.items.len(), 2, "old.dmg and the cache folder");

        // Picked by hand, a Documents item may go to quarantine, never Delete now.
        let mut picked = req(h.join("Documents/report.pdf"), Safety::YourCall);
        picked.hand_picked = true;
        assert_eq!(
            prepare(&ctx, std::slice::from_ref(&picked), Mode::Quarantine)
                .items
                .len(),
            1
        );
        assert_eq!(
            prepare(&ctx, &[picked], Mode::DeleteNow).skipped[0].reason,
            Reason::HandPickOnly
        );
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn delete_now_only_for_safe_and_dry_run_touches_nothing() {
        let (base, ctx) = setup("modes");
        let h = &ctx.home;
        let your_call = prepare(
            &ctx,
            &[req(h.join("Downloads/old.dmg"), Safety::YourCall)],
            Mode::DeleteNow,
        );
        assert_eq!(your_call.skipped[0].reason, Reason::NotSafeForDeleteNow);
        assert!(!your_call.undoable);

        let plan = prepare(
            &ctx,
            &[req(h.join("Library/Caches/app"), Safety::Safe)],
            Mode::DeleteNow,
        );
        let dry = remove(&ctx, &plan, true).unwrap();
        assert_eq!(dry.done.len(), 1);
        assert!(
            h.join("Library/Caches/app/blob").exists(),
            "dry-run changes nothing"
        );
        let real = remove(&ctx, &plan, false).unwrap();
        assert_eq!(real.done.len(), 1);
        assert!(!h.join("Library/Caches/app").exists());
        assert!(ctx.audit.recent(10).len() >= 2);
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn changed_items_are_skipped_at_the_last_moment() {
        let (base, ctx) = setup("changed");
        let file = ctx.home.join("Downloads/old.dmg");
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        // The file is replaced between preview and action (a different file, same name).
        crate::platform::remove_test_file(&file);
        fs::write(&file, b"something else").unwrap();
        let report = remove(&ctx, &plan, false).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::Changed);
        assert_eq!(fs::read(&file).unwrap(), b"something else");
        crate::platform::remove_test_dir(&base);
    }

    fn skip_logged(ctx: &Context, path: &Path, result: &str) -> bool {
        ctx.audit.recent(50).iter().any(|r| {
            r["action"] == "skip"
                && r["target"] == path.to_string_lossy().as_ref()
                && r["result"] == result
        })
    }

    #[test]
    fn edited_items_are_skipped_and_reported() {
        let (base, ctx) = setup("edited");
        let file = ctx.home.join("Downloads/old.dmg");
        // Same file (same inode), new contents.
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(b"more")
            .unwrap();
        let report = remove(&ctx, &plan, false).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::Changed);
        assert!(skip_logged(&ctx, &file, "changed since preview"));
        // Same size, only the modified date moved: still "changed".
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(SystemTime::now() - std::time::Duration::from_secs(86_400 * 3))
            .unwrap();
        let report = remove(&ctx, &plan, false).unwrap();
        assert_eq!(report.skipped[0].reason, Reason::Changed);
        assert_eq!(
            fs::metadata(&file).unwrap().len(),
            3004,
            "left exactly as it was"
        );
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn items_swapped_for_a_link_are_skipped_and_reported() {
        let (base, ctx) = setup("swapped");
        let h = ctx.home.clone();
        // A "your call" file becomes a link to a private file.
        let file = h.join("Downloads/old.dmg");
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        crate::platform::remove_test_file(&file);
        std::os::unix::fs::symlink(h.join("Documents/report.pdf"), &file).unwrap();
        let report = remove(&ctx, &plan, false).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::Changed);
        assert!(skip_logged(&ctx, &file, "changed since preview"));
        assert!(fs::symlink_metadata(&file)
            .unwrap()
            .file_type()
            .is_symlink());
        // A safe cache folder becomes a link to the Keychains folder.
        let cache = h.join("Library/Caches/app");
        let mut r = req(cache.clone(), Safety::Safe);
        r.root = Some(h.join("Library/Caches"));
        let plan = prepare(&ctx, &[r], Mode::DeleteNow);
        assert_eq!(plan.items.len(), 1);
        crate::platform::remove_test_file(&cache.join("blob"));
        fs::remove_dir(&cache).unwrap();
        std::os::unix::fs::symlink(h.join("Library/Keychains"), &cache).unwrap();
        let report = remove(&ctx, &plan, false).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::Changed);
        assert_eq!(
            fs::read(h.join("Library/Keychains/login.keychain-db")).unwrap(),
            b"secret"
        );
        assert_eq!(fs::read(h.join("Documents/report.pdf")).unwrap(), b"report");
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn never_touch_is_refused_by_remove_and_delegate() {
        let (base, mut ctx) = setup("nevertouch");
        let h = ctx.home.clone();
        // Added to the list after the preview: remove still refuses it.
        let file = h.join("Downloads/old.dmg");
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        assert_eq!(plan.items.len(), 1);
        ctx.exclusions.add(h.join("Downloads")).unwrap();
        for dry_run in [true, false] {
            let report = remove(&ctx, &plan, dry_run).unwrap();
            assert!(report.done.is_empty());
            assert_eq!(report.skipped[0].reason, Reason::Excluded);
        }
        assert!(skip_logged(&ctx, &file, "refused at the last moment"));
        assert!(file.exists());
        // The list survives a relaunch.
        let reopened = Context::new(h.clone(), ctx.data_dir.clone()).unwrap();
        assert_eq!(
            prepare(
                &reopened,
                &[req(file.clone(), Safety::YourCall)],
                Mode::Quarantine
            )
            .skipped[0]
                .reason,
            Reason::Excluded
        );
        // Delegated: emptying the Trash is refused while the Trash is on the list, before any
        // OS action runs (tests never reach the real one either; see `platform::run_delegated`).
        fs::create_dir_all(h.join(".Trash")).unwrap();
        fs::write(h.join(".Trash/kept.txt"), b"kept").unwrap();
        ctx.exclusions.add(h.join(".Trash")).unwrap();
        for dry_run in [true, false] {
            let err = delegate(&ctx, Delegated::EmptyTrash, dry_run).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        }
        assert!(h.join(".Trash/kept.txt").exists());
        // Off the list, dry-run reports the size.
        ctx.exclusions.remove(&h.join(".Trash")).unwrap();
        assert!(delegate(&ctx, Delegated::EmptyTrash, true).unwrap() > 0);
        assert_ne!(
            delegate(&ctx, Delegated::EmptyTrash, false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied,
            "past the list check, the test build stops before the OS"
        );
        crate::platform::remove_test_dir(&base);
    }

    /// A "your call" file another process has open is skipped, at preview and at the last moment.
    #[test]
    fn open_files_are_skipped() {
        let (base, ctx) = setup("openfile");
        let file = ctx.home.join("Downloads/old.dmg");
        let hold = || {
            std::process::Command::new("/bin/sleep")
                .arg("30")
                .stdin(fs::File::open(&file).unwrap())
                .spawn()
                .unwrap()
        };
        // Open before the preview.
        let mut child = hold();
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        assert_eq!(plan.skipped[0].reason, Reason::InUse);
        let _ = child.kill();
        let _ = child.wait();
        // Closed at preview, opened before the action.
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        assert_eq!(plan.items.len(), 1);
        let mut child = hold();
        let report = remove(&ctx, &plan, false).unwrap();
        let _ = child.kill();
        let _ = child.wait();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::InUse);
        assert!(file.exists());
        // Closed again: it goes.
        let plan = prepare(
            &ctx,
            &[req(file.clone(), Safety::YourCall)],
            Mode::Quarantine,
        );
        assert_eq!(remove(&ctx, &plan, false).unwrap().done.len(), 1);
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn symlinks_cant_escape_a_root_or_reach_protected_places() {
        let (base, ctx) = setup("links");
        let h = &ctx.home;
        // A cache root containing a link to the Keychains folder.
        std::os::unix::fs::symlink(
            h.join("Library/Keychains"),
            h.join("Library/Caches/app/escape"),
        )
        .unwrap();
        let mut through_link = req(
            h.join("Library/Caches/app/escape/login.keychain-db"),
            Safety::Safe,
        );
        through_link.root = Some(h.join("Library/Caches/app"));
        let plan = prepare(&ctx, &[through_link], Mode::DeleteNow);
        assert_eq!(plan.skipped[0].reason, Reason::Protected);
        // The link itself can go: it's removed as a link, the folder it points to stays.
        let mut link = req(h.join("Library/Caches/app/escape"), Safety::Safe);
        link.root = Some(h.join("Library/Caches/app"));
        let plan = prepare(&ctx, &[link], Mode::DeleteNow);
        remove(&ctx, &plan, false).unwrap();
        assert!(h.join("Library/Keychains/login.keychain-db").exists());
        // An item outside its rule's root is refused.
        let mut outside = req(h.join("Downloads/old.dmg"), Safety::Safe);
        outside.root = Some(h.join("Library/Caches"));
        assert_eq!(
            prepare(&ctx, &[outside], Mode::DeleteNow).skipped[0].reason,
            Reason::OutsideRoot
        );
        crate::platform::remove_test_dir(&base);
    }

    #[test]
    fn exclusions_are_enforced() {
        let (base, mut ctx) = setup("exclude");
        ctx.exclusions.add(ctx.home.join("Downloads")).unwrap();
        let plan = prepare(
            &ctx,
            &[req(ctx.home.join("Downloads/old.dmg"), Safety::YourCall)],
            Mode::Quarantine,
        );
        assert_eq!(plan.skipped[0].reason, Reason::Excluded);
        crate::platform::remove_test_dir(&base);
    }

    /// PLAN.md Phase 2 checkpoint: clean a fixture home with every bundled rule (all to quarantine),
    /// check protected and personal files are untouched, restore everything, compare byte for byte.
    #[test]
    fn fixture_home_clean_and_restore_is_byte_identical() {
        fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
            let mut out = Vec::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(p) = stack.pop() {
                for e in fs::read_dir(&p).unwrap().flatten() {
                    let md = fs::symlink_metadata(e.path()).unwrap();
                    if md.is_dir() {
                        stack.push(e.path());
                    } else {
                        out.push((
                            e.path().strip_prefix(root).unwrap().to_path_buf(),
                            fs::read(e.path()).unwrap(),
                        ));
                    }
                }
            }
            out.sort();
            out
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("wrasse-safety-e2e-{nanos}"));
        let home = base.join("home");
        fs::create_dir_all(&home).unwrap();
        wrasse_fixtures::mac_home(&home).unwrap();
        let ctx = Context::new(home, base.join("data")).unwrap();
        let h = ctx.home.clone();
        let before = snapshot(&h);

        let (rules, errors) =
            crate::rules::load(crate::rules::bundled_text(), false, &h, &ctx.protected);
        assert!(errors.is_empty(), "{errors:?}");
        let (found, _) = crate::rules::find(&rules, &[], &|p| crate::quarantine::size_on_disk(p));
        let requests: Vec<Request> = found
            .iter()
            .filter(|m| m.delegate.is_none())
            .map(|m| Request {
                path: m.path.clone(),
                safety: m.safety,
                rule_id: Some(m.rule_id.clone()),
                root: Some(m.root.clone()),
                hand_picked: false,
            })
            .collect();
        assert_eq!(requests.len(), 9, "{found:#?}");
        let plan = prepare(&ctx, &requests, Mode::Quarantine);
        assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
        let report = remove(&ctx, &plan, false).unwrap();
        assert_eq!(report.done.len(), 9);

        // Personal and protected files are all still there; the junk is gone.
        for keep in [
            "Documents/thesis.docx",
            "Desktop/todo.txt",
            "Library/Keychains/login.keychain-db",
            "Library/Mail/V10/mailbox",
            "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
            "Downloads/Recent.dmg",
            "Downloads/notes.txt",
            ".Trash/deleted-file.txt",
        ] {
            assert!(h.join(keep).exists(), "{keep} must not be touched");
        }
        assert!(!h.join("Downloads/OldInstaller.dmg").exists());
        assert!(!h.join("Library/Caches/com.spotify.client").exists());

        // After "relaunching", everything restores byte for byte.
        let reopened = Context::new(h.clone(), ctx.data_dir.clone()).unwrap();
        let ids: Vec<String> = report.done.iter().filter_map(|d| d.entry.clone()).collect();
        let back = restore(&reopened, &ids).unwrap();
        assert_eq!(back.len(), 9);
        assert_eq!(snapshot(&h), before);
        crate::platform::remove_test_dir(&base);
    }
}
