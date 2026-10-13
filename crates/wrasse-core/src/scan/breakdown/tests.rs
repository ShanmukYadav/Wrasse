use super::*;
use crate::platform::Volumes;
use crate::scan::{scan, ScanControl};
use std::fs;
use std::io::Write;

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = fs::File::create(path).unwrap();
    f.write_all(&vec![1u8; bytes]).unwrap();
}

/// A real safetensors header, then padding, so the file is both valid and large.
fn write_safetensors(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let header = br#"{"__metadata__":{"format":"pt"}}"#;
    let mut f = fs::File::create(path).unwrap();
    f.write_all(&(header.len() as u64).to_le_bytes()).unwrap();
    f.write_all(header).unwrap();
    let used = 8 + header.len();
    assert!(bytes > used);
    f.write_all(&vec![0u8; bytes - used]).unwrap();
}

#[test]
fn categories_add_up_and_each_byte_is_shown_once() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("wrasse-breakdown-{nanos}"));
    let home = root.join("Users/me");
    write(&home.join("Documents/report.pdf"), 30_000_000);
    write(&home.join(".ollama/models/blobs/sha256-1"), 40_000_000);
    write(
        &home.join("Library/Developer/Xcode/DerivedData/a.o"),
        20_000_000,
    );
    write(
        &home.join("Library/Caches/com.example.App/blob"),
        15_000_000,
    );
    write(&home.join("Library/Caches/other/blob"), 12_000_000);
    write(&home.join("Library/Caches/other/tiny-1"), 5_000);
    write(&home.join("Library/Caches/other/tiny-2"), 5_000);
    write(
        &root.join("Applications/Example.app/Contents/MacOS/Example"),
        25_000_000,
    );
    write(&root.join("Library/Fonts/big.ttc"), 11_000_000);

    let tree = scan(&root, &ScanControl::default()).unwrap();
    let prefix = root.to_string_lossy().into_owned();
    let scanned = tree.node(0).size;
    let apps = [crate::apps::App {
        id: Some("com.example.App".into()),
        name: "Example".into(),
        bundle: PathBuf::from("/Applications/Example.app"),
        version: None,
        folder: None,
        source: crate::apps::Source::Folder,
        owner: crate::apps::Owner::User,
        last_used_days: Some(12),
        added_days: Some(100),
        leftovers: vec![crate::apps::Leftover {
            path: PathBuf::from("/Users/me/Library/Caches/com.example.App"),
            kind: Leftover::Caches,
            matched_by: crate::apps::MatchedBy::BundleId,
            shared_with: vec![],
            needs_admin: false,
        }],
    }];
    let input = Input {
        tree: &tree,
        display_prefix: &prefix,
        home: Some(PathBuf::from("/Users/me")),
        volumes: Volumes {
            capacity: scanned * 4,
            available: scanned * 2,
            os_used: scanned / 2,
        },
        volume_name: "Test HD".into(),
        previous: None,
        running: vec![],
        today: 20_000,
        env: HashMap::new(),
        hf_models: &|_: &Path| Vec::new(),
        apps: &apps,
        orphans: &[],
    };
    let disk = build(&input);
    let n = |name: &str| {
        disk.nodes
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };

    // The root is everything in use: scanned + macOS + System data.
    assert_eq!(disk.nodes[0].size, scanned * 2);
    assert_eq!(n("System data").size, scanned * 2 - scanned / 2 - scanned);
    // Each category holds its folders, and nothing is counted twice.
    assert!(n("AI Models").size >= 40_000_000);
    assert!(n("Developer").size >= 20_000_000);
    let app = disk.nodes.iter().find(|n| n.kind == NodeKind::App).unwrap();
    assert_eq!(app.name, "Example");
    assert_eq!(app.last_opened_days, 12, "from Spotlight");
    // "Smaller items" carries its folder's path, so it must be marked as an aggregate (the UI
    // and the ranker never remove it as one item).
    let smaller: Vec<&UiNode> = disk
        .nodes
        .iter()
        .filter(|n| n.name == "Smaller items")
        .collect();
    assert!(!smaller.is_empty());
    for n in smaller {
        assert!(n.aggregate);
        assert_eq!(n.path, disk.nodes[n.parent as usize].path);
    }
    assert_eq!(
        app.app.as_ref().unwrap().route,
        crate::apps::uninstall::Route::Tray
    );
    let caches = disk
        .nodes
        .iter()
        .find(|n| n.leftover == Some(Leftover::Caches))
        .unwrap();
    assert_eq!(caches.parent, i64::from(app.id));
    assert!(n("Your Files").size >= 30_000_000 && n("Your Files").size < 31_000_000 + 1_000_000);
    let cats: u64 = disk.nodes[0]
        .children
        .iter()
        .map(|&c| disk.nodes[c as usize].size)
        .sum();
    assert_eq!(cats, disk.nodes[0].size);
    // Lens bytes: AI models and dev match their lens.
    assert!(disk.nodes[0].match_bytes.ai_models >= 40_000_000);
    assert!(disk.nodes[0].match_bytes.dev >= 20_000_000);
    assert!(disk.nodes[0].match_bytes.junk >= 12_000_000);
    crate::platform::remove_test_dir(&root);
}

/// Stale project folders move into Developer. The Wrasse checkout does not.
/// A Python env in Documents is shown and is not pre-selected.
#[test]
fn developer_junk_is_claimed_and_the_wrasse_checkout_is_not() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("wrasse-devjunk-{nanos}"));
    let home = root.join("Users/me");
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    let times = fs::FileTimes::new().set_accessed(old).set_modified(old);
    let age = |path: &Path| {
        fn rec(p: &Path, times: fs::FileTimes) {
            let meta = fs::symlink_metadata(p).unwrap();
            if meta.is_dir() {
                for entry in fs::read_dir(p).unwrap() {
                    rec(&entry.unwrap().path(), times);
                }
            }
            fs::File::options()
                .read(true)
                .open(p)
                .unwrap()
                .set_times(times)
                .unwrap();
        }
        rec(path, times);
    };

    write(
        &home.join("Projects/web/node_modules/pkg/index.js"),
        12_000_000,
    );
    write(&home.join("Projects/web/package.json"), 20);
    age(&home.join("Projects/web/node_modules"));
    write(
        &home.join("Projects/fresh/node_modules/pkg/index.js"),
        12_000_000,
    );
    write(&home.join("Documents/Wrasse/.toolchain/rust/README"), 20);
    write(
        &home.join("Documents/Wrasse/target/debug/wrasse"),
        12_000_000,
    );
    write(&home.join("Documents/Wrasse/Cargo.toml"), 20);
    age(&home.join("Documents/Wrasse"));
    write(&home.join("Documents/app/.venv/pyvenv.cfg"), 30);
    write(&home.join("Documents/app/.venv/lib/python"), 12_000_000);
    age(&home.join("Documents/app/.venv"));
    write(&home.join("code/api/Cargo.toml"), 20);
    write(&home.join("code/api/target/debug/api"), 12_000_000);
    age(&home.join("code/api/target"));
    write(
        &home.join("Library/Containers/com.docker.docker/Data/vms/disk"),
        12_000_000,
    );

    let tree = scan(&root, &ScanControl::default()).unwrap();
    let prefix = root.to_string_lossy().into_owned();
    let scanned = tree.node(0).size;
    let input = Input {
        tree: &tree,
        display_prefix: &prefix,
        home: Some(PathBuf::from("/Users/me")),
        volumes: Volumes {
            capacity: scanned * 4,
            available: scanned,
            os_used: 0,
        },
        volume_name: "Test HD".into(),
        previous: None,
        running: vec![],
        today: 20_000,
        env: HashMap::new(),
        apps: &[],
        orphans: &[],
        hf_models: &|_: &Path| Vec::new(),
    };
    let disk = build(&input);
    let find = |name: &str| disk.nodes.iter().find(|n| n.name == name && n.size > 0);
    let modules = find("web · node_modules").expect("stale node_modules");
    assert_eq!(modules.category, Category::Developer);
    assert_eq!(modules.dev_kind.as_deref(), Some("modules"));
    assert!(modules.rebuildable);
    assert_eq!(modules.safety, Safety::Safe);
    assert!(find("fresh · node_modules").is_none());
    let fresh = disk
        .nodes
        .iter()
        .find(|n| n.path.ends_with("/Projects/fresh/node_modules"))
        .expect("recent node_modules stays in the project");
    assert_ne!(fresh.category, Category::Developer);

    let wrasse = disk
        .nodes
        .iter()
        .find(|n| n.path.contains("/Documents/Wrasse/target"))
        .expect("wrasse target stays visible");
    assert_ne!(wrasse.category, Category::Developer);
    assert!(wrasse.read_only && wrasse.safety == Safety::Keep);

    let venv = find("app · Python").expect("documents venv");
    assert_eq!(venv.dev_kind.as_deref(), Some("venv"));
    assert_eq!(venv.safety, Safety::YourCall);
    assert!(!venv.rebuildable);

    let target = find("api · target").expect("cargo target");
    assert!(target.rebuildable && target.safety == Safety::Safe);
    assert_eq!(target.dev_kind.as_deref(), Some("target"));

    let docker = disk
        .nodes
        .iter()
        .find(|n| n.path.contains("com.docker.docker") && n.name == "Docker")
        .expect("docker");
    assert!(docker.read_only && docker.safety == Safety::Keep);

    let limits = crate::rank::Limits {
        hand_pick_only: vec![PathBuf::from("/Users/me/Documents")],
        ..Default::default()
    };
    let (offered, _, _) = crate::rank::candidates(&disk, &limits);
    let offered_paths: Vec<&str> = offered
        .iter()
        .map(|c| disk.nodes[c.id as usize].path.as_str())
        .collect();
    assert!(offered_paths
        .iter()
        .any(|p| p.ends_with("/Projects/web/node_modules")));
    assert!(offered_paths
        .iter()
        .any(|p| p.ends_with("/code/api/target")));
    assert!(offered_paths.iter().all(|p| !p.contains("/Documents/")));
    assert!(offered_paths.iter().all(|p| !p.contains("/Wrasse")));
    assert!(offered_paths
        .iter()
        .all(|p| !p.contains("com.docker.docker")));
    crate::platform::remove_test_dir(&root);
}

#[test]
fn models_are_found_anywhere_and_grouped_by_project() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("wrasse-models-{nanos}"));
    let proj = root.join("Users/me/Documents/vyakta");
    write(&proj.join("env/hf_cache/hub/blobs/w1"), 60_000_000);
    let snap = proj.join("env/hf_cache/hub/models--mlx-community--Mistral-7B/snapshots/rev");
    fs::create_dir_all(&snap).unwrap();
    std::os::unix::fs::symlink("../../../blobs/w1", snap.join("model.safetensors")).unwrap();
    write_safetensors(
        &proj.join("models/checkpoint-1/adapter_model.safetensors"),
        55_000_000,
    );
    // Named like a weight file and large. A config without model keys does not make it one.
    write(&proj.join("notes/pytorch_model.bin"), 50_000_000);
    fs::write(proj.join("notes/config.json"), br#"{"name":"not-a-model"}"#).unwrap();
    write(&proj.join("notes/readme.bin"), 2_000_000);
    write(&root.join("Users/me/Documents/report.pdf"), 30_000_000);

    let tree = scan(&root, &ScanControl::default()).unwrap();
    let prefix = root.to_string_lossy().into_owned();
    let scanned = tree.node(0).size;
    let input = Input {
        tree: &tree,
        display_prefix: &prefix,
        home: Some(PathBuf::from("/Users/me")),
        volumes: Volumes {
            capacity: scanned * 4,
            available: scanned * 2,
            os_used: 0,
        },
        volume_name: "Test HD".into(),
        previous: None,
        running: vec![],
        today: 20_000,
        env: HashMap::new(),
        apps: &[],
        orphans: &[],
        hf_models: &crate::scan::hf::hf_models,
    };
    let disk = build(&input);
    let n = |name: &str| {
        disk.nodes
            .iter()
            .find(|n| n.name == name && n.size > 0)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    let ai = n("AI Models");
    // Your Files also has a "vyakta" folder now (the zero-filled bin lives there).
    let group = disk
        .nodes
        .iter()
        .find(|n| n.name == "Project models" && n.category == Category::AiModels && n.size > 0)
        .expect("project model group");
    assert_eq!(group.parent, i64::from(ai.id));
    assert_eq!(group.safety, Safety::YourCall);
    let mistral = n("Mistral-7B");
    assert_eq!(mistral.kind, NodeKind::Model);
    assert_eq!(mistral.safety, Safety::YourCall);
    assert!(!mistral.rebuildable);
    assert!(
        mistral.why.contains("part of this project"),
        "a project cache is project data: {}",
        mistral.why
    );
    assert!(!mistral.why.contains("Downloads again"));
    assert!(disk
        .nodes
        .iter()
        .filter(|n| n.path.contains("hf_cache"))
        .all(|n| { n.category != Category::Junk && n.safety != Safety::Safe && !n.rebuildable }));
    assert!(
        mistral.approx,
        "a model dated from file dates is approximate"
    );
    assert!(!ai.approx, "a category folder is not an approximate date");
    assert!(mistral.size >= 60_000_000);
    let ckpt = n("checkpoint-1 · adapter_model.safetensors");
    assert_eq!(ckpt.kind, NodeKind::Model);
    // The zero-filled .bin stays a file under Your Files. It is not an AI model.
    let stray = disk
        .nodes
        .iter()
        .find(|n| n.name.contains("pytorch_model"))
        .expect("zero-filled bin stays in the tree");
    assert_ne!(stray.kind, NodeKind::Model);
    assert_ne!(stray.category, Category::AiModels);
    assert!(stray.approx, "the file's age still comes from file dates");
    // Claimed bytes leave Your Files, and nothing is counted twice.
    assert!(n("Your Files").size >= 50_000_000 + 30_000_000);
    assert!(n("Your Files").size < 50_000_000 + 30_000_000 + 15_000_000);
    let cats: u64 = disk.nodes[0]
        .children
        .iter()
        .map(|&c| disk.nodes[c as usize].size)
        .sum();
    assert_eq!(cats, disk.nodes[0].size);
    assert!(disk.nodes[0].match_bytes.ai_models >= 115_000_000);
    assert!(disk.nodes[0].match_bytes.ai_models < 200_000_000);
    crate::platform::remove_test_dir(&root);
}

#[test]
fn project_copies_in_icloud_are_duplicates_and_two_formats_are_labeled() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("wrasse-project-models-{nanos}"));
    let home = root.join("Users/me");
    let local = home.join("Documents/vyakta/models/fpc/model.safetensors");
    let cloud = home
        .join("Library/Mobile Documents/com~apple~CloudDocs/Vyakta_Backup/fpc/model.safetensors");
    write_safetensors(&local, 50_000_000);
    write_safetensors(&cloud, 50_000_000);
    let gector = home.join("Documents/vyakta/models/gector");
    write_safetensors(&gector.join("model.safetensors"), 50_000_000);
    write_pytorch(&gector.join("pytorch_model.bin"), 51_000_000);
    fs::write(gector.join("config.json"), br#"{"model_type":"deberta"}"#).unwrap();

    let tree = scan(&root, &ScanControl::default()).unwrap();
    let prefix = root.to_string_lossy().into_owned();
    let scanned = tree.node(0).size;
    let input = Input {
        tree: &tree,
        display_prefix: &prefix,
        home: Some(PathBuf::from("/Users/me")),
        volumes: Volumes {
            capacity: scanned * 4,
            available: scanned * 2,
            os_used: 0,
        },
        volume_name: "Test HD".into(),
        previous: None,
        running: vec![],
        today: 20_000,
        env: HashMap::new(),
        apps: &[],
        orphans: &[],
        hf_models: &|_: &Path| Vec::new(),
    };
    let disk = build(&input);
    let models: Vec<&UiNode> = disk
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Model && n.category == Category::AiModels)
        .collect();
    assert!(
        models.len() >= 4,
        "local, icloud, and both formats: {models:?}"
    );
    assert!(models.iter().all(|n| n.safety == Safety::YourCall));
    let cloud_node = models
        .iter()
        .find(|n| n.path.contains("Mobile Documents"))
        .expect("icloud copy");
    assert!(
        cloud_node
            .note
            .as_deref()
            .is_some_and(|s| s.contains(crate::llm::ICLOUD_DUPE_NOTE)),
        "icloud note: {:?}",
        cloud_node.note
    );
    assert!(
        cloud_node.model.as_ref().unwrap().dupe_level.is_some()
            || cloud_node.duplicate_of.is_some()
    );
    let local_node = models
        .iter()
        .find(|n| n.path.contains("/Documents/") && n.path.ends_with("fpc/model.safetensors"))
        .expect("local copy");
    assert!(
        local_node
            .note
            .as_deref()
            .map(|s| !s.contains("iCloud"))
            .unwrap_or(true),
        "the Documents copy is not the synced warning: {:?}",
        local_node.note
    );
    let formats: Vec<&&UiNode> = models
        .iter()
        .filter(|n| n.path.contains("/gector/"))
        .collect();
    assert_eq!(formats.len(), 2, "{formats:?}");
    assert!(formats.iter().all(|n| {
        n.note
            .as_deref()
            .is_some_and(|s| s.contains(crate::llm::FORMAT_NOTE))
            && n.duplicate_of.is_none()
    }));
    let limits = crate::rank::Limits {
        hand_pick_only: vec![
            PathBuf::from("/Users/me/Documents"),
            PathBuf::from("/Users/me/Desktop"),
            PathBuf::from("/Users/me/Library/Mobile Documents"),
        ],
        ..Default::default()
    };
    let (offered, _, _) = crate::rank::candidates(&disk, &limits);
    let offered_ids: Vec<u32> = offered.iter().map(|c| c.id).collect();
    for n in models {
        assert!(!offered_ids.contains(&n.id), "{} was pre-selected", n.path);
    }
    crate::platform::remove_test_dir(&root);
}

fn write_pytorch(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let magic = b"PK\x03\x04data.pkl";
    let mut f = fs::File::create(path).unwrap();
    f.write_all(magic).unwrap();
    assert!(bytes > magic.len());
    f.write_all(&vec![0u8; bytes - magic.len()]).unwrap();
}

fn write_gguf(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = fs::File::create(path).unwrap();
    f.write_all(b"GGUF").unwrap();
    f.write_all(&vec![0u8; bytes - 4]).unwrap();
}

#[test]
fn each_tool_lists_its_models_and_shared_weights_count_once() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("wrasse-tools-{nanos}"));
    let home = root.join("Users/me");
    let models = home.join(".ollama/models");
    let shared = "a".repeat(64);
    let own = "b".repeat(64);
    write(&models.join(format!("blobs/sha256-{shared}")), 10_000_000);
    write(&models.join(format!("blobs/sha256-{own}")), 10_000_000);
    write(&models.join("blobs/sha256-orphan"), 10_000_000);
    let big = format!(
        r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{shared}","size":1}},{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{own}","size":2}}]}}"#
    );
    let tiny = format!(
        r#"{{"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{shared}","size":1}}]}}"#
    );
    let manifest = models.join("manifests/registry.ollama.ai/library/llama-Q4_K_M/latest");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, big).unwrap();
    let other = models.join("manifests/registry.ollama.ai/library/tiny/latest");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, tiny).unwrap();
    write_gguf(
        &home.join("Library/Application Support/Jan/models/coder-Q5_K_M.gguf"),
        10_000_000,
    );
    write_gguf(
        &home.join("Library/Application Support/nomic.ai/GPT4All/gpt-Q8_0.gguf"),
        10_000_000,
    );
    let lms = home.join("Models");
    write_gguf(&lms.join("publisher/story-Q4_K_M.gguf"), 10_000_000);
    let settings = home.join("Library/Application Support/LM Studio/settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(
        &settings,
        format!(r#"{{"downloadsFolder":"{}"}}"#, lms.display()),
    )
    .unwrap();
    let hub = home.join(".cache/huggingface/hub");
    write(&hub.join("blobs/w"), 10_000_000);
    let repo = hub.join("models--org--plain");
    fs::create_dir_all(repo.join("snapshots/rev")).unwrap();
    std::os::unix::fs::symlink(
        "../../../blobs/w",
        repo.join("snapshots/rev/model.safetensors"),
    )
    .unwrap();
    fs::write(
        repo.join("snapshots/rev/config.json"),
        r#"{"quantization_config":{"bits":4,"quant_method":"bitsandbytes"}}"#,
    )
    .unwrap();

    let tree = scan(&root, &ScanControl::default()).unwrap();
    let prefix = root.to_string_lossy().into_owned();
    let scanned = tree.node(0).size;
    let input = Input {
        tree: &tree,
        display_prefix: &prefix,
        home: Some(PathBuf::from("/Users/me")),
        volumes: Volumes {
            capacity: scanned * 4,
            available: scanned,
            os_used: 0,
        },
        volume_name: "Test HD".into(),
        previous: None,
        running: vec![],
        today: 20_000,
        env: HashMap::new(),
        apps: &[],
        orphans: &[],
        hf_models: &crate::scan::hf::hf_models,
    };
    let disk = build(&input);
    let find = |name: &str| {
        disk.nodes
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    let llama = find("llama-Q4_K_M:latest");
    assert_eq!(llama.kind, NodeKind::Model);
    assert_eq!(
        llama.model.as_ref().unwrap().quantization.as_deref(),
        Some("Q4_K_M")
    );
    assert!(llama.size >= 20_000_000);
    assert!(!llama.model.as_ref().unwrap().shares_weights);
    let tiny = find("tiny:latest");
    assert!(tiny.model.as_ref().unwrap().shares_weights);
    assert_eq!(
        tiny.model.as_ref().unwrap().shares_with.as_deref(),
        Some("llama-Q4_K_M:latest")
    );
    let orphan = disk
        .nodes
        .iter()
        .find(|n| n.model.as_ref().is_some_and(|m| m.orphan_blob))
        .unwrap();
    assert!(orphan.size >= 10_000_000);
    let jan = find("models · coder-Q5_K_M.gguf");
    assert_eq!(jan.model.as_ref().unwrap().tool, crate::llm::ModelTool::Jan);
    assert_eq!(
        jan.model.as_ref().unwrap().quantization.as_deref(),
        Some("Q5_K_M")
    );
    let gpt = find("gpt-Q8_0.gguf");
    assert_eq!(
        gpt.model.as_ref().unwrap().tool,
        crate::llm::ModelTool::Gpt4all
    );
    let story = find("publisher · story-Q4_K_M.gguf");
    assert_eq!(
        story.model.as_ref().unwrap().tool,
        crate::llm::ModelTool::LmStudio
    );
    let hf = find("plain");
    assert_eq!(
        hf.model.as_ref().unwrap().quantization.as_deref(),
        Some("4-bit")
    );
    assert_eq!(
        hf.model.as_ref().unwrap().tool,
        crate::llm::ModelTool::HuggingFace
    );
    let ai = find("AI Models").size;
    assert!(ai >= 70_000_000, "{ai}");
    assert!(ai < 85_000_000, "shared blob counted once: {ai}");
    crate::platform::remove_test_dir(&root);
}
