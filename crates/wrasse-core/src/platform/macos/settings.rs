//! Opens System Settings at Privacy & Security › Full Disk Access (PLAN.md §5.5).

use std::io;
use std::process::Command;

const FULL_DISK_ACCESS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

pub fn open_full_disk_access_settings() -> io::Result<()> {
    Command::new("/usr/bin/open")
        .arg(FULL_DISK_ACCESS)
        .status()
        .map(|_| ())
}
