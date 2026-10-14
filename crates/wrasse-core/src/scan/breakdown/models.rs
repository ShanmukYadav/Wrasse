//! AI models: finding model folders and weights, Hugging Face and Ollama layouts, duplicate copies.

use super::*;

impl Builder<'_> {
    /// A Hugging Face hub: a folder holding `models--org--name` folders.
    pub(super) fn is_hf_hub(&self, id: u32) -> bool {
        self.tree
            .children(id)
            .any(|c| self.tree.name(c).starts_with("models--"))
    }

    /// The hub inside a folder: the folder itself, or its `hub/` child (`~/.cache/huggingface`).
    pub(super) fn find_hub(&self, id: u32) -> Option<u32> {
        if self.is_hf_hub(id) {
            return Some(id);
        }
        self.tree
            .children(id)
            .find(|&c| self.tree.name(c) == "hub" && self.is_hf_hub(c))
    }

    /// Walks the scan for model data outside the known tool folders and claims it for AI Models.
    pub(super) fn discover_models(&mut self, home: &str) -> Vec<Found> {
        let mut hits: Vec<(u32, FoundItem)> = Vec::new();
        let mut stack = vec![self.tree.root()];
        while let Some(id) = stack.pop() {
            if self.carved.contains(&id) {
                continue;
            }
            let node = self.tree.node(id);
            let name = self.tree.name(id);
            if node.kind == Kind::File {
                if is_weights_file(&self.tree.path(id), name, node.size) {
                    hits.push((id, FoundItem::Weights(id)));
                }
                continue;
            }
            if name.ends_with(".app") {
                continue;
            }
            if self.is_hf_hub(id) {
                hits.push((id, FoundItem::Hub(id)));
                continue;
            }
            let has = |n: &str| self.tree.children(id).any(|c| self.tree.name(c) == n);
            if name == "models" && has("blobs") && has("manifests") {
                hits.push((id, FoundItem::Ollama(id)));
                continue;
            }
            stack.extend(self.tree.children(id));
        }
        let mut out = Vec::new();
        for (id, item) in hits {
            let display = self.display(&self.tree.path(id));
            if self.claim_id(id).is_none() {
                continue;
            }
            let (group, group_path) = model_group(&display, home);
            out.push(Found {
                group,
                group_path,
                item,
            });
        }
        out.sort_by(|a, b| a.group.cmp(&b.group));
        out
    }

    /// A Hugging Face cache as one tile per model, sized by the blobs its snapshots link to.
    /// `project` is a cache inside Documents, Desktop, or iCloud: project files, not junk.
    pub(super) fn emit_hf(
        &mut self,
        label: &str,
        src: u32,
        hub: u32,
        parent: u32,
        project: bool,
    ) -> u32 {
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let last = self
            .input
            .today
            .saturating_sub(self.tree.node(src).last_used_day);
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models downloaded from Hugging Face.",
        );
        let models = (self.input.hf_models)(Path::new(&self.tree.path(hub)));
        let mut accounted = 0u64;
        for m in models {
            let shares = m.bytes == 0 && m.logical_bytes >= MIN_ITEM;
            let bytes = if shares {
                m.logical_bytes
            } else {
                m.bytes.min(size.saturating_sub(accounted))
            };
            if !shares && bytes < MIN_ITEM {
                continue;
            }
            if !shares {
                accounted += bytes;
            }
            let name = m.repo.rsplit('/').next().unwrap_or(&m.repo).to_string();
            let mp = self.display(&m.dir.to_string_lossy());
            let why = if let Some(other) = &m.shares_with {
                format!("{name} from Hugging Face. Weights are already counted with {other}.")
            } else if project {
                format!("{name} is part of this project. It stays until you choose it.")
            } else {
                format!("{name} from Hugging Face. Downloads again if a project needs it.")
            };
            let id = self.virtual_node(
                &name,
                &mp,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &why,
            );
            let weight = first_weight(&m.dir);
            let quant = crate::llm::quant_of(&m.repo, weight.as_deref())
                .or_else(|| crate::llm::quant_from_config(&m.dir));
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = last;
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool: crate::llm::ModelTool::HuggingFace,
                quantization: quant,
                removal: crate::llm::ModelRemoval::HuggingFace,
                ollama_name: None,
                models_dir: None,
                weight: weight.as_ref().map(|p| self.display(&p.to_string_lossy())),
                partner_weight: None,
                dupe_level: None,
                content_hash: weight
                    .as_ref()
                    .and_then(|p| crate::llm::blob_content_hash(p)),
                orphan_blob: false,
                clone_copy: false,
                shares_weights: shares,
                shares_with: m.shares_with.clone(),
            });
            if let Some(w) = &weight {
                self.refine_used(id, w);
                self.mark_clone(id, w);
            } else {
                self.refine_used(id, &m.dir);
            }
        }
        self.fold_remainder(node, size, accounted, last);
        node
    }

    /// Ollama, one row per manifest plus orphaned blobs. `false` when this folder isn't a store.
    pub(super) fn emit_ollama(&mut self, label: &str, src: u32, parent: u32) -> bool {
        let arena = PathBuf::from(self.tree.path(src));
        let Some(store) = crate::llm::Store::open(&arena) else {
            return false;
        };
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models downloaded with Ollama.",
        );
        let mut accounted = 0u64;
        let dir_display = self.display(&store.dir.to_string_lossy());
        for m in &store.models {
            let shares = m.shares_weights && m.logical >= MIN_ITEM;
            let own = !m.shares_weights && m.exclusive >= MIN_ITEM;
            if !shares && !own {
                continue;
            }
            let bytes = if shares { m.logical } else { m.exclusive };
            let weight = m
                .weight
                .as_ref()
                .map(|p| self.display(&p.to_string_lossy()));
            let manifest = m
                .manifest
                .as_ref()
                .map(|p| self.display(&p.to_string_lossy()));
            let node_path = manifest
                .clone()
                .or(weight.clone())
                .unwrap_or_else(|| path.clone());
            let why = if m.orphan {
                "An Ollama blob no manifest uses.".to_string()
            } else if let Some(other) = &m.shares_with {
                format!("Weights are already counted with {other}.")
            } else {
                format!("{} in Ollama.", m.name)
            };
            let id = self.virtual_node(
                &m.name,
                &node_path,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &why,
            );
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = self.input.today.saturating_sub(m.last_day);
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool: crate::llm::ModelTool::Ollama,
                quantization: m.quantization.clone(),
                removal: if m.orphan {
                    crate::llm::ModelRemoval::Quarantine
                } else {
                    crate::llm::ModelRemoval::Ollama
                },
                ollama_name: (!m.orphan).then(|| m.name.clone()),
                models_dir: Some(dir_display.clone()),
                weight: weight.clone(),
                partner_weight: None,
                dupe_level: None,
                content_hash: m.content_hash.clone(),
                orphan_blob: m.orphan,
                clone_copy: false,
                shares_weights: shares,
                shares_with: m.shares_with.clone(),
            });
            let stamp = m.weight.clone().or_else(|| m.manifest.clone());
            if let Some(p) = stamp {
                self.refine_used(id, &p);
                self.mark_clone(id, &p);
            }
            if !shares {
                accounted += m.exclusive;
            }
        }
        let last = self.nodes[node as usize].last_opened_days;
        self.fold_remainder(node, size, accounted, last);
        true
    }

    /// Weight files inside a known tool folder (LM Studio, Jan, GPT4All). `false` when there
    /// are none big enough to list, so the folder is shown as one item instead.
    pub(super) fn emit_tool_models(&mut self, label: &str, src: u32, parent: u32) -> bool {
        let mut files = Vec::new();
        self.collect_weights(src, &mut files, 0);
        if files.is_empty() {
            return false;
        }
        let size = self.size(src);
        let path = self.display(&self.tree.path(src));
        let node = self.virtual_node(
            label,
            &path,
            Some(parent),
            Category::AiModels,
            NodeKind::Dir,
            Safety::YourCall,
            "Models kept by this tool.",
        );
        let tool = tool_of(label);
        let removal = removal_of(tool);
        let mut accounted = 0u64;
        for aid in files {
            let bytes = self.size(aid);
            if bytes < MIN_ITEM {
                continue;
            }
            let parent_name = self.tree.name(self.tree.node(aid).parent);
            let file = self.tree.name(aid);
            let title = if parent_name == self.tree.name(src) {
                file.to_string()
            } else {
                format!("{parent_name} · {file}")
            };
            let display = self.display(&self.tree.path(aid));
            let id = self.virtual_node(
                &title,
                &display,
                Some(node),
                Category::AiModels,
                NodeKind::Model,
                Safety::YourCall,
                &format!("{title} in {label}."),
            );
            let arena = PathBuf::from(self.tree.path(aid));
            let day = self.tree.node(aid).last_used_day;
            self.nodes[id as usize].size = bytes;
            self.nodes[id as usize].last_opened_days = self.input.today.saturating_sub(day);
            self.nodes[id as usize].approx = true;
            self.nodes[id as usize].model = Some(crate::llm::ModelMeta {
                tool,
                quantization: crate::llm::quant_of(&title, Some(&arena)),
                removal,
                ollama_name: None,
                models_dir: None,
                weight: Some(display),
                partner_weight: None,
                dupe_level: None,
                content_hash: crate::llm::blob_content_hash(&arena),
                orphan_blob: false,
                clone_copy: false,
                shares_weights: false,
                shares_with: None,
            });
            self.refine_used(id, &arena);
            self.mark_clone(id, &arena);
            accounted += bytes;
        }
        let last = self
            .input
            .today
            .saturating_sub(self.tree.node(src).last_used_day);
        self.fold_remainder(node, size, accounted, last);
        true
    }

    pub(super) fn collect_weights(&self, id: u32, out: &mut Vec<u32>, depth: u32) {
        if depth > 8 {
            return;
        }
        for c in self.tree.children(id) {
            let node = self.tree.node(c);
            let name = self.tree.name(c);
            if name.ends_with(".app") {
                continue;
            }
            if node.kind == Kind::File {
                if node.size >= MIN_ITEM
                    && crate::scan::weights::is_model_weights(Path::new(&self.tree.path(c)))
                {
                    out.push(c);
                }
            } else {
                self.collect_weights(c, out, depth + 1);
            }
        }
    }

    pub(super) fn attach_loose(&mut self, ui: u32, aid: u32) {
        let arena = PathBuf::from(self.tree.path(aid));
        let name = self.nodes[ui as usize].name.clone();
        let display = self.nodes[ui as usize].path.clone();
        self.nodes[ui as usize].model = Some(crate::llm::ModelMeta {
            tool: crate::llm::ModelTool::Loose,
            quantization: crate::llm::quant_of(&name, Some(&arena)),
            removal: crate::llm::ModelRemoval::Quarantine,
            ollama_name: None,
            models_dir: None,
            weight: Some(display),
            partner_weight: None,
            dupe_level: None,
            content_hash: crate::llm::blob_content_hash(&arena),
            orphan_blob: false,
            clone_copy: false,
            shares_weights: false,
            shares_with: None,
        });
        self.refine_used(ui, &arena);
        self.mark_clone(ui, &arena);
    }

    /// Size, then a sample of the first and last 16 MB. Dataless files are not opened.
    pub(super) fn attach_dupes(&mut self) {
        let mut idxs = Vec::new();
        let mut cands = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(weight) = n.model.as_ref().and_then(|m| m.weight.clone()) else {
                continue;
            };
            let arena = self.arena_path(&weight);
            let path = PathBuf::from(&arena);
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let dataless = crate::platform::is_dataless(&md);
            let len = if dataless {
                0
            } else if md.file_type().is_symlink() {
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            } else {
                md.len()
            };
            let hash = n.model.as_ref().and_then(|m| m.content_hash.clone());
            cands.push(crate::llm::DupeCand {
                path,
                len,
                content_hash: hash,
                dataless,
            });
            idxs.push(i);
        }
        for (k, cand) in cands.iter().enumerate() {
            if !cand.dataless {
                self.mark_clone(idxs[k] as u32, &cand.path);
            }
        }
        // Two seconds of edge reads. Longer than that, the copies stay unlabeled and the
        // scan ends; removing one still requires a full hash. Tests have no deadline: on a busy
        // CI machine two seconds can pass before the fixture's copies are read.
        let deadline =
            (!cfg!(test)).then(|| std::time::Instant::now() + std::time::Duration::from_secs(2));
        let pairs = crate::llm::pair_duplicates_until(&cands, deadline);
        for (k, pair) in pairs.into_iter().enumerate() {
            let Some(pair) = pair else { continue };
            let display = self.display(&pair.partner.to_string_lossy());
            let id = idxs[k];
            if let Some(m) = self.nodes[id].model.as_mut() {
                m.partner_weight = Some(display.clone());
                m.dupe_level = Some(pair.level);
            }
            if pair.level == crate::llm::DupeLevel::Full {
                if let Some(pid) = self.nodes.iter().position(|n| {
                    n.model.as_ref().and_then(|m| m.weight.as_deref()) == Some(display.as_str())
                }) {
                    self.nodes[id].duplicate_of = Some(pid as u32);
                }
            } else if self.nodes[id].note.is_none() {
                self.nodes[id].note = Some(crate::llm::SAMPLE_NOTE.into());
            }
        }
        self.note_icloud_copies();
        for &i in &idxs {
            if self.nodes[i].model.as_ref().is_some_and(|m| m.clone_copy) {
                self.nodes[i].note = Some(crate::llm::CLONE_NOTE.into());
            }
        }
        self.note_two_formats();
    }

    /// An iCloud Drive copy that matches another model. The warning is on the synced copy.
    pub(super) fn note_icloud_copies(&mut self) {
        let mut partners = Vec::new();
        let mut tagged = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(m) = &n.model else { continue };
            if m.dupe_level.is_none() && n.duplicate_of.is_none() {
                continue;
            }
            let own = m.weight.clone().unwrap_or_else(|| n.path.clone());
            let partner = m.partner_weight.clone().unwrap_or_default();
            let touches = crate::llm::icloud_synced_path(&own)
                || crate::llm::icloud_synced_path(&n.path)
                || crate::llm::icloud_synced_path(&partner);
            if !touches {
                continue;
            }
            if !partner.is_empty() {
                partners.push(partner);
            }
            if crate::llm::icloud_synced_path(&own) || crate::llm::icloud_synced_path(&n.path) {
                tagged.push(i);
            }
        }
        for (i, n) in self.nodes.iter().enumerate() {
            let own = n
                .model
                .as_ref()
                .and_then(|m| m.weight.clone())
                .unwrap_or_else(|| n.path.clone());
            let synced =
                crate::llm::icloud_synced_path(&own) || crate::llm::icloud_synced_path(&n.path);
            if synced && partners.iter().any(|p| p == &own || p == &n.path) {
                tagged.push(i);
            }
        }
        tagged.sort_unstable();
        tagged.dedup();
        for i in tagged {
            add_note(&mut self.nodes[i], crate::llm::ICLOUD_DUPE_NOTE);
        }
    }

    /// The same checkpoint saved as pytorch_model.bin and model.safetensors. Headers only.
    pub(super) fn note_two_formats(&mut self) {
        let mut groups: Vec<(std::path::PathBuf, Vec<usize>)> = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            let Some(weight) = n.model.as_ref().and_then(|m| m.weight.clone()) else {
                continue;
            };
            let arena = std::path::PathBuf::from(self.arena_path(&weight));
            let Some(dir) = arena.parent().map(|p| p.to_path_buf()) else {
                continue;
            };
            if let Some((_, ids)) = groups.iter_mut().find(|(d, _)| d == &dir) {
                ids.push(i);
            } else {
                groups.push((dir, vec![i]));
            }
        }
        for (dir, ids) in groups {
            if !crate::scan::weights::same_weights_two_formats(&dir) {
                continue;
            }
            for i in ids {
                add_note(&mut self.nodes[i], crate::llm::FORMAT_NOTE);
            }
        }
    }
}
