//! Battery level and Low Power Mode, from `pmset` (read-only).

use super::commands::run_readonly;

/// What `pmset -g batt` and `pmset -g` report.
pub fn power_state() -> Option<super::super::PowerState> {
    let batt = run_readonly("/usr/bin/pmset", &["-g", "batt"]).ok()?;
    let batt = String::from_utf8_lossy(&batt.stdout);
    let on_battery = batt.contains("'Battery Power'");
    // "-InternalBattery-0 (id=…)\t33%; discharging; …"; desktops have no battery line.
    let percent = batt
        .split('\t')
        .nth(1)
        .and_then(|rest| rest.split('%').next())
        .and_then(|p| p.trim().parse::<u8>().ok());
    let general = run_readonly("/usr/bin/pmset", &["-g"]).ok()?;
    let low_power = String::from_utf8_lossy(&general.stdout)
        .lines()
        .any(|l| l.trim_start().starts_with("lowpowermode") && l.trim_end().ends_with('1'));
    Some(super::super::PowerState {
        on_battery: on_battery && percent.is_some(),
        percent: percent.unwrap_or(100),
        low_power,
    })
}
