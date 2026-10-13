//! Space diagnostics (PLAN.md §6.7): why deleted space has not come back.
//!
//! Read-only measurement lives here. Thinning snapshots goes through `safety::delegate`.
//! Nothing in this module deletes, renames, or starts a program.

use crate::scan::breakdown::{Category, NodeKind, Safety, UiDisk, UiNode};
use serde::{Deserialize, Serialize};

/// One local Time Machine snapshot name, as `tmutil listlocalsnapshots` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Free space the file manager shows. Purgeable space is not included.
    pub available: u64,
    /// Free space including what macOS can reclaim for an important allocation.
    pub important_available: u64,
    /// `important_available - available`. macOS reclaims this when an app needs it.
    pub purgeable: u64,
    /// Local Time Machine snapshot names. Empty when there are none.
    pub snapshots: Vec<String>,
    /// Estimated bytes those snapshots are holding. 0 when there are none.
    /// macOS does not publish a per-snapshot size; while any exist, this is the purgeable space.
    pub snapshot_bytes: u64,
    /// The user's Trash, plus per-volume `.Trashes` for this user.
    pub trash_bytes: u64,
    /// The Wrasse tray (quarantine).
    pub tray_bytes: u64,
    /// Free space expected after a cleanup, once `with_expectation` has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_available: Option<u64>,
    /// `expected_available - available`, or 0 when the disk already has at least that much free.
    #[serde(default)]
    pub gap: u64,
}

/// Names from `tmutil listlocalsnapshots` output. The header line is skipped.
pub(crate) fn parse_snapshots(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Snapshots for ") {
            continue;
        }
        // Local Time Machine snapshots end in `.local`. The sealed OS snapshot does not
        // appear in this list.
        if line.contains("TimeMachine") || line.ends_with(".local") {
            names.push(line.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Purgeable space: what an important allocation can use, beyond what looks free.
pub(crate) fn purgeable_bytes(important: u64, available: u64) -> u64 {
    important.saturating_sub(available)
}

/// While local snapshots exist, the only size macOS gives us without thinning is the
/// purgeable space. With no snapshots, that space has another cause, so the estimate is 0.
pub(crate) fn snapshot_bytes(snapshots: usize, purgeable: u64) -> u64 {
    if snapshots == 0 {
        0
    } else {
        purgeable
    }
}

/// Bytes `thinlocalsnapshots` is asked to free. The purgeable estimate when it is
/// non-zero; otherwise a 1 GB request so a snapshot with no published size still thins.
pub fn thin_request(snapshots: usize, purgeable: u64) -> u64 {
    if snapshots == 0 {
        0
    } else if purgeable > 0 {
        purgeable
    } else {
        1_000_000_000
    }
}

/// Fills expected free space (`before + freed_now`) and the gap against `available`.
pub fn with_expectation(mut report: Report, before_available: u64, freed_now: u64) -> Report {
    let expected = before_available.saturating_add(freed_now);
    report.gap = expected.saturating_sub(report.available);
    report.expected_available = Some(expected);
    report
}

/// Live read of snapshots, purgeable space, Trash and the tray. Never thins or deletes.
pub fn measure(home: &std::path::Path, tray_bytes: u64) -> Report {
    let text = crate::platform::snapshot_list_text();
    let snapshots = parse_snapshots(&text);
    let (available, important) = crate::platform::volume_capacities().unwrap_or((0, 0));
    let purgeable = purgeable_bytes(important, available);
    Report {
        available,
        important_available: important,
        purgeable,
        snapshot_bytes: snapshot_bytes(snapshots.len(), purgeable),
        snapshots,
        trash_bytes: trash_bytes(home),
        tray_bytes,
        expected_available: None,
        gap: 0,
    }
}

/// How many bytes the current snapshots are estimated to hold. 0 on any read failure.
pub(crate) fn held_now() -> u64 {
    let snapshots = parse_snapshots(&crate::platform::snapshot_list_text());
    let purgeable = crate::platform::volume_capacities().map_or(0, |(a, i)| purgeable_bytes(i, a));
    snapshot_bytes(snapshots.len(), purgeable)
}

/// The user's Trash and other volumes' `.Trashes/<uid>`. Symlinks are not followed.
pub(crate) fn trash_bytes(home: &std::path::Path) -> u64 {
    crate::quarantine::size_on_disk(&home.join(".Trash")) + volume_trashes()
}

/// Per-volume Trash folders for this user. Other volumes are a Unix path.
#[cfg(unix)]
fn volume_trashes() -> u64 {
    let mut total = 0u64;
    let name = unsafe { libc::geteuid() }.to_string();
    if let Ok(vols) = std::fs::read_dir("/Volumes") {
        for vol in vols.flatten() {
            total += crate::quarantine::size_on_disk(&vol.path().join(".Trashes").join(&name));
        }
    }
    total
}

#[cfg(not(unix))]
fn volume_trashes() -> u64 {
    0
}

/// Shows held snapshot space as the dashed tile. The bytes move out of "System data"
/// so the volume total does not grow. A held size of 0 removes the tile.
pub(crate) fn attach_ghost(disk: &mut UiDisk, held: u64) {
    detach_ghost(disk);
    if held == 0 {
        return;
    }
    let Some(sd) = disk
        .nodes
        .iter()
        .position(|n| n.name == "System data" && n.category == Category::System)
    else {
        return;
    };
    let sd = sd as u32;
    let take = held.min(disk.nodes[sd as usize].size);
    if take == 0 {
        return;
    }
    let mut cur = Some(sd);
    while let Some(id) = cur {
        disk.nodes[id as usize].size = disk.nodes[id as usize].size.saturating_sub(take);
        let parent = disk.nodes[id as usize].parent;
        cur = (parent >= 0).then_some(parent as u32);
    }
    let id = disk.nodes.len() as u32;
    let mut node = UiNode::plain(
        id,
        "Held by snapshots",
        "/",
        i64::from(disk.root),
        1,
        Category::Other,
        NodeKind::Ghost,
        Safety::Keep,
        "Space from files you already deleted. macOS keeps it in local Time Machine snapshots until they expire, usually within a day.",
    );
    node.size = take;
    node.delegate = true;
    node.delegate_action = Some("thinSnapshots".into());
    disk.nodes.push(node);
    disk.nodes[disk.root as usize].children.push(id);
    disk.nodes[disk.root as usize].size += take;
    disk.ghost_id = i64::from(id);
}

fn detach_ghost(disk: &mut UiDisk) {
    if disk.ghost_id < 0 {
        return;
    }
    let gid = disk.ghost_id as u32;
    disk.ghost_id = -1;
    if gid as usize >= disk.nodes.len() {
        return;
    }
    let size = disk.nodes[gid as usize].size;
    let parent = disk.nodes[gid as usize].parent;
    disk.nodes[gid as usize].size = 0;
    if parent >= 0 {
        disk.nodes[parent as usize].children.retain(|&c| c != gid);
    }
    // Those bytes were taken out of System data. Put them back. The root already
    // included them (the ghost was part of its total), so the root size stays.
    if size == 0 {
        return;
    }
    let Some(sd) = disk
        .nodes
        .iter()
        .position(|n| n.name == "System data" && n.category == Category::System)
    else {
        return;
    };
    let root = disk.root;
    let mut cur = Some(sd as u32);
    while let Some(id) = cur {
        if id != root {
            disk.nodes[id as usize].size = disk.nodes[id as usize].size.saturating_add(size);
        }
        let p = disk.nodes[id as usize].parent;
        cur = (p >= 0).then_some(p as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::breakdown::Summary;

    #[test]
    fn snapshot_names_come_from_the_list() {
        let text = "\
Snapshots for disk /:
com.apple.TimeMachine.2026-10-11-120000.local
com.apple.TimeMachine.2026-10-11-130000.local
Snapshots for disk /System/Volumes/Data:
com.apple.TimeMachine.2026-10-11-120000.local
not a snapshot
";
        assert_eq!(
            parse_snapshots(text),
            vec![
                "com.apple.TimeMachine.2026-10-11-120000.local".to_string(),
                "com.apple.TimeMachine.2026-10-11-130000.local".to_string(),
            ]
        );
        assert!(parse_snapshots("Snapshots for disk /:\n").is_empty());
    }

    #[test]
    fn purgeable_is_the_gap_between_the_two_capacities() {
        assert_eq!(purgeable_bytes(110, 100), 10);
        assert_eq!(purgeable_bytes(100, 100), 0);
        assert_eq!(purgeable_bytes(90, 100), 0);
    }

    #[test]
    fn snapshot_estimate_is_purgeable_only_while_snapshots_exist() {
        assert_eq!(snapshot_bytes(0, 5_000), 0);
        assert_eq!(snapshot_bytes(2, 5_000), 5_000);
        assert_eq!(snapshot_bytes(2, 0), 0);
        assert_eq!(thin_request(0, 5_000), 0);
        assert_eq!(thin_request(1, 0), 1_000_000_000);
        assert_eq!(thin_request(1, 20), 20);
    }

    #[test]
    fn expectation_is_before_plus_what_actually_left_the_disk() {
        let report = Report {
            available: 80,
            important_available: 90,
            purgeable: 10,
            snapshots: vec!["com.apple.TimeMachine.2026-10-11-120000.local".into()],
            snapshot_bytes: 10,
            trash_bytes: 0,
            tray_bytes: 0,
            expected_available: None,
            gap: 0,
        };
        let got = with_expectation(report, 70, 30);
        assert_eq!(got.expected_available, Some(100));
        assert_eq!(got.gap, 20);
        let ahead = with_expectation(
            Report {
                available: 200,
                ..got
            },
            70,
            30,
        );
        assert_eq!(ahead.gap, 0);
    }

    #[test]
    fn the_ghost_tile_takes_its_bytes_from_system_data() {
        let mut root = UiNode::plain(
            0,
            "Macintosh HD",
            "/",
            -1,
            0,
            Category::Other,
            NodeKind::Volume,
            Safety::Keep,
            "",
        );
        root.size = 100;
        root.children = vec![1];
        let mut system = UiNode::plain(
            1,
            "System data",
            "/",
            0,
            1,
            Category::System,
            NodeKind::File,
            Safety::Keep,
            "unaccounted",
        );
        system.size = 40;
        let mut disk = UiDisk {
            nodes: vec![root, system],
            root: 0,
            capacity: 200,
            ghost_id: -1,
            leaf_count: 1,
            summary: Summary::default(),
        };
        attach_ghost(&mut disk, 15);
        assert_eq!(disk.ghost_id, 2);
        assert_eq!(disk.nodes[1].size, 25);
        assert_eq!(disk.nodes[2].size, 15);
        assert_eq!(disk.nodes[2].kind, NodeKind::Ghost);
        assert_eq!(
            disk.nodes[2].delegate_action.as_deref(),
            Some("thinSnapshots")
        );
        // The volume total is unchanged: the ghost is the same bytes, shown apart.
        assert_eq!(disk.nodes[0].size, 100);
        attach_ghost(&mut disk, 0);
        assert_eq!(disk.ghost_id, -1);
        assert_eq!(disk.nodes[0].children, vec![1]);
        assert_eq!(disk.nodes[0].size, 100);
        assert_eq!(disk.nodes[1].size, 40);
    }
}
