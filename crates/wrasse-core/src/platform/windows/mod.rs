use super::Platform;
use crate::format::UnitSystem;

pub struct Windows;

impl Platform for Windows {
    fn name(&self) -> &'static str {
        "Windows"
    }
    fn file_manager(&self) -> &'static str {
        "File Explorer"
    }
    fn units(&self) -> UnitSystem {
        UnitSystem::Binary
    }
}
