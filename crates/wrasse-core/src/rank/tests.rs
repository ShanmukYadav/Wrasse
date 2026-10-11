//! Free Up Space on a synthetic tree (docs/phase3-plan.md §7: reaches a 10 GB target, never offers
//! hand-pick-only, running, keep, protected or aggregate items, orders by score, reports the
//! shortfall).

use super::*;
use serde_json::{json, Value};

const GB: u64 = 1_000_000_000;

struct T {
    nodes: Vec<Value>,
}

impl T {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        parent: i64,
        name: &str,
        path: &str,
        size: u64,
        days: u32,
        safety: &str,
        extra: Value,
    ) -> u32 {
        let id = self.nodes.len() as u32;
        let depth = if parent < 0 {
            0
        } else {
            self.nodes[parent as usize]["depth"].as_u64().unwrap() + 1
        };
        let mut n = json!({
            "id": id, "name": name, "path": path, "size": size, "parent": parent, "children": [],
            "depth": depth, "category": "files", "kind": "file", "lastOpenedDays": days,
            "safety": safety, "why": "", "lenses": 0, "deep": 0, "matchBytes": {}, "grewBy": 0, "shade": 0
        });
        for (k, v) in extra.as_object().unwrap() {
            n[k] = v.clone();
        }
        if parent >= 0 {
            self.nodes[parent as usize]["children"]
                .as_array_mut()
                .unwrap()
                .push(json!(id));
        }
        self.nodes.push(n);
        id
    }

    fn disk(self) -> UiDisk {
        serde_json::from_value(json!({
            "nodes": self.nodes, "root": 0, "capacity": 500 * GB, "ghostId": -1, "leafCount": 0,
            "summary": {
                "capacity": 0, "available": 0, "osUsed": 0, "scanned": 0, "unaccounted": 0, "home": 0,
                "files": 0, "dirs": 0, "unreadableDirs": 0, "datalessItems": 0, "hardlinksDeduped": 0,
                "cloneSharedBytes": 0, "cancelled": false
            }
        }))
        .unwrap()
    }
}

fn app_meta(route: &str, never_opened: bool) -> Value {
    json!({
        "source": { "kind": if route == "notRemovable" { "system" } else { "folder" } },
        "owner": "user", "neverOpened": never_opened, "lastUsedKnown": true,
        "route": { "kind": route }, "adminLeftovers": []
    })
}

struct Fixture {
    disk: UiDisk,
    ids: std::collections::HashMap<&'static str, u32>,
}

fn fixture() -> Fixture {
    let mut t = T { nodes: Vec::new() };
    let mut ids = std::collections::HashMap::new();
    let root = t.add(
        -1,
        "Macintosh HD",
        "/",
        0,
        0,
        "keep",
        json!({ "kind": "volume" }),
    );
    let files = t.add(
        root as i64,
        "Your Files",
        "/Users/me",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir" }),
    );
    let f = files as i64;
    ids.insert(
        "thesis",
        t.add(
            f,
            "old-thesis.mov",
            "/Users/me/Documents/old-thesis.mov",
            3 * GB,
            400,
            "yourCall",
            json!({}),
        ),
    );
    ids.insert(
        "icloud",
        t.add(
            f,
            "notes.pages",
            "/Users/me/Library/Mobile Documents/notes.pages",
            2 * GB,
            400,
            "yourCall",
            json!({}),
        ),
    );
    ids.insert(
        "iso",
        t.add(
            f,
            "big.iso",
            "/Users/me/Downloads/big.iso",
            4 * GB,
            300,
            "yourCall",
            json!({}),
        ),
    );
    ids.insert(
        "recent",
        t.add(
            f,
            "recent.mov",
            "/Users/me/Movies/recent.mov",
            5 * GB,
            2,
            "yourCall",
            json!({}),
        ),
    );
    ids.insert(
        "smaller",
        t.add(
            f,
            "Smaller items",
            "/Users/me",
            2 * GB,
            400,
            "safe",
            json!({ "aggregate": true }),
        ),
    );
    ids.insert(
        "dataless",
        t.add(
            f,
            "film.mov",
            "/Users/me/Movies/film.mov",
            GB,
            400,
            "yourCall",
            json!({ "note": "In iCloud only" }),
        ),
    );

    let apps = t.add(
        root as i64,
        "Apps",
        "/Applications",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir", "category": "apps" }),
    );
    let a = apps as i64;
    let old = t.add(
        a,
        "OldApp",
        "/Applications/OldApp.app",
        0,
        200,
        "yourCall",
        json!({ "kind": "app", "category": "apps", "app": app_meta("tray", false) }),
    );
    ids.insert("oldApp", old);
    ids.insert(
        "oldBundle",
        t.add(
            old as i64,
            "OldApp.app",
            "/Applications/OldApp.app",
            3 * GB,
            200,
            "yourCall",
            json!({ "category": "apps", "appBundle": true }),
        ),
    );
    ids.insert("oldCaches", t.add(old as i64, "Caches", "/Users/me/Library/Caches/com.old.app", GB, 200, "safe", json!({ "category": "apps", "leftover": "caches", "leftoverMeta": { "matchedBy": "bundleId", "sharedWith": [], "defaultOff": false } })));
    ids.insert("oldSupport", t.add(old as i64, "Application Support", "/Users/me/Library/Application Support/OldApp", GB / 2, 200, "yourCall", json!({ "category": "apps", "leftover": "support", "leftoverMeta": { "matchedBy": "name", "sharedWith": [], "defaultOff": true } })));
    let running = t.add(a, "Busy", "/Applications/Busy.app", 0, 400, "yourCall", json!({ "kind": "app", "category": "apps", "running": true, "app": app_meta("tray", false) }));
    ids.insert("busy", running);
    t.add(
        running as i64,
        "Busy.app",
        "/Applications/Busy.app",
        2 * GB,
        400,
        "yourCall",
        json!({ "category": "apps", "appBundle": true }),
    );
    let sys = t.add(
        a,
        "Safari",
        "/Applications/Safari.app",
        0,
        400,
        "keep",
        json!({ "kind": "app", "category": "apps", "app": app_meta("notRemovable", false) }),
    );
    ids.insert("system", sys);
    t.add(
        sys as i64,
        "Safari.app",
        "/Applications/Safari.app",
        5 * GB,
        400,
        "keep",
        json!({ "category": "apps", "appBundle": true }),
    );
    let gone = t.add(
        a,
        "Leftovers from removed apps",
        "/Users/me/Library",
        0,
        400,
        "yourCall",
        json!({ "kind": "dir", "category": "apps", "orphan": true }),
    );
    let o = t.add(
        gone as i64,
        "com.gone.app",
        "/Users/me/Library/Caches/com.gone.app",
        0,
        400,
        "yourCall",
        json!({ "kind": "dir", "category": "apps", "orphan": true }),
    );
    ids.insert("orphanGroup", o);
    ids.insert(
        "orphan",
        t.add(
            o as i64,
            "Caches",
            "/Users/me/Library/Caches/com.gone.app",
            3 * GB / 2,
            400,
            "yourCall",
            json!({ "category": "apps", "orphan": true }),
        ),
    );

    let junk = t.add(
        root as i64,
        "Library & caches",
        "/Users/me/Library",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir", "category": "junk" }),
    );
    let j = junk as i64;
    ids.insert(
        "caches",
        t.add(
            j,
            "Spotify",
            "/Users/me/Library/Caches/com.spotify.client",
            2 * GB,
            10,
            "safe",
            json!({ "category": "junk", "ruleId": "app-caches", "rebuildable": true }),
        ),
    );
    ids.insert(
        "trash",
        t.add(
            j,
            "Trash",
            "/Users/me/.Trash",
            3 * GB,
            30,
            "safe",
            json!({ "category": "junk", "delegate": true }),
        ),
    );
    ids.insert(
        "keep",
        t.add(
            j,
            "Mail",
            "/Users/me/Library/Mail",
            2 * GB,
            400,
            "keep",
            json!({ "category": "junk" }),
        ),
    );
    ids.insert(
        "readOnly",
        t.add(
            j,
            "Photos Library.photoslibrary",
            "/Users/me/Pictures/Photos Library.photoslibrary",
            6 * GB,
            400,
            "yourCall",
            json!({ "readOnly": true }),
        ),
    );
    // Library internals no rule covers (an app's database, a search index): never offered.
    ids.insert(
        "libraryDb",
        t.add(
            j,
            "globalKnowledge.db",
            "/Users/me/Library/IntelligencePlatform/globalKnowledge.db",
            3 * GB,
            400,
            "yourCall",
            json!({ "category": "junk" }),
        ),
    );
    // A rule match with children: offered whole, its parts never on their own.
    let chrome = t.add(
        j,
        "Chrome",
        "/Users/me/Library/Caches/Google/Chrome/Default",
        0,
        30,
        "safe",
        json!({ "kind": "dir", "category": "junk", "ruleId": "chrome-cache" }),
    );
    ids.insert("chrome", chrome);
    ids.insert(
        "chromePart",
        t.add(
            chrome as i64,
            "Cache",
            "/Users/me/Library/Caches/Google/Chrome/Default/Cache",
            GB,
            30,
            "safe",
            json!({ "category": "junk" }),
        ),
    );
    let dev = t.add(
        root as i64,
        "Developer",
        "/Users/me/Library/Developer",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir", "category": "developer" }),
    );
    let nm = t.add(
        dev as i64,
        "node_modules",
        "/Users/me/Projects/web/node_modules",
        0,
        1,
        "safe",
        json!({ "kind": "dir", "category": "developer", "rebuildable": true }),
    );
    ids.insert("nodeModules", nm);
    ids.insert(
        "nmPart",
        t.add(
            nm as i64,
            "react",
            "/Users/me/Projects/web/node_modules/react",
            GB,
            1,
            "safe",
            json!({ "category": "developer" }),
        ),
    );
    ids.insert(
        "toolchain",
        t.add(
            dev as i64,
            "Xcode toolchain",
            "/Users/me/Library/Developer/Toolchains/x",
            2 * GB,
            400,
            "yourCall",
            json!({ "category": "developer" }),
        ),
    );
    let sys = t.add(
        root as i64,
        "System",
        "/System",
        0,
        0,
        "keep",
        json!({ "kind": "dir", "category": "system" }),
    );
    ids.insert(
        "sysFile",
        t.add(
            sys as i64,
            "swapfile",
            "/private/var/vm/swapfile0",
            2 * GB,
            400,
            "yourCall",
            json!({ "category": "system" }),
        ),
    );
    // Folder sizes are the sum of their children (the ranker reads each node's own size).
    for i in (0..t.nodes.len()).rev() {
        let kids: Vec<usize> = t.nodes[i]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_u64().unwrap() as usize)
            .collect();
        if !kids.is_empty() {
            let sum: u64 = kids
                .iter()
                .map(|&c| t.nodes[c]["size"].as_u64().unwrap())
                .sum();
            t.nodes[i]["size"] = json!(sum);
        }
    }
    Fixture {
        disk: t.disk(),
        ids,
    }
}

fn limits() -> Limits {
    Limits {
        hand_pick_only: vec![
            PathBuf::from("/Users/me/Documents"),
            PathBuf::from("/Users/me/Desktop"),
            PathBuf::from("/Users/me/Library/Mobile Documents"),
        ],
        ..Default::default()
    }
}

#[test]
fn unused_factor_and_weights() {
    assert_eq!(unused_factor(0), 0.1);
    assert!((unused_factor(30) - 0.5).abs() < 1e-9);
    assert!((unused_factor(15) - 0.3).abs() < 1e-9);
    assert!((unused_factor(105) - 0.75).abs() < 1e-9);
    assert_eq!(unused_factor(180), 1.0);
    assert_eq!(unused_factor(5000), 1.0);
    assert_eq!(safety_weight(Safety::Keep), 0.0);
}

#[test]
fn reaches_a_10_gb_target_with_only_what_may_go() {
    let f = fixture();
    let p = plan(&f.disk, 10 * GB, &limits());
    assert!(p.total >= 10 * GB && p.shortfall == 0, "{p:#?}");
    let chosen: Vec<u32> = p.items.iter().map(|c| c.id).collect();
    let id = |k: &str| f.ids[k];
    // Score order: big.iso and OldApp (4 GB, unused, your call) tie, then the removed app's
    // leftovers, then the Chrome cache (a rule match, whole, used 30 days ago).
    assert_eq!(
        chosen,
        [id("iso"), id("oldApp"), id("orphanGroup"), id("chrome")]
    );
    let old = p.items.iter().find(|c| c.id == id("oldApp")).unwrap();
    assert_eq!(
        old.parts,
        [id("oldBundle"), id("oldCaches")],
        "name-matched leftovers stay unticked"
    );
    assert_eq!(old.bytes, 4 * GB);
    assert_eq!(
        p.items
            .iter()
            .find(|c| c.id == id("orphanGroup"))
            .unwrap()
            .why,
        Why::AppRemoved
    );
    assert_eq!(
        p.items.iter().find(|c| c.id == id("chrome")).unwrap().why,
        Why::Rebuilds
    );
    let iso = p.items.iter().find(|c| c.id == id("iso")).unwrap();
    assert_eq!(iso.why, Why::LargeAndOld);
    assert!(
        !iso.default_off,
        "300 days is under a year, so it still starts ticked"
    );
    // Never offered, whatever the target.
    let (all, held_recent, held_running) = candidates(&f.disk, &limits());
    for k in [
        "thesis",
        "icloud",
        "recent",
        "smaller",
        "dataless",
        "busy",
        "system",
        "trash",
        "keep",
        "readOnly",
        "oldSupport",
        "libraryDb",
        "chromePart",
        "nmPart",
        "toolchain",
        "sysFile",
    ] {
        assert!(
            !all.iter()
                .any(|c| c.id == id(k) || c.parts.contains(&id(k))),
            "{k} must never be offered"
        );
    }
    assert_eq!(held_recent, 5 * GB, "used two days ago");
    assert_eq!(held_running, 2 * GB, "the app is open");
}

#[test]
fn shortfall_gone_items_and_never_touch() {
    let f = fixture();
    let p = plan(&f.disk, 20 * GB, &limits());
    // Everything offerable: 4 + 4 + 1.5 + 1 (Chrome) + 2 (Spotify) + 1 (node_modules).
    assert_eq!(p.total, 27 * GB / 2);
    assert_eq!(p.shortfall, 20 * GB - 27 * GB / 2);
    assert_eq!(
        p.held_recent,
        5 * GB,
        "the movie used two days ago; rebuilding caches aren't held"
    );

    let mut l = limits();
    l.gone.insert(f.ids["iso"]);
    l.gone.insert(f.ids["oldBundle"]); // the bundle is already in the tray: the app isn't offered
    l.never_touch
        .push(PathBuf::from("/Users/me/Library/Caches"));
    let p = plan(&f.disk, 10 * GB, &l);
    let chosen: Vec<u32> = p.items.iter().map(|c| c.id).collect();
    // Only node_modules is left: the rest is gone or under the never-touch Caches folder.
    assert_eq!(chosen, [f.ids["nodeModules"]]);
    assert_eq!(p.total, GB);
    assert_eq!(p.shortfall, 9 * GB);
}

/// Only an approximate date over a year starts unticked. A known last-used date over a year
/// stays ticked, and so does a younger file.
#[test]
fn approx_old_files_and_models_start_unticked() {
    let mut t = T { nodes: Vec::new() };
    let root = t.add(
        -1,
        "Macintosh HD",
        "/",
        0,
        0,
        "keep",
        json!({ "kind": "volume" }),
    );
    let files = t.add(
        root as i64,
        "Your Files",
        "/Users/me",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir" }),
    );
    let known = t.add(
        files as i64,
        "known.iso",
        "/Users/me/Downloads/known.iso",
        2 * GB,
        400,
        "yourCall",
        json!({ "kind": "file", "approx": false }),
    );
    let old = t.add(
        files as i64,
        "old.iso",
        "/Users/me/Downloads/old.iso",
        2 * GB,
        400,
        "yourCall",
        json!({ "kind": "file", "approx": true }),
    );
    let younger = t.add(
        files as i64,
        "year.iso",
        "/Users/me/Downloads/year.iso",
        2 * GB,
        200,
        "yourCall",
        json!({ "kind": "file", "approx": true }),
    );
    let models = t.add(
        root as i64,
        "AI Models",
        "/Users/me/models",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir", "category": "aiModels" }),
    );
    let known_model = t.add(
        models as i64,
        "known.gguf",
        "/Users/me/models/known.gguf",
        2 * GB,
        500,
        "yourCall",
        json!({ "kind": "model", "category": "aiModels", "approx": false }),
    );
    let model = t.add(
        models as i64,
        "old.gguf",
        "/Users/me/models/old.gguf",
        2 * GB,
        500,
        "yourCall",
        json!({ "kind": "model", "category": "aiModels", "approx": true }),
    );
    let p = plan(&t.disk(), 100 * GB, &Limits::default());
    let off = |id: u32| p.items.iter().find(|c| c.id == id).unwrap().default_off;
    assert!(
        !off(known),
        "a known last-used date over a year stays ticked"
    );
    assert!(off(old), "an approximate date over a year starts unticked");
    assert!(!off(younger), "under a year still starts ticked");
    assert!(
        !off(known_model),
        "a model with a known date over a year stays ticked"
    );
    assert!(
        off(model),
        "a model with an approximate old date starts unticked"
    );
}

#[test]
fn icloud_duplicate_models_are_not_offered() {
    let mut t = T { nodes: Vec::new() };
    let root = t.add(-1, "HD", "/", 0, 0, "keep", json!({ "kind": "volume" }));
    let ai = t.add(
        root as i64,
        "AI Models",
        "/Users/me/.ollama",
        0,
        0,
        "yourCall",
        json!({ "kind": "dir", "category": "aiModels" }),
    );
    let cache_path = "/Users/me/.cache/huggingface/hub/models--a/snapshots/rev/model.safetensors";
    let cloud_path =
        "/Users/me/Library/Mobile Documents/com~apple~CloudDocs/Backup/model.safetensors";
    let cache = t.add(
        ai as i64,
        "cache-copy",
        cache_path,
        2 * GB,
        400,
        "yourCall",
        json!({
            "kind": "model",
            "category": "aiModels",
            "model": {
                "tool": "huggingFace",
                "removal": "huggingFace",
                "weight": cache_path,
                "partnerWeight": cloud_path,
                "dupeLevel": "sampled"
            }
        }),
    );
    let cloud = t.add(
        ai as i64,
        "cloud-copy",
        cloud_path,
        2 * GB,
        400,
        "yourCall",
        json!({
            "kind": "model",
            "category": "aiModels",
            "model": {
                "tool": "huggingFace",
                "removal": "huggingFace",
                "weight": cloud_path
            }
        }),
    );
    let ollama = t.add(
        ai as i64,
        "llama",
        "/Users/me/.ollama/models/manifests/library/llama/latest",
        3 * GB,
        200,
        "yourCall",
        json!({
            "kind": "model",
            "category": "aiModels",
            "model": { "tool": "ollama", "removal": "ollama" }
        }),
    );
    let (offered, _, _) = candidates(&t.disk(), &limits());
    let ids: Vec<u32> = offered.iter().map(|c| c.id).collect();
    assert!(
        !ids.contains(&cache),
        "the other copy of an iCloud duplicate stays unselected"
    );
    assert!(!ids.contains(&cloud), "the iCloud copy stays unselected");
    assert!(
        ids.contains(&ollama),
        "a model outside that pair is still offered"
    );
}
