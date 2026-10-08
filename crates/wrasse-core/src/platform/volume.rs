//! Free/total space of the system volume, as the OS file manager reports it.

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSpace {
    pub total: u64,
    pub available: u64,
}

impl VolumeSpace {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
}

/// The volume that holds the OS and the user's data: `/` on macOS (the Data volume shares
/// its APFS container), the system drive on Windows.
pub fn system_volume() -> Option<VolumeSpace> {
    let root = system_root();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .find(|d| d.mount_point() == Path::new(&root))
        .map(|d| VolumeSpace {
            total: d.total_space(),
            available: d.available_space(),
        })
}

fn system_root() -> String {
    if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        format!("{drive}\\")
    } else {
        "/".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_never_underflows() {
        let v = VolumeSpace {
            total: 10,
            available: 20,
        };
        assert_eq!(v.used(), 0);
    }

    #[test]
    fn system_volume_is_found() {
        let v = system_volume().expect("system volume should be listed");
        assert!(v.total > 0 && v.available <= v.total);
    }
}
