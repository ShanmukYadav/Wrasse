//! macOS protected locations (PLAN.md §4.2), plus iCloud-synced folders.

use super::super::{Protected, Protection};
use std::path::{Path, PathBuf};

pub(crate) fn protected(home: &Path) -> Vec<Protected> {
    let hard = |p: PathBuf| Protected {
        path: p,
        protection: Protection::Hard,
    };
    let pick = |p: PathBuf| Protected {
        path: p,
        protection: Protection::HandPickOnly,
    };
    let folder = |p: PathBuf| Protected {
        path: p,
        protection: Protection::FolderOnly,
    };
    let h = |rel: &str| home.join(rel);
    let mut out = vec![
        // AUDIT-BEGIN macos-protected
        hard("/System".into()),
        hard("/usr".into()),
        hard("/bin".into()),
        hard("/sbin".into()),
        hard("/private/var/db".into()),
        hard("/Library/Apple".into()),
        hard("/Users/Shared".into()),
        hard(h("Library/Keychains")),
        hard(h("Library/Mail")),
        hard(h("Library/Messages")),
        hard(h("Library/Accounts")),
        hard(h("Library/Cookies")),
        hard(h("Library/Group Containers/group.com.apple.notes")),
        pick(h("Documents")),
        pick(h("Desktop")),
        pick(h("Library/Mobile Documents")),
        pick(h("Library/CloudStorage")),
        folder(h("Downloads")),
        folder(h("Movies")),
        folder(h("Music")),
        folder(h("Pictures")),
        folder(h("Public")),
        folder(h("Library")),
        folder(h("Applications")),
        folder("/Applications".into()),
        folder("/Library".into()),
        folder("/Users".into()),
        folder("/private".into()),
        folder("/opt".into()),
        // AUDIT-END macos-protected
    ];
    // Other people's home folders: nothing in them is ever touched (current user only).
    if let Ok(entries) = std::fs::read_dir("/Users") {
        for e in entries.flatten() {
            let p = e.path();
            if p != home && p.is_dir() {
                out.push(hard(p));
            }
        }
    }
    out
}

/// Folders iCloud syncs to other devices: iCloud Drive always; Desktop and Documents when
/// "Desktop & Documents Folders" is on (iCloud Drive then shows Desktop and Documents folders).
pub fn synced(home: &Path) -> Vec<PathBuf> {
    let drive = home.join("Library/Mobile Documents");
    let mut out = vec![drive.clone()];
    let cloud_docs = drive.join("com~apple~CloudDocs");
    if cloud_docs.join("Desktop").exists() || cloud_docs.join("Documents").exists() {
        out.push(home.join("Desktop"));
        out.push(home.join("Documents"));
    }
    out
}
