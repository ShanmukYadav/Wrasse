//! Developer projects: finding them and applying each tool's cleanup policy.

use super::*;

impl Builder<'_> {
    /// Stale project folders and Python envs, claimed so they show under Developer.
    /// Anything inside the Wrasse checkout (it contains `.toolchain`) stays put.
    pub(super) fn discover_projects(&mut self) -> Vec<(u32, crate::devjunk::ProjectItem, String)> {
        struct Frame {
            id: u32,
            in_modules: bool,
            in_wrasse: bool,
        }
        let mut stack = vec![Frame {
            id: self.tree.root(),
            in_modules: false,
            in_wrasse: false,
        }];
        let mut hits = Vec::new();
        while let Some(frame) = stack.pop() {
            if self.carved.contains(&frame.id) || self.inside_carved(frame.id) {
                continue;
            }
            let node = self.tree.node(frame.id);
            if node.kind == Kind::File {
                continue;
            }
            let name = self.tree.name(frame.id);
            let child_names: Vec<&str> = self
                .tree
                .children(frame.id)
                .map(|c| self.tree.name(c))
                .collect();
            let wrasse_here = frame.in_wrasse || crate::devjunk::is_wrasse_root(&child_names);
            let parent = node.parent;
            let parent_name = if parent == crate::scan::tree::NONE {
                ""
            } else {
                self.tree.name(parent)
            };
            let grand = if parent == crate::scan::tree::NONE {
                crate::scan::tree::NONE
            } else {
                self.tree.node(parent).parent
            };
            let grand_name = if grand == crate::scan::tree::NONE {
                ""
            } else {
                self.tree.name(grand)
            };
            let parent_children: Vec<&str> = if parent == crate::scan::tree::NONE {
                Vec::new()
            } else {
                self.tree
                    .children(parent)
                    .map(|c| self.tree.name(c))
                    .collect()
            };
            let age = self.input.today.saturating_sub(node.last_used_day);
            // Small files are not nodes in the tree. A `pyvenv.cfg` or `Cargo.toml` is
            // almost always smaller than that, so look the name up on disk.
            let mut children = child_names.clone();
            if matches!(name, "venv" | ".venv" | "env")
                && has_dir_entry(&self.tree.path(frame.id), "pyvenv.cfg")
                && !children.contains(&"pyvenv.cfg")
            {
                children.push("pyvenv.cfg");
            }
            let mut parent_kids = parent_children.clone();
            if name == "target"
                && parent != crate::scan::tree::NONE
                && has_dir_entry(&self.tree.path(parent), "Cargo.toml")
                && !parent_kids.contains(&"Cargo.toml")
            {
                parent_kids.push("Cargo.toml");
            }
            if !wrasse_here {
                if let Some(kind) = crate::devjunk::project_item(&crate::devjunk::ProjectSpot {
                    name,
                    children: &children,
                    parent_children: &parent_kids,
                    parent_name,
                    grandparent_name: grand_name,
                    age_days: age,
                    in_modules: frame.in_modules,
                    in_wrasse: false,
                }) {
                    if self.size(frame.id) >= MIN_ITEM {
                        if let Some(id) = self.claim_id(frame.id) {
                            let project = match kind {
                                crate::devjunk::ProjectItem::Conda => name.to_string(),
                                _ => {
                                    if parent_name.is_empty() {
                                        "Project".into()
                                    } else {
                                        parent_name.to_string()
                                    }
                                }
                            };
                            hits.push((id, kind, project));
                            continue;
                        }
                    }
                }
            }
            let in_modules = frame.in_modules || name == "node_modules";
            for c in self.tree.children(frame.id) {
                stack.push(Frame {
                    id: c,
                    in_modules,
                    in_wrasse: wrasse_here,
                });
            }
        }
        hits
    }

    /// Applies developer policy once folder ages include their newest child.
    pub(super) fn apply_dev_policy(&mut self, dev: u32) {
        let mut stack = vec![dev];
        while let Some(id) = stack.pop() {
            let kids = self.nodes[id as usize].children.clone();
            stack.extend(kids);
            if id == dev {
                continue;
            }
            let path = self.nodes[id as usize].path.clone();
            let age = self.nodes[id as usize].last_opened_days;
            let Some(policy) = crate::devjunk::policy_for(&path, age) else {
                continue;
            };
            let n = &mut self.nodes[id as usize];
            n.safety = policy.safety;
            n.rebuildable = policy.rebuildable;
            n.read_only = policy.read_only;
            n.dev_kind = policy.dev_kind.map(str::to_string);
            n.why = policy.why;
        }
    }
}
