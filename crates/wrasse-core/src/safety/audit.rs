//! The audit log: one JSON line per action at `<data>/logs/actions.log`, rotated at 10 MB
//! (PLAN.md §4.3 rule 12).

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ROTATE_AT: u64 = 10_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record<'a> {
    pub at: u64,
    /// "quarantine", "delete", "restore", "purge", "delegate", "skip".
    pub action: &'a str,
    /// The path, or for delegated actions the command.
    pub target: &'a str,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<&'a str>,
    pub dry_run: bool,
    /// "ok", or why it was skipped or failed.
    pub result: &'a str,
}

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(data_dir: &Path) -> Self {
        AuditLog {
            path: data_dir.join("logs").join("actions.log"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, mut record: Record) -> io::Result<()> {
        if record.at == 0 {
            record.at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        if fs::metadata(&self.path).is_ok_and(|m| m.len() > ROTATE_AT) {
            fs::rename(&self.path, self.path.with_extension("1.log"))?;
        }
        let mut line = serde_json::to_vec(&record).map_err(io::Error::other)?;
        line.push(b'\n');
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(&line)
    }

    /// The most recent records, newest first.
    pub fn recent(&self, limit: usize) -> Vec<serde_json::Value> {
        let text = fs::read_to_string(&self.path).unwrap_or_default();
        text.lines()
            .rev()
            .take(limit)
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}
