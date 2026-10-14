//! One folder per feature, each built on scan/, safety/ and platform/:
//! apps (uninstall), devjunk (developer caches and projects), diagnostics (snapshots, free space),
//! files (Large & Old, duplicates), llm (AI models), rank (Free Up Space), startup (Startup Items).

pub mod apps;
pub mod devjunk;
pub mod diagnostics;
pub mod files;
pub mod llm;
pub mod rank;
#[cfg(target_os = "macos")]
pub mod startup;
