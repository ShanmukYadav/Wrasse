use super::Platform;
use crate::format::UnitSystem;

pub struct Windows;

/// Mica requires Windows 11, which starts at build 22000.
const FIRST_WINDOWS_11_BUILD: u32 = 22000;

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
    fn window_effects(&self) -> bool {
        sysinfo::System::kernel_version()
            .and_then(|v| parse_build(&v))
            .is_some_and(|build| build >= FIRST_WINDOWS_11_BUILD)
    }
}

/// Accepts "22631" or "10.0.22631".
fn parse_build(version: &str) -> Option<u32> {
    version.rsplit('.').next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_build;

    #[test]
    fn parses_build_numbers() {
        assert_eq!(parse_build("22631"), Some(22631));
        assert_eq!(parse_build("10.0.19045"), Some(19045));
        assert_eq!(parse_build("unknown"), None);
    }
}
