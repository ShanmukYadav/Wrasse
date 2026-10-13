//! Delegated OS actions: the OS or a tool deletes on Wrasse's behalf, from a fixed allowlist.

use super::*;

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
    /// `ollama rm`: POST `/api/delete` on 127.0.0.1:11434. Not undoable.
    OllamaRm { model: String },
    /// `ollama stop`: POST `/api/generate` with `keep_alive` 0. The model stays on disk.
    OllamaStop { model: String },
    /// `tmutil thinlocalsnapshots / <bytes> 4`. Not undoable. `bytes` is measured again
    /// on the Mac before this runs; 0 is refused.
    ThinLocalSnapshots { bytes: u64 },
    /// `brew cleanup -s`. Homebrew itself stays. Not undoable.
    BrewCleanup,
    /// `docker system prune -f` (no `--volumes`), only while Docker is running.
    DockerPrune,
    /// `xcrun simctl delete unavailable`. Not undoable.
    SimctlDeleteUnavailable,
}

impl Delegated {
    /// The UI's name for this action. Stable across versions.
    pub(crate) fn action_name(&self) -> &'static str {
        match self {
            Self::EmptyTrash => "emptyTrash",
            Self::BrewUninstallCask { .. } => "brewUninstall",
            Self::FinderMoveToTrash { .. } => "finderTrash",
            Self::OllamaRm { .. } => "ollamaRm",
            Self::OllamaStop { .. } => "ollamaStop",
            Self::ThinLocalSnapshots { .. } => "thinSnapshots",
            Self::BrewCleanup => "brewCleanup",
            Self::DockerPrune => "dockerPrune",
            Self::SimctlDeleteUnavailable => "deleteUnavailableSimulators",
        }
    }
}

/// Why a delegated action was refused before running.
pub(super) fn refuse(why: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, why.into())
}

/// Checks an app bundle a delegated action would remove: an `.app` directly in (or one folder
/// inside) an Applications folder, on the startup disk, not protected, not on the never-touch
/// list, not part of the OS, and not running. Returns its size.
pub(super) fn check_app_target(ctx: &Context, app: &Path) -> io::Result<u64> {
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
pub(super) fn brew_binary(ctx: &Context) -> Option<PathBuf> {
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
pub(crate) fn cask_matches(
    token: &str,
    app: &Path,
    cask_of: &dyn Fn(&Path) -> Option<String>,
) -> bool {
    platform::valid_cask_token(token) && cask_of(app).as_deref() == Some(token)
}

pub fn delegate(ctx: &Context, action: Delegated, dry_run: bool) -> io::Result<u64> {
    delegate_with(ctx, action, dry_run, &platform::cask_of)
}

/// [`delegate`] with the cask lookup injected (tests use a fixture Caskroom).
pub(crate) fn delegate_with(
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
        Delegated::OllamaRm { model } => {
            if !platform::valid_ollama_name(model) {
                Err(refuse("that isn't a model name"))
            } else {
                Ok((format!("ollama rm {model}"), 0, None))
            }
        }
        Delegated::OllamaStop { model } => {
            if !platform::valid_ollama_name(model) {
                Err(refuse("that isn't a model name"))
            } else {
                Ok((format!("ollama stop {model}"), 0, None))
            }
        }
        Delegated::ThinLocalSnapshots { bytes } => {
            if *bytes == 0 {
                Err(refuse("no space held by snapshots"))
            } else {
                Ok((
                    format!("tmutil thinlocalsnapshots / {bytes} 4"),
                    *bytes,
                    None,
                ))
            }
        }
        Delegated::BrewCleanup => {
            let brew = brew_binary(ctx);
            if brew.is_none() && !dry_run {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Homebrew isn't installed",
                ))
            } else {
                Ok(("brew cleanup -s".to_string(), 0, brew))
            }
        }
        Delegated::DockerPrune => {
            if !dry_run && platform::docker_bin().is_none() {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Docker isn't running",
                ))
            } else {
                Ok(("docker system prune -f".to_string(), 0, None))
            }
        }
        Delegated::SimctlDeleteUnavailable => {
            Ok(("xcrun simctl delete unavailable".to_string(), 0, None))
        }
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
