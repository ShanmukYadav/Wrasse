//! Wrasse core: all scanning, rules and cleanup logic. No Tauri dependency.

pub mod apps;
pub mod devjunk;
pub mod diagnostics;
pub mod files;
pub mod format;
pub mod llm;
pub mod platform;
pub mod quarantine;
pub mod rank;
pub mod rules;
pub mod safety;
pub mod scan;
pub mod session;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
