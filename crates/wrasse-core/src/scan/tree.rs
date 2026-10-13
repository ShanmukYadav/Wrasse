//! The scan result: a flat arena of folders and large files (PLAN.md §5.3).
//!
//! Files under [`SMALL_FILE`] bytes are not stored one by one; their bytes and count are added
//! to the parent folder (`small_size`, `small_count`). That keeps memory proportional to the
//! number of folders and large files, not to every file on disk.

/// Files below this size on disk are aggregated into their folder.
pub(crate) const SMALL_FILE: u64 = 1_000_000;

/// No node: the root's parent, a leaf's first child, the last sibling.
pub(crate) const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Dir = 0,
    File = 1,
}

/// Node flags (bitmask).
pub mod flag {
    /// The folder could not be read (permission denied); its size is unknown, not zero.
    pub(crate) const UNREADABLE: u8 = 1;
    /// iCloud "dataless" placeholder: 0 bytes stored locally, contents only in the cloud.
    pub(crate) const DATALESS: u8 = 2;
    /// APFS clone: some of its blocks may be shared with other files.
    pub(crate) const CLONE: u8 = 4;
    /// Another volume is mounted here; it is not part of this scan.
    pub(crate) const MOUNT: u8 = 8;
    /// A hard link already counted elsewhere (adds 0 bytes here).
    pub(crate) const HARDLINK_SEEN: u8 = 16;
}

#[derive(Debug, Clone)]
pub struct Node {
    pub parent: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    /// Index into [`Tree::names`].
    pub name: u32,
    /// Bytes on disk of this node and everything under it.
    pub size: u64,
    /// Bytes and count of files under [`SMALL_FILE`] directly in this folder.
    pub small_size: u64,
    pub small_count: u32,
    /// Files and folders under this node (including aggregated small files).
    pub items: u64,
    /// Most recent access (or modification) time under this node, in days since the Unix epoch.
    pub last_used_day: u32,
    /// Modification time of this file, in seconds since the Unix epoch. `0` for a folder: Large
    /// & Old compares a file's size and this time against the live file before moving it.
    pub mtime: u64,
    /// Bytes under this node that may be shared with APFS clones.
    pub clone_shared: u64,
    pub kind: Kind,
    pub flags: u8,
}

#[derive(Debug, Default, Clone)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub names: Vec<Box<str>>,
    /// Absolute path of node 0.
    pub root_path: String,
    pub stats: ScanStats,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScanStats {
    pub files: u64,
    pub dirs: u64,
    pub unreadable_dirs: u64,
    pub dataless_items: u64,
    pub hardlinks_deduped: u64,
    pub clone_shared_bytes: u64,
    pub cancelled: bool,
}

impl Tree {
    pub fn root(&self) -> u32 {
        0
    }

    pub fn name(&self, id: u32) -> &str {
        &self.names[self.nodes[id as usize].name as usize]
    }

    pub fn node(&self, id: u32) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn children(&self, id: u32) -> Children<'_> {
        Children {
            tree: self,
            next: self.nodes[id as usize].first_child,
        }
    }

    /// Absolute path of a node.
    pub fn path(&self, id: u32) -> String {
        let mut parts = Vec::new();
        let mut n = id;
        while n != 0 && n != NONE {
            parts.push(self.name(n));
            n = self.nodes[n as usize].parent;
        }
        let mut path = self.root_path.trim_end_matches('/').to_string();
        for p in parts.iter().rev() {
            path.push('/');
            path.push_str(p);
        }
        if path.is_empty() {
            path.push('/');
        }
        path
    }

    /// The node at an absolute path, if it was scanned (large files and folders only).
    pub fn find(&self, path: &str) -> Option<u32> {
        let root = self.root_path.trim_end_matches('/');
        let rest = path.strip_prefix(root)?;
        if !(rest.is_empty() || rest.starts_with('/')) {
            return None;
        }
        let mut id = 0;
        for part in rest.split('/').filter(|p| !p.is_empty()) {
            id = self.children(id).find(|&c| self.name(c) == part)?;
        }
        Some(id)
    }
}

pub struct Children<'a> {
    tree: &'a Tree,
    next: u32,
}

impl Iterator for Children<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.next == NONE {
            return None;
        }
        let id = self.next;
        self.next = self.tree.nodes[id as usize].next_sibling;
        Some(id)
    }
}
