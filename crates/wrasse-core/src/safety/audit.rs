//! The audit log: one JSON line per action at `<data>/logs/actions.log`, rotated at 10 MB
//! (PLAN.md §4.3 rule 12).

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const ROTATE_AT: u64 = 10_000_000;

/// What started an action, written on every audit line so the log says who did it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// A click in the app.
    Ui,
    /// The `wrasse` command line.
    Cli,
    /// A script driving the command line (it sets `WRASSE_ORIGIN=script`).
    Script,
    /// An automated test.
    Test,
    /// Not set by the program that wrote the line.
    Unknown,
}

static ORIGIN: OnceLock<Origin> = OnceLock::new();

/// Sets the origin for this process, once, at startup (the app: `Ui`; the CLI: `Cli` or `Script`).
pub fn set_origin(origin: Origin) {
    let _ = ORIGIN.set(origin);
}

/// This process's origin. Tests are always `Test`, whatever was set.
pub fn origin() -> Origin {
    if cfg!(test) {
        return Origin::Test;
    }
    ORIGIN.get().copied().unwrap_or(Origin::Unknown)
}

/// One line as written: the record plus who started it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Line<'a> {
    #[serde(flatten)]
    record: Record<'a>,
    origin: Origin,
}

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
        let mut line = serde_json::to_vec(&Line {
            record,
            origin: origin(),
        })
        .map_err(io::Error::other)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_says_who_started_it() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wrasse-audit-origin-{nanos}"));
        let log = AuditLog::new(&dir);
        set_origin(Origin::Ui); // ignored in tests: they always write "test"
        log.write(Record {
            at: 0,
            action: "quarantine",
            target: "/tmp/x",
            bytes: 1,
            rule_id: None,
            dry_run: true,
            result: "ok",
        })
        .unwrap();
        let line = &log.recent(1)[0];
        assert_eq!(line["origin"], "test");
        assert_eq!(line["action"], "quarantine");
        crate::platform::remove_test_dir(&dir);
    }
}
