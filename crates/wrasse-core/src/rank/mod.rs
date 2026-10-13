//! Free Up Space (PLAN.md §6.5, docs/history/phase3-plan.md §4): ranks what could go to reach a target,
//! by `reclaimable bytes × unused factor × safety weight`. It works on the scan's items, taking
//! each source as a whole where the parts only make sense together:
//! - Library: only what a junk rule covers (the rule's match, whole);
//! - Apps: whole apps (bundle plus default leftovers) and removed apps' leftovers;
//! - Developer: folders the tools rebuild; AI Models: whole models;
//! - Your Files: single items as shown (never Documents, Desktop or iCloud Drive);
//! - never System or Other.
//!
//! Read-only: it only proposes.

use crate::scan::breakdown::{Category, NodeKind, Safety, UiDisk, UiNode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Things used more recently than this are never offered (docs/history/phase3-plan.md decision 8).
pub const RECENT_DAYS: u32 = 7;

/// 0 days → 0.1, 30 → 0.5, 180 and more → 1.0, linear in between.
pub fn unused_factor(days: u32) -> f64 {
    let d = f64::from(days);
    if d <= 0.0 {
        0.1
    } else if d <= 30.0 {
        0.1 + 0.4 * d / 30.0
    } else if d < 180.0 {
        0.5 + 0.5 * (d - 30.0) / 150.0
    } else {
        1.0
    }
}

pub fn safety_weight(s: Safety) -> f64 {
    match s {
        Safety::Safe => 1.0,
        Safety::YourCall => 0.7,
        Safety::Keep => 0.0,
    }
}

/// What the ranker must leave out besides what the tree says.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Nodes already removed in this session (the UI detaches them; the tree here is the scan's).
    pub gone: HashSet<u32>,
    /// Places whose items are only ever picked by hand (Documents, Desktop, iCloud Drive).
    pub hand_pick_only: Vec<PathBuf>,
    /// The user's Never touch list.
    pub never_touch: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    /// A cache or other item that rebuilds itself.
    Rebuilds,
    /// An app not opened for a while (with its data).
    UnusedApp,
    /// An app that was never opened.
    NeverOpened,
    /// Data of an app that's no longer installed.
    AppRemoved,
    /// A large file or folder not used for a long time.
    LargeAndOld,
    /// Anything else, ranked by size and age.
    Unused,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The node the user sees (an app, a folder, a file).
    pub id: u32,
    /// What actually goes: the node itself, or an app's bundle plus its default leftovers.
    pub parts: Vec<u32>,
    pub bytes: u64,
    pub last_used_days: u32,
    pub safety: Safety,
    pub score: f64,
    pub why: Why,
    /// Starts unticked. Only a file or model whose date is approximate (`approx`) and over a
    /// year. A known last-used date stays ticked.
    #[serde(default)]
    pub default_off: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub target: u64,
    /// Chosen, best first, until the target is reached.
    pub items: Vec<Candidate>,
    pub total: u64,
    /// How much is missing when the target can't be reached.
    pub shortfall: u64,
    /// Bytes left out only because they were used in the last [`RECENT_DAYS`] days.
    pub held_recent: u64,
    /// Bytes left out because their app is open.
    pub held_running: u64,
}

/// A file or model older than a year whose date is approximate (file dates, not a known last use).
fn approx_old(n: &UiNode) -> bool {
    n.approx && n.last_opened_days > 365 && matches!(n.kind, NodeKind::File | NodeKind::Model)
}

/// Both copies of a duplicate when one of them is in iCloud Drive. Neither is offered.
fn icloud_duplicate_ids(nodes: &[UiNode]) -> HashSet<u32> {
    let mut weights = HashSet::new();
    for n in nodes {
        let Some(m) = &n.model else { continue };
        if m.dupe_level.is_none() && n.duplicate_of.is_none() {
            continue;
        }
        let own = m.weight.as_deref().unwrap_or(n.path.as_str());
        let partner = m.partner_weight.as_deref().unwrap_or("");
        if crate::llm::icloud_synced_path(own)
            || crate::llm::icloud_synced_path(&n.path)
            || crate::llm::icloud_synced_path(partner)
        {
            weights.insert(own.to_string());
            weights.insert(n.path.clone());
            if !partner.is_empty() {
                weights.insert(partner.to_string());
            }
        }
    }
    nodes
        .iter()
        .filter(|n| {
            let own = n
                .model
                .as_ref()
                .and_then(|m| m.weight.as_deref())
                .unwrap_or(n.path.as_str());
            weights.contains(own) || weights.contains(&n.path)
        })
        .map(|n| n.id)
        .collect()
}

fn under(path: &str, places: &[PathBuf]) -> bool {
    let p = Path::new(path);
    places.iter().any(|d| p.starts_with(d))
}

/// Every candidate, best first, with what was held back.
pub fn candidates(disk: &UiDisk, limits: &Limits) -> (Vec<Candidate>, u64, u64) {
    let nodes = &disk.nodes;
    let attached = |n: &UiNode| {
        let mut cur = Some(n);
        while let Some(c) = cur {
            if limits.gone.contains(&c.id) {
                return false;
            }
            cur = (c.parent >= 0).then(|| &nodes[c.parent as usize]);
        }
        true
    };
    let allowed = |n: &UiNode| {
        n.size > 0
            && n.safety != Safety::Keep
            && !n.read_only
            && !n.aggregate
            && !n.delegate
            && n.kind != NodeKind::Ghost
            && n.kind != NodeKind::Volume
            && n.depth > 1
            && n.note.as_deref() != Some("In iCloud only")
            && !under(&n.path, &limits.hand_pick_only)
            && !under(&n.path, &limits.never_touch)
            && attached(n)
    };
    let icloud_dupes = icloud_duplicate_ids(nodes);
    let mut out = Vec::new();
    let (mut held_recent, mut held_running) = (0u64, 0u64);
    let mut push = |c: Candidate| {
        // Recent use holds back what the user made or chose; rebuilding caches can always go.
        if c.last_used_days < RECENT_DAYS && c.why != Why::Rebuilds {
            held_recent += c.bytes;
        } else {
            out.push(c);
        }
    };
    let single = |n: &UiNode, why: Why| Candidate {
        id: n.id,
        parts: vec![n.id],
        bytes: n.size,
        last_used_days: n.last_opened_days,
        safety: n.safety,
        score: n.size as f64 * unused_factor(n.last_opened_days) * safety_weight(n.safety),
        why,
        default_off: approx_old(n),
    };
    // Walk the tree; a node taken whole (an app, a rule match, a rebuildable folder, a model, an
    // orphan) is not looked into further.
    let mut stack: Vec<u32> = vec![disk.root];
    while let Some(id) = stack.pop() {
        let n = &nodes[id as usize];
        if !attached(n) || crate::devjunk::wrasse_path(&n.path) {
            continue;
        }
        let descend = |stack: &mut Vec<u32>| stack.extend(n.children.iter().copied());
        match n.category {
            // macOS and other people's folders: never.
            Category::System | Category::Other if n.depth >= 1 => continue,
            Category::Apps if n.kind == NodeKind::App => {
                // Whole apps: the bundle and the leftovers that start ticked, as one choice.
                let Some(meta) = &n.app else { continue };
                if meta.route == crate::apps::uninstall::Route::NotRemovable {
                    continue;
                }
                let parts: Vec<&UiNode> = n
                    .children
                    .iter()
                    .map(|&c| &nodes[c as usize])
                    .filter(|c| {
                        allowed(c)
                            && (c.app_bundle
                                || c.leftover_meta.as_ref().is_some_and(|m| !m.default_off))
                    })
                    .collect();
                if !parts.iter().any(|p| p.app_bundle) {
                    continue; // the bundle is gone or can't go: nothing to uninstall
                }
                let bytes: u64 = parts.iter().map(|p| p.size).sum();
                if n.running {
                    held_running += bytes;
                    continue;
                }
                push(Candidate {
                    id: n.id,
                    parts: parts.iter().map(|p| p.id).collect(),
                    bytes,
                    last_used_days: n.last_opened_days,
                    safety: Safety::YourCall,
                    score: bytes as f64 * unused_factor(n.last_opened_days) * 0.7,
                    why: if meta.never_opened {
                        Why::NeverOpened
                    } else {
                        Why::UnusedApp
                    },
                    default_off: false,
                });
            }
            // An orphan (one removed app's folders) goes whole.
            Category::Apps if n.orphan && n.kind == NodeKind::Dir && n.depth >= 3 => {
                let parts: Vec<&UiNode> = n
                    .children
                    .iter()
                    .map(|&c| &nodes[c as usize])
                    .filter(|c| allowed(c))
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                let bytes: u64 = parts.iter().map(|p| p.size).sum();
                push(Candidate {
                    id: n.id,
                    parts: parts.iter().map(|p| p.id).collect(),
                    bytes,
                    last_used_days: n.last_opened_days,
                    safety: Safety::YourCall,
                    score: bytes as f64 * unused_factor(n.last_opened_days) * 0.7,
                    why: Why::AppRemoved,
                    default_off: false,
                });
            }
            Category::Apps => descend(&mut stack),
            // The Library: only what a junk rule covers (vetted, with "Why is this safe?").
            Category::Junk => {
                if n.rule_id.is_some() && allowed(n) {
                    push(single(
                        n,
                        if n.safety == Safety::Safe {
                            Why::Rebuilds
                        } else {
                            Why::Unused
                        },
                    ));
                } else {
                    descend(&mut stack);
                }
            }
            // Developer: rebuildable caches whole, and stale envs or archives. Docker,
            // Homebrew and the Wrasse checkout are shown and never offered. A tool's own
            // cleanup (brew, docker, simulators) is offered as one action.
            Category::Developer => {
                if n.read_only || crate::devjunk::wrasse_path(&n.path) {
                    continue;
                }
                if n.delegate
                    && n.delegate_action.is_some()
                    && n.size > 0
                    && attached(n)
                    && !under(&n.path, &limits.hand_pick_only)
                    && !under(&n.path, &limits.never_touch)
                {
                    push(single(n, Why::Unused));
                    continue;
                }
                match crate::devjunk::rank_why(
                    n.rebuildable,
                    n.dev_kind.as_deref(),
                    n.last_opened_days,
                ) {
                    Some(crate::devjunk::RankWhy::Rebuilds) if allowed(n) => {
                        push(single(n, Why::Rebuilds));
                    }
                    Some(crate::devjunk::RankWhy::Unused) if allowed(n) => {
                        push(single(n, Why::Unused));
                    }
                    // Hand-pick and other project items stay visible and are not opened up.
                    Some(_) => {}
                    None if n.dev_kind.is_some() => {}
                    None => descend(&mut stack),
                }
            }
            // AI models, whole.
            Category::AiModels => {
                // A clone shares its blocks: removing it frees nothing, so it is not offered.
                // An iCloud duplicate pair stays unticked on both copies.
                let clone = n.model.as_ref().is_some_and(|m| m.clone_copy);
                if icloud_dupes.contains(&n.id) {
                    continue;
                }
                if n.kind == NodeKind::Model && allowed(n) && !clone {
                    push(single(
                        n,
                        if n.size >= 1_000_000_000 && n.last_opened_days >= 180 {
                            Why::LargeAndOld
                        } else {
                            Why::Unused
                        },
                    ));
                } else {
                    descend(&mut stack);
                }
            }
            // Your Files: single items as shown.
            Category::Files => {
                if n.children.is_empty() {
                    if allowed(n) {
                        let why = if n.size >= 1_000_000_000 && n.last_opened_days >= 180 {
                            Why::LargeAndOld
                        } else {
                            Why::Unused
                        };
                        push(single(n, why));
                    }
                } else {
                    descend(&mut stack);
                }
            }
            _ => descend(&mut stack),
        }
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
    (out, held_recent, held_running)
}

/// The best candidates until `target` bytes are reached.
pub fn plan(disk: &UiDisk, target: u64, limits: &Limits) -> Plan {
    let (all, held_recent, held_running) = candidates(disk, limits);
    let mut items = Vec::new();
    let mut total = 0u64;
    for c in all {
        if total >= target {
            break;
        }
        total += c.bytes;
        items.push(c);
    }
    Plan {
        target,
        items,
        total,
        shortfall: target.saturating_sub(total),
        held_recent,
        held_running,
    }
}

#[cfg(test)]
mod tests;
