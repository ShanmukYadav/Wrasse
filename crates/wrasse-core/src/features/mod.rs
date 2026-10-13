//! One folder per feature, each built on scan/, safety/ and platform/:
//! apps (uninstall), devjunk (developer caches and projects), diagnostics (snapshots, free space),
//! files (Large & Old, duplicates), llm (AI models), rank (Free Up Space).

pub mod apps;
pub mod devjunk;
pub mod diagnostics;
pub mod files;
pub mod llm;
pub mod rank;
