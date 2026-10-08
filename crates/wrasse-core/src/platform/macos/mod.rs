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
