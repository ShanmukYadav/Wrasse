//! The user's "Never touch" list (PLAN.md §4.3 rule 11), enforced inside both entrypoints.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Exclusions {
    file: PathBuf,
    pub paths: Vec<PathBuf>,
}

impl Exclusions {
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join("exclusions.json");
        let paths = fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok())
            .unwrap_or_default();
        Exclusions { file, paths }
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.paths.iter().any(|p| path.starts_with(p))
    }

    pub fn add(&mut self, path: PathBuf) -> io::Result<()> {
        if !self.paths.contains(&path) {
            self.paths.push(path);
        }
        self.save()
    }

    pub fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.paths.retain(|p| p != path);
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(
            &self.file,
            serde_json::to_vec_pretty(&self.paths).map_err(io::Error::other)?,
        )
    }
}
