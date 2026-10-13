//! Removes one model. A loaded model is refused. Ollama goes through `ollama rm` when the
//! local server answers; otherwise only the manifest and blobs no other manifest uses are
//! quarantined. Everything else goes through `safety::remove`.

use super::ollama::{self, valid_name};
use super::{blob_content_hash, full_hash, DupeLevel, ModelRemoval, ModelTool};
use crate::safety::{self, Context, Delegated, Mode, Reason, Report, Request, Safety, Skipped};
use std::io;
use std::path::{Path, PathBuf};

/// What the UI asks to remove. Paths are the ones the scan showed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelJob {
    pub path: PathBuf,
    pub tool: ModelTool,
    pub removal: ModelRemoval,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ollama_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner_weight: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dupe_level: Option<DupeLevel>,
    pub bytes: u64,
    pub safety: Safety,
    #[serde(default)]
    pub hand_picked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clone_copy: bool,
}

/// Whether the Ollama server is up, and whether a named model is loaded.
pub trait Probe {
    fn reachable(&self) -> bool;
    fn loaded(&self, name: &str) -> bool;
    /// LM Studio publishes no per-model list. Process memory is the signal, and it is approximate.
    fn lm_studio_busy(&self) -> bool {
        false
    }
}

/// The probe production uses: 127.0.0.1:11434, then the LM Studio process.
pub struct Live;

impl Probe for Live {
    fn reachable(&self) -> bool {
        crate::platform::ollama_reachable()
    }

    fn loaded(&self, name: &str) -> bool {
        if !self.reachable() {
            return false;
        }
        // A missing or unreadable process list refuses the removal.
        match crate::platform::ollama_get("/api/ps") {
            Ok(body) => super::ps_contains(&body, name),
            Err(_) => true,
        }
    }

    fn lm_studio_busy(&self) -> bool {
        crate::platform::process_memory(&["lm studio", "lmstudio", "lm-studio"]) > 0
    }
}

pub fn remove_model(ctx: &Context, job: &ModelJob, dry_run: bool) -> io::Result<Report> {
    remove_model_with(ctx, job, dry_run, &Live)
}

pub fn remove_model_with(
    ctx: &Context,
    job: &ModelJob,
    dry_run: bool,
    probe: &dyn Probe,
) -> io::Result<Report> {
    if let Some(reason) = confirm_duplicate(job) {
        return Ok(skip(job, reason, dry_run));
    }
    if model_loaded(job, probe) {
        return Ok(skip(job, Reason::InUse, dry_run));
    }
    match job.removal {
        ModelRemoval::Ollama => remove_ollama(ctx, job, dry_run, probe),
        ModelRemoval::HuggingFace => {
            if !hf_repo(&job.path) {
                return Ok(skip(job, Reason::Failed, dry_run));
            }
            quarantine_paths(ctx, job, dry_run, std::slice::from_ref(&job.path))
        }
        ModelRemoval::Quarantine => {
            quarantine_paths(ctx, job, dry_run, std::slice::from_ref(&job.path))
        }
    }
}

/// Asks Ollama to unload a model (`keep_alive: 0`). Does not delete it.
pub fn unload_model(ctx: &Context, name: &str, dry_run: bool) -> io::Result<()> {
    if !valid_name(name) {
        return Err(io::Error::other("that isn't a model name"));
    }
    safety::delegate(
        ctx,
        Delegated::OllamaStop {
            model: name.to_string(),
        },
        dry_run,
    )
    .map(|_| ())
}

fn model_loaded(job: &ModelJob, probe: &dyn Probe) -> bool {
    if job.tool == ModelTool::LmStudio && probe.lm_studio_busy() {
        return true;
    }
    job.removal == ModelRemoval::Ollama
        && job
            .ollama_name
            .as_deref()
            .is_some_and(|name| probe.loaded(name))
}

/// `Ok(())` is not used: `None` means the duplicate check passed (or there is no duplicate).
fn confirm_duplicate(job: &ModelJob) -> Option<Reason> {
    let level = job.dupe_level?;
    let Some(weight) = job.weight.as_deref() else {
        return Some(Reason::NotConfirmed);
    };
    let Some(partner) = job.partner_weight.as_deref() else {
        return Some(Reason::NotConfirmed);
    };
    if !weight.is_file() || !partner.is_file() {
        return Some(Reason::NotConfirmed);
    }
    if level == DupeLevel::Full {
        if let (Some(a), Some(b)) = (
            blob_content_hash(Path::new(weight)),
            blob_content_hash(Path::new(partner)),
        ) {
            if a == b {
                return None;
            }
        }
    }
    match (full_hash(Path::new(weight)), full_hash(Path::new(partner))) {
        (Ok(a), Ok(b)) if a == b => None,
        (Err(e), _) | (_, Err(e)) if e.to_string().contains("iCloud") => Some(Reason::InCloudOnly),
        _ => Some(Reason::NotConfirmed),
    }
}

fn remove_ollama(
    ctx: &Context,
    job: &ModelJob,
    dry_run: bool,
    probe: &dyn Probe,
) -> io::Result<Report> {
    let Some(name) = job.ollama_name.clone() else {
        return Ok(skip(job, Reason::Failed, dry_run));
    };
    if !valid_name(&name) {
        return Ok(skip(job, Reason::Failed, dry_run));
    }
    if probe.reachable() {
        // The server deletes the model. A failure must not fall through to quarantine.
        safety::delegate(ctx, Delegated::OllamaRm { model: name }, dry_run)?;
        return Ok(Report {
            done: vec![safety::Done {
                path: job.path.clone(),
                bytes: job.bytes,
                entry: None,
            }],
            bytes: job.bytes,
            dry_run,
            ..Report::default()
        });
    }
    let Some(dir) = job.models_dir.clone() else {
        return Ok(skip(job, Reason::Failed, dry_run));
    };
    let paths = ollama::exclusive_paths(&dir, &name);
    if paths.is_empty() {
        return Ok(skip(job, Reason::NotFound, dry_run));
    }
    quarantine_paths(ctx, job, dry_run, &paths)
}

fn hf_repo(path: &Path) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if md.file_type().is_symlink() || !md.is_dir() {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !name.starts_with("models--") {
        return false;
    }
    let parent_hub = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some("hub");
    parent_hub || path.join("snapshots").is_dir()
}

fn quarantine_paths(
    ctx: &Context,
    job: &ModelJob,
    dry_run: bool,
    paths: &[PathBuf],
) -> io::Result<Report> {
    let reqs: Vec<Request> = paths
        .iter()
        .map(|path| Request {
            path: path.clone(),
            safety: job.safety,
            rule_id: None,
            root: None,
            hand_picked: job.hand_picked,
            scanned: None,
        })
        .collect();
    let plan = safety::prepare(ctx, &reqs, Mode::Quarantine);
    let mut report = safety::remove(ctx, &plan, dry_run)?;
    let moved = !report.done.is_empty();
    let bytes = report.done.iter().map(|d| d.bytes).sum();
    let entry = report.done.iter().find_map(|d| d.entry.clone());
    // The UI maps the node by the path it showed. One done row covers the manifest and its blobs.
    if moved {
        report.done = vec![safety::Done {
            path: job.path.clone(),
            bytes,
            entry,
        }];
        report.bytes = bytes;
    } else if let Some(skip) = report.skipped.first_mut() {
        skip.path = job.path.clone();
    }
    report.dry_run = dry_run;
    Ok(report)
}

fn skip(job: &ModelJob, reason: Reason, dry_run: bool) -> Report {
    Report {
        skipped: vec![Skipped {
            path: job.path.clone(),
            reason,
        }],
        dry_run,
        ..Report::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::Context;
    use std::fs;

    struct Scripted {
        reachable: bool,
        loaded: bool,
    }

    impl Probe for Scripted {
        fn reachable(&self) -> bool {
            self.reachable
        }
        fn loaded(&self, _: &str) -> bool {
            self.loaded
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-rm-{tag}-{nanos}"));
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::create_dir_all(dir.join("data")).unwrap();
        dir
    }

    fn ctx(dir: &Path) -> Context {
        Context::new(dir.join("home"), dir.join("data")).unwrap()
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn job(path: &Path, removal: ModelRemoval) -> ModelJob {
        ModelJob {
            path: path.to_path_buf(),
            tool: ModelTool::Loose,
            removal,
            ollama_name: None,
            models_dir: None,
            weight: Some(path.to_path_buf()),
            partner_weight: None,
            dupe_level: None,
            bytes: 4,
            safety: Safety::YourCall,
            hand_picked: true,
            clone_copy: false,
        }
    }

    #[test]
    fn a_sampled_mismatch_is_not_removed() {
        let dir = scratch("dupe");
        let a = dir.join("home/a.bin");
        let c = dir.join("home/c.bin");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        for path in [&a, &c] {
            let f = fs::File::create(path).unwrap();
            f.set_len(33 * 1024 * 1024).unwrap();
        }
        let mut fc = fs::OpenOptions::new().write(true).open(&c).unwrap();
        use std::io::{Seek, SeekFrom, Write};
        fc.seek(SeekFrom::Start(16 * 1024 * 1024 + 100)).unwrap();
        fc.write_all(&[7u8]).unwrap();
        let mut item = job(&c, ModelRemoval::Quarantine);
        item.weight = Some(c.clone());
        item.partner_weight = Some(a.clone());
        item.dupe_level = Some(DupeLevel::Sampled);
        item.bytes = 33 * 1024 * 1024;
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: false,
            loaded: false,
        };
        let report = remove_model_with(&context, &item, false, &probe).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::NotConfirmed);
        assert!(c.is_file() && a.is_file());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn a_full_match_of_a_sampled_pair_can_be_quarantined() {
        let dir = scratch("same");
        let a = dir.join("home/a.bin");
        let b = dir.join("home/b.bin");
        write_file(&a, b"same-weights");
        write_file(&b, b"same-weights");
        let mut item = job(&b, ModelRemoval::Quarantine);
        item.partner_weight = Some(a.clone());
        item.dupe_level = Some(DupeLevel::Sampled);
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: false,
            loaded: false,
        };
        let report = remove_model_with(&context, &item, false, &probe).unwrap();
        assert_eq!(report.done.len(), 1);
        assert_eq!(report.done[0].path, b);
        assert!(!b.exists());
        assert!(a.is_file());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn server_down_quarantines_the_manifest_and_leaves_the_shared_blob() {
        let dir = scratch("off");
        let models = dir.join("home/models");
        let hex_shared = "a".repeat(64);
        let hex_own = "c".repeat(64);
        write_file(
            &models.join(format!("blobs/sha256-{hex_shared}")),
            b"shared-blob",
        );
        write_file(
            &models.join(format!("blobs/sha256-{hex_own}")),
            b"owned-blob",
        );
        let body = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_shared}","size":1}},{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_own}","size":2}}]}}"#
        );
        let manifest = models.join("manifests/registry.ollama.ai/library/wrasse-fixture/test");
        write_file(&manifest, body.as_bytes());
        // A second manifest keeps the shared blob.
        let other = format!(
            r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{hex_shared}","size":9}}]}}"#
        );
        write_file(
            &models.join("manifests/registry.ollama.ai/library/other/latest"),
            other.as_bytes(),
        );
        let mut item = job(&manifest, ModelRemoval::Ollama);
        item.tool = ModelTool::Ollama;
        item.ollama_name = Some("wrasse-fixture:test".into());
        item.models_dir = Some(models.clone());
        item.weight = Some(models.join(format!("blobs/sha256-{hex_own}")));
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: false,
            loaded: false,
        };
        let report = remove_model_with(&context, &item, false, &probe).unwrap();
        assert_eq!(report.done.len(), 1);
        assert_eq!(report.done[0].path, manifest);
        assert!(!manifest.exists());
        assert!(!models.join(format!("blobs/sha256-{hex_own}")).exists());
        assert!(models.join(format!("blobs/sha256-{hex_shared}")).is_file());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn a_loaded_model_is_left_in_place() {
        let dir = scratch("loaded");
        let manifest =
            dir.join("home/models/manifests/registry.ollama.ai/library/wrasse-fixture/test");
        write_file(&manifest, b"{}");
        let mut item = job(&manifest, ModelRemoval::Ollama);
        item.tool = ModelTool::Ollama;
        item.ollama_name = Some("wrasse-fixture:test".into());
        item.models_dir = Some(dir.join("home/models"));
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: true,
            loaded: true,
        };
        let report = remove_model_with(&context, &item, false, &probe).unwrap();
        assert!(report.done.is_empty());
        assert_eq!(report.skipped[0].reason, Reason::InUse);
        assert!(manifest.is_file());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn a_reachable_server_does_not_fall_through_to_quarantine() {
        let dir = scratch("up");
        let manifest =
            dir.join("home/models/manifests/registry.ollama.ai/library/wrasse-fixture/test");
        let blob = dir.join("home/models/blobs/sha256-1");
        write_file(&manifest, b"{}");
        write_file(&blob, b"blob");
        let mut item = job(&manifest, ModelRemoval::Ollama);
        item.tool = ModelTool::Ollama;
        item.ollama_name = Some("wrasse-fixture:test".into());
        item.models_dir = Some(dir.join("home/models"));
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: true,
            loaded: false,
        };
        let err = remove_model_with(&context, &item, false, &probe).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("delegated actions don't run in tests")
                || msg.contains("isn't running")
                || msg.contains("loaded"),
            "{msg}"
        );
        assert!(manifest.is_file());
        assert!(blob.is_file());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn a_hugging_face_repo_is_quarantined() {
        let dir = scratch("hf");
        let repo = dir.join("home/hub/models--org--name");
        write_file(&repo.join("snapshots/rev/config.json"), b"{}");
        let item = ModelJob {
            tool: ModelTool::HuggingFace,
            removal: ModelRemoval::HuggingFace,
            ..job(&repo, ModelRemoval::HuggingFace)
        };
        let context = ctx(&dir);
        let probe = Scripted {
            reachable: false,
            loaded: false,
        };
        let report = remove_model_with(&context, &item, false, &probe).unwrap();
        assert_eq!(report.done.len(), 1);
        assert!(!repo.exists());
        crate::platform::remove_test_dir(&dir);
    }

    #[test]
    fn dry_run_unload_does_not_call_the_server() {
        let dir = scratch("stop");
        let context = ctx(&dir);
        assert!(unload_model(&context, "wrasse-fixture:test", true).is_ok());
        crate::platform::remove_test_dir(&dir);
    }
}
