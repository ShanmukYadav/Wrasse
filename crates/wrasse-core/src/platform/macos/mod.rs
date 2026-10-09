pub mod awake;
pub mod commands;
mod fs;
pub mod fsevents;
pub mod power;
pub mod protect;
mod settings;

pub use fs::{
    clone_shared_bytes, container_space, full_disk_access, is_dataless, lower_thread_priority,
    volume_space_used,
};
pub use settings::open_full_disk_access_settings;

use super::Platform;
use crate::format::UnitSystem;

pub struct MacOs;

impl Platform for MacOs {
    fn name(&self) -> &'static str {
        "macOS"
    }
    fn file_manager(&self) -> &'static str {
        "Finder"
    }
    fn units(&self) -> UnitSystem {
        UnitSystem::Decimal
    }
}
