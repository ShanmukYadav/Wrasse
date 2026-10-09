//! Thin CLI over wrasse-core. Full commands arrive in Phase 6; for now it can run a read-only
//! scan and print totals, which the Phase 1 checkpoint uses.
//!
//!   wrasse scan            whole volume, with the category breakdown
//!   wrasse scan <folder>   one folder (e.g. ~ to compare with `du -sk ~`)
//!   wrasse scan --dump <file>   whole volume, and write the full breakdown as JSON
//!   wrasse scan --cache <dir>   whole volume, incremental when <dir> holds a previous scan

use std::path::PathBuf;
use std::time::Instant;
use wrasse_core::scan::{scan, ScanControl};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") if args.get(1).map(String::as_str) == Some("--dump") => {
            run_scan(None, args.get(2).map(PathBuf::from), None)
        }
        Some("scan") if args.get(1).map(String::as_str) == Some("--cache") => {
            run_scan(None, None, args.get(2).map(PathBuf::from))
        }
        Some("scan") => run_scan(args.get(1).map(PathBuf::from), None, None),
        _ => println!(
            "wrasse {}\nusage: wrasse scan [folder]",
            wrasse_core::VERSION
        ),
    }
}

fn run_scan(folder: Option<PathBuf>, dump: Option<PathBuf>, cache: Option<PathBuf>) {
    let started = Instant::now();
    let control = ScanControl::default();
    match folder {
        Some(path) => {
            let tree = scan(&path, &control).expect("scan failed");
            let s = tree.stats;
            println!(
                "{}",
                serde_json::json!({
                    "path": path, "bytes": tree.node(0).size, "kilobytes": tree.node(0).size / 1024,
                    "files": s.files, "dirs": s.dirs, "unreadableDirs": s.unreadable_dirs,
                    "datalessItems": s.dataless_items, "hardlinksDeduped": s.hardlinks_deduped,
                    "cloneSharedBytes": s.clone_shared_bytes, "arenaNodes": tree.nodes.len(),
                    "seconds": started.elapsed().as_secs_f64(),
                })
            );
        }
        None => {
            let disk =
                wrasse_core::session::scan_volume(&control, cache.as_deref()).expect("scan failed");
            if let Some(path) = dump {
                std::fs::write(&path, serde_json::to_vec(&disk).unwrap())
                    .expect("couldn't write the dump");
            }
            let ui_nodes = disk.nodes.len();
            let mut out = serde_json::to_value(&disk.summary).unwrap();
            out["uiNodes"] = ui_nodes.into();
            out["seconds"] = started.elapsed().as_secs_f64().into();
            out["categories"] = disk.nodes[0]
                .children
                .iter()
                .map(|&c| {
                    let n = &disk.nodes[c as usize];
                    serde_json::json!({ "name": n.name, "bytes": n.size })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
    }
}
