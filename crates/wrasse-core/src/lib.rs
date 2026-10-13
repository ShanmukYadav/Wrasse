//! Wrasse core: all scanning, rules and cleanup logic. No Tauri dependency.
//!
//! - `scan`: walk the disk, build the tree, break it into what the UI shows
//! - `safety`: the only way anything is removed (to the tray, or delegated to the OS); audit log
//! - `quarantine`: the tray
//! - `rules`: junk rule packs
//! - `features`: apps, developer junk, diagnostics, files, AI models, Free Up Space
//! - `platform`: everything OS-specific

pub mod features;
pub mod format;
pub mod platform;
pub mod quarantine;
pub mod rules;
pub mod safety;
pub mod scan;
pub mod session;

pub use features::{apps, devjunk, diagnostics, files, llm, rank};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
