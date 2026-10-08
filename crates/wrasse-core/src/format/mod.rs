//! Size formatting that matches the OS file manager (PLAN.md §6.13).
//! macOS uses decimal units like Finder; Windows uses binary values labeled "GB" like Explorer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystem {
    Decimal,
    Binary,
}

impl UnitSystem {
    pub const fn native() -> Self {
        if cfg!(windows) {
            UnitSystem::Binary
        } else {
            UnitSystem::Decimal
        }
    }

    const fn base(self) -> f64 {
        match self {
            UnitSystem::Decimal => 1000.0,
            UnitSystem::Binary => 1024.0,
        }
    }
}

const UNITS: [&str; 6] = ["bytes", "KB", "MB", "GB", "TB", "PB"];

/// Formats a byte count, e.g. `1.2 GB`. Below 1 KB the exact count is shown.
pub fn format_bytes(bytes: u64, system: UnitSystem) -> String {
    let base = system.base();
    if (bytes as f64) < base {
        return if bytes == 1 {
            "1 byte".into()
        } else {
            format!("{bytes} bytes")
        };
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= base && unit < UNITS.len() - 1 {
        value /= base;
        unit += 1;
    }
    // Avoid "1000 KB": rounding can push the value up to the next unit.
    if value >= 99.95 && format!("{value:.0}").len() > 3 && unit < UNITS.len() - 1 {
        value /= base;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_matches_finder() {
        assert_eq!(format_bytes(0, UnitSystem::Decimal), "0 bytes");
        assert_eq!(format_bytes(1, UnitSystem::Decimal), "1 byte");
        assert_eq!(format_bytes(999, UnitSystem::Decimal), "999 bytes");
        assert_eq!(format_bytes(1_000, UnitSystem::Decimal), "1.0 KB");
        assert_eq!(format_bytes(1_200_000_000, UnitSystem::Decimal), "1.2 GB");
        assert_eq!(format_bytes(250_000_000_000, UnitSystem::Decimal), "250 GB");
        assert_eq!(format_bytes(999_999, UnitSystem::Decimal), "1.0 MB");
    }

    #[test]
    fn binary_matches_explorer() {
        assert_eq!(format_bytes(1_023, UnitSystem::Binary), "1023 bytes");
        assert_eq!(format_bytes(1_024, UnitSystem::Binary), "1.0 KB");
        assert_eq!(format_bytes(1 << 30, UnitSystem::Binary), "1.0 GB");
        assert_eq!(format_bytes(1_200_000_000, UnitSystem::Binary), "1.1 GB");
    }
}
