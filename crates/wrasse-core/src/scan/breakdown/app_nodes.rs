//! Apps: claiming bundles and their leftovers, and the app and orphaned-data nodes.

use super::*;

impl Builder<'_> {
    /// The installed apps' bundles and leftovers, claimed so their bytes show under Apps.
    pub(super) fn claim_apps(&mut self) -> Vec<App> {
        let mut out = Vec::new();
        for info in self.input.apps {
            let Some(bundle) = self.claim(&info.bundle.to_string_lossy()) else {
                continue;
            };
            let leftovers = info
                .leftovers
                .iter()
                .filter(|l| !l.needs_admin)
                .filter_map(|l| {
                    self.claim(&l.path.to_string_lossy())
                        .map(|id| (id, l.clone()))
                })
                .collect();
            out.push(App {
                bundle,
                info: info.clone(),
                leftovers,
            });
        }
        out
    }

    /// Orphaned leftovers in the scan: (orphan, its items' arena nodes).
    pub(super) fn claim_orphans(
        &mut self,
    ) -> Vec<(crate::apps::Orphan, Vec<(u32, crate::apps::Leftover)>)> {
        let mut out = Vec::new();
        for o in self.input.orphans {
            let items: Vec<(u32, crate::apps::Leftover)> = o
                .items
                .iter()
                .filter_map(|l| {
                    self.claim(&l.path.to_string_lossy())
                        .map(|id| (id, l.clone()))
                })
                .collect();
            if !items.is_empty() {
                out.push((o.clone(), items));
            }
        }
        out
    }

    pub(super) fn emit_app(&mut self, app: App, parent: u32) {
        use crate::apps::{uninstall::route, Source};
        let info = &app.info;
        let bundle_path = self.tree.path(app.bundle);
        let display = self.display(&bundle_path);
        let running = self.input.running.iter().any(|p| {
            crate::platform::is_app_process(Path::new(&bundle_path), p)
                || crate::platform::is_app_process(&info.bundle, p)
        });
        let system = info.source == Source::System;
        let a = self.virtual_node(
            &info.name,
            &display,
            Some(parent),
            Category::Apps,
            NodeKind::App,
            if system {
                Safety::Keep
            } else {
                Safety::YourCall
            },
            if system {
                "Part of macOS. It can't be removed."
            } else {
                "An app and the data it keeps in your Library."
            },
        );
        let last_day = self.tree.node(app.bundle).last_used_day;
        let n = &mut self.nodes[a as usize];
        n.running = running;
        n.last_opened_days = info
            .last_used_days
            .unwrap_or_else(|| self.input.today.saturating_sub(last_day));
        n.app = Some(AppMeta {
            source: info.source.clone(),
            owner: info.owner,
            bundle_id: info.id.clone(),
            version: info.version.clone(),
            never_opened: info.never_opened(),
            last_used_known: info.last_used_days.is_some(),
            route: route(info),
            admin_leftovers: info
                .leftovers
                .iter()
                .filter(|l| l.needs_admin)
                .map(|l| l.path.to_string_lossy().into_owned())
                .collect(),
        });
        // The bundle itself: one item, not a folder to open.
        let bctx = Ctx {
            category: Category::Apps,
            safety: if system {
                Safety::Keep
            } else {
                Safety::YourCall
            },
            rebuildable: false,
            leftover: None,
        };
        let last_used = self.nodes[a as usize].last_opened_days;
        let b = self.leaf_from(app.bundle, a, bctx, NodeKind::Dir);
        self.nodes[b as usize].app_bundle = true;
        // The app's parts were last used when the app was (PLAN.md §6.5: caches use their app's).
        self.nodes[b as usize].last_opened_days = last_used;
        self.nodes[b as usize].why = format!("The {} app itself.", info.name);
        if system {
            self.nodes[b as usize].safety = Safety::Keep;
        }
        for (id, l) in app.leftovers {
            let ctx = Ctx {
                category: Category::Apps,
                safety: if l.kind.regenerates() {
                    Safety::Safe
                } else {
                    Safety::YourCall
                },
                rebuildable: l.kind == Leftover::Caches,
                leftover: Some(l.kind),
            };
            let n = self.leaf_from(id, a, ctx, NodeKind::Dir);
            let node = &mut self.nodes[n as usize];
            node.name = l.kind.label().into();
            node.why = leftover_why(&l, &info.name);
            node.last_opened_days = last_used;
            node.leftover_meta = Some(LeftoverMeta {
                matched_by: l.matched_by,
                shared_with: l.shared_with.clone(),
                default_off: !l.ticked_by_default(),
            });
        }
    }

    /// "Leftovers from removed apps": one node per bundle id, its folders as items.
    pub(super) fn emit_orphans(
        &mut self,
        orphans: Vec<(crate::apps::Orphan, Vec<(u32, crate::apps::Leftover)>)>,
        parent: u32,
    ) {
        if orphans.is_empty() {
            return;
        }
        let home = self
            .input
            .home
            .as_ref()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default();
        let group = self.virtual_node(
            "Leftovers from removed apps",
            &format!("{home}/Library"),
            Some(parent),
            Category::Apps,
            NodeKind::Dir,
            Safety::YourCall,
            "Data apps left behind after they were removed. No installed app uses these folders.",
        );
        self.nodes[group as usize].orphan = true;
        for (o, items) in orphans {
            let first = self.display(&self.tree.path(items[0].0));
            let node = self.virtual_node(
                &o.id,
                &first,
                Some(group),
                Category::Apps,
                NodeKind::Dir,
                Safety::YourCall,
                &format!(
                    "Left by an app with the id {}, which isn't installed on this Mac. Nothing here changed in {} days.",
                    o.id, o.untouched_days
                ),
            );
            self.nodes[node as usize].orphan = true;
            self.nodes[node as usize].last_opened_days = o.untouched_days;
            for (id, l) in items {
                let ctx = Ctx {
                    category: Category::Apps,
                    safety: Safety::YourCall,
                    rebuildable: false,
                    leftover: Some(l.kind),
                };
                let n = self.leaf_from(id, node, ctx, NodeKind::Dir);
                let item = &mut self.nodes[n as usize];
                item.orphan = true;
                item.name = l.kind.label().into();
                item.why = format!(
                    "{} of an app that's no longer installed ({}).",
                    l.kind.label(),
                    o.id
                );
                item.last_opened_days = o.untouched_days;
                item.leftover_meta = Some(LeftoverMeta {
                    matched_by: l.matched_by,
                    shared_with: Vec::new(),
                    default_off: true,
                });
            }
        }
    }
}
