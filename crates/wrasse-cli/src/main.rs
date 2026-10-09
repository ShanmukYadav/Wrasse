//! Thin CLI over wrasse-core. Full commands arrive in Phase 6; for now it can run a read-only
//! scan and print totals, which the Phase 1 checkpoint uses.
//!
//!   wrasse scan            whole volume, with the category breakdown
//!   wrasse scan <folder>   one folder (e.g. ~ to compare with `du -sk ~`)
//!   wrasse scan --dump <file>   whole volume, and write the full breakdown as JSON
//!   wrasse scan --cache <dir>   whole volume, incremental when <dir> holds a previous scan
//!   wrasse clean [--dry-run] [--delete-now] [--hand-picked] [--safe] <path>...
//!                          move items to the app's quarantine tray (or delete Safe items) through
//!                          the same safety checks as the app; prints the report as JSON
//!   wrasse restore <entry-id>...   put tray items back
//!   wrasse tray            list what's in the tray
//!   wrasse apps [--orphans] list installed apps with source, owner, last use and leftovers (JSON);
//!                          with --orphans, data folders whose app is gone. Read-only.

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
        Some("clean") => clean(&args[1..]),
        Some("restore") => with_ctx(|ctx| {
            let restored = wrasse_core::safety::restore(ctx, &args[1..])?;
            if restored.len() < args.len() - 1 {
                eprintln!(
                    "warning: {} of {} ids weren't in the tray",
                    args.len() - 1 - restored.len(),
                    args.len() - 1
                );
            }
            Ok(serde_json::json!(restored))
        }),
        Some("tray") => with_ctx(|ctx| {
            Ok(serde_json::json!(ctx
                .quarantine
                .stored()
                .into_iter()
                .map(|(_, e)| e)
                .collect::<Vec<_>>()))
        }),
        Some("apps") => {
            let home = wrasse_core::platform::home_dir().expect("no home folder");
            let apps = wrasse_core::platform::installed_apps(&home);
            let v = if args.get(1).map(String::as_str) == Some("--orphans") {
                serde_json::json!(wrasse_core::platform::orphaned_leftovers(&home, &apps))
            } else {
                serde_json::json!(apps)
            };
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
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

const APP_ID: &str = "com.shanmukyadav.wrasse";

/// Runs `f` with the app's safety context (same data folder and tray as the app) and prints JSON.
fn with_ctx(f: impl FnOnce(&wrasse_core::safety::Context) -> std::io::Result<serde_json::Value>) {
    let home = wrasse_core::platform::home_dir().expect("no home folder");
    let data = wrasse_core::scan::cache::data_dir(APP_ID).expect("no data folder");
    let ctx =
        wrasse_core::safety::Context::new(home, data).expect("couldn't open the safety context");
    let _ = ctx.quarantine.reconcile();
    match f(&ctx) {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn clean(args: &[String]) {
    use wrasse_core::safety::{self, Mode, Request, Safety};
    let flag = |f: &str| args.iter().any(|a| a == f);
    let (dry_run, delete_now, hand_picked, safe) = (
        flag("--dry-run"),
        flag("--delete-now"),
        flag("--hand-picked"),
        flag("--safe"),
    );
    let paths: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect();
    with_ctx(|ctx| {
        let reqs: Vec<Request> = paths
            .iter()
            .map(|p| Request {
                path: p.clone(),
                safety: if safe { Safety::Safe } else { Safety::YourCall },
                rule_id: None,
                root: None,
                hand_picked,
            })
            .collect();
        let plan = safety::prepare(
            ctx,
            &reqs,
            if delete_now {
                Mode::DeleteNow
            } else {
                Mode::Quarantine
            },
        );
        let report = safety::remove(ctx, &plan, dry_run)?;
        Ok(serde_json::json!({ "plan": plan, "report": report }))
    });
}
