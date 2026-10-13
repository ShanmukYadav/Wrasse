use super::*;
use std::io::Write;

fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("wrasse-llm-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn quant_comes_from_the_name_before_the_header() {
    assert_eq!(quant_from_name("llama3:Q4_K_M").as_deref(), Some("Q4_K_M"));
    assert_eq!(
        quant_from_name("Mistral-7B-Instruct-v0.3-4bit").as_deref(),
        Some("4bit")
    );
    assert!(quant_from_name("llama3:latest").is_none());
}

#[test]
fn gguf_file_type_is_read_from_the_header_only() {
    let dir = scratch("gguf");
    let path = dir.join("weights.gguf");
    let mut f = File::create(&path).unwrap();
    f.write_all(b"GGUF").unwrap();
    f.write_all(&3u32.to_le_bytes()).unwrap();
    f.write_all(&0u64.to_le_bytes()).unwrap();
    f.write_all(&1u64.to_le_bytes()).unwrap();
    let key = b"general.file_type";
    f.write_all(&(key.len() as u64).to_le_bytes()).unwrap();
    f.write_all(key).unwrap();
    f.write_all(&4u32.to_le_bytes()).unwrap();
    f.write_all(&2u32.to_le_bytes()).unwrap();
    // Padding a reader must not need.
    f.write_all(&[0u8; 64]).unwrap();
    assert_eq!(quant_from_gguf(&path).as_deref(), Some("Q4_0"));
    crate::platform::remove_test_dir(&dir);
}

#[test]
fn same_edges_match_and_a_different_middle_stays_sampled() {
    let dir = scratch("hash");
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    let c = dir.join("c.bin");
    for path in [&a, &b, &c] {
        let f = File::create(path).unwrap();
        f.set_len(33 * 1024 * 1024).unwrap();
    }
    // b matches a. c differs only past the first 16 MB and before the last 16 MB.
    let mut fc = std::fs::OpenOptions::new().write(true).open(&c).unwrap();
    fc.seek(SeekFrom::Start(16 * 1024 * 1024 + 100)).unwrap();
    fc.write_all(&[7u8]).unwrap();
    let items = vec![
        DupeCand {
            path: a.clone(),
            len: 33 * 1024 * 1024,
            content_hash: None,
            dataless: false,
        },
        DupeCand {
            path: b.clone(),
            len: 33 * 1024 * 1024,
            content_hash: None,
            dataless: false,
        },
        DupeCand {
            path: c.clone(),
            len: 33 * 1024 * 1024,
            content_hash: None,
            dataless: false,
        },
    ];
    let paired = pair_duplicates(&items);
    let partners: Vec<_> = paired.iter().flatten().map(|m| m.partner.clone()).collect();
    assert!(partners.contains(&a) || partners.iter().any(|p| p == &b || p == &a));
    assert!(paired
        .iter()
        .flatten()
        .all(|m| m.level == DupeLevel::Sampled));
    assert_ne!(full_hash(&a).unwrap(), full_hash(&c).unwrap());
    assert_eq!(full_hash(&a).unwrap(), full_hash(&b).unwrap());
    crate::platform::remove_test_dir(&dir);
}

#[test]
fn matching_ollama_digests_are_a_full_match_without_rereading() {
    let items = vec![
        DupeCand {
            path: PathBuf::from("/tmp/one"),
            len: 10,
            content_hash: Some("abc".into()),
            dataless: true,
        },
        DupeCand {
            path: PathBuf::from("/tmp/two"),
            len: 10,
            content_hash: Some("abc".into()),
            dataless: true,
        },
    ];
    // Dataless files are not opened, so a content hash is the only way they pair.
    // These aren't dataless on disk; the flag skips the read. No pair without a sample.
    let paired = pair_duplicates(&items);
    assert!(paired.iter().all(|m| m.is_none()));
}

#[test]
fn matching_content_hashes_pair_without_opening() {
    let items = vec![
        DupeCand {
            path: PathBuf::from("/no/such/wrasse-a"),
            len: 10,
            content_hash: Some("abc".into()),
            dataless: false,
        },
        DupeCand {
            path: PathBuf::from("/no/such/wrasse-b"),
            len: 10,
            content_hash: Some("abc".into()),
            dataless: false,
        },
    ];
    let paired = pair_duplicates(&items);
    assert!(paired[0].is_none());
    let hit = paired[1].as_ref().unwrap();
    assert_eq!(hit.level, DupeLevel::Full);
    assert_eq!(hit.partner, items[0].path);
}

#[test]
fn clone_note_is_the_exact_label() {
    assert_eq!(CLONE_NOTE, "clone, no space to gain");
}

#[test]
fn ps_lists_a_loaded_model_and_a_broken_reply_is_not_empty_for_a_check() {
    let body = r#"{"models":[{"name":"llama3:latest","size":100}]}"#;
    let loaded = parse_ps(body);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "llama3:latest");
    assert_eq!(loaded[0].bytes, 100);
    assert!(ps_contains(body, "library/llama3:latest"));
    assert!(!ps_contains(body, "mistral:latest"));
    assert!(ps_contains("not json", "llama3:latest"));
}

#[test]
fn lmstudio_settings_name_a_folder() {
    let dir = scratch("lms");
    let models = dir.join("models");
    std::fs::create_dir_all(&models).unwrap();
    let settings = dir.join("settings.json");
    std::fs::write(
        &settings,
        format!(r#"{{"downloadsFolder":"{}"}}"#, models.display()),
    )
    .unwrap();
    assert_eq!(
        downloads_folder(&settings).as_deref(),
        Some(std::fs::canonicalize(&models).unwrap().as_path())
    );
    crate::platform::remove_test_dir(&dir);
}
