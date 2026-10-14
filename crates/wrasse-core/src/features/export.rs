//! Report export (PLAN.md §6.14): a table from the app (the scan summary, a Free Up Space plan,
//! Large & Old, History) written as Markdown, CSV or JSON to the Downloads folder. Writes a new
//! file only; never overwrites one ("… 2.md" when the name is taken).

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Markdown,
    Csv,
    Json,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Markdown => "md",
            Format::Csv => "csv",
            Format::Json => "json",
        }
    }
}

/// What to export: a title, column names, and rows of cells (already formatted by the app).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// The table in the given format.
pub fn render(table: &Table, format: Format) -> String {
    match format {
        Format::Markdown => markdown(table),
        Format::Csv => csv(table),
        Format::Json => json(table),
    }
}

fn markdown(t: &Table) -> String {
    let cell = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut out = format!("# {}\n\n", cell(&t.title));
    out.push_str(&format!(
        "| {} |\n",
        t.columns
            .iter()
            .map(|c| cell(c))
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    out.push_str(&format!("|{}\n", " --- |".repeat(t.columns.len())));
    for row in &t.rows {
        out.push_str(&format!(
            "| {} |\n",
            row.iter().map(|c| cell(c)).collect::<Vec<_>>().join(" | ")
        ));
    }
    out
}

fn csv(t: &Table) -> String {
    // RFC 4180: quote fields with a comma, quote or line break; double the quotes. A leading
    // = + - @ is prefixed with ' so a spreadsheet doesn't run it as a formula.
    let field = |s: &str| {
        let s = if s.starts_with(['=', '+', '-', '@']) {
            format!("'{s}")
        } else {
            s.to_string()
        };
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    };
    let line = |cells: &[String]| cells.iter().map(|c| field(c)).collect::<Vec<_>>().join(",");
    let mut out = line(&t.columns);
    out.push_str("\r\n");
    for row in &t.rows {
        out.push_str(&line(row));
        out.push_str("\r\n");
    }
    out
}

fn json(t: &Table) -> String {
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = t
        .rows
        .iter()
        .map(|row| {
            t.columns
                .iter()
                .zip(row)
                .map(|(c, v)| (c.clone(), serde_json::Value::String(v.clone())))
                .collect()
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "title": t.title, "rows": rows }))
        .unwrap_or_default()
}

/// Writes the table to `dir` as "<title> <date>.<ext>", picking a free name. Returns the path.
pub fn write(dir: &Path, table: &Table, format: Format, date: &str) -> io::Result<PathBuf> {
    let base: String = format!("{} {date}", table.title)
        .chars()
        .map(|c| {
            if matches!(c, '/' | ':' | '\\') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let ext = format.extension();
    let mut path = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} {n}.{ext}"));
        n += 1;
    }
    // create_new: never replaces a file that appeared meanwhile.
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    f.write_all(render(table, format).as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table {
            title: "Large & Old".into(),
            columns: vec!["Name".into(), "Size".into(), "Path".into()],
            rows: vec![
                vec![
                    "a, b.mov".into(),
                    "4.1 GB".into(),
                    "~/Movies/a, b.mov".into(),
                ],
                vec!["=cmd|x".into(), "1 GB".into(), "say \"hi\"".into()],
            ],
        }
    }

    #[test]
    fn csv_quotes_and_defuses_formulas() {
        let out = render(&table(), Format::Csv);
        assert_eq!(
            out,
            "Name,Size,Path\r\n\"a, b.mov\",4.1 GB,\"~/Movies/a, b.mov\"\r\n'=cmd|x,1 GB,\"say \"\"hi\"\"\"\r\n"
        );
    }

    #[test]
    fn markdown_escapes_pipes() {
        let out = render(&table(), Format::Markdown);
        assert!(out.starts_with("# Large & Old\n\n| Name | Size | Path |\n| --- | --- | --- |\n"));
        assert!(out.contains("| =cmd\\|x | 1 GB |"));
    }

    #[test]
    fn json_has_named_fields() {
        let v: serde_json::Value = serde_json::from_str(&render(&table(), Format::Json)).unwrap();
        assert_eq!(v["rows"][0]["Size"], "4.1 GB");
        assert_eq!(v["title"], "Large & Old");
    }

    #[test]
    fn write_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("wrasse-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = write(&dir, &table(), Format::Csv, "2026-10-14").unwrap();
        let b = write(&dir, &table(), Format::Csv, "2026-10-14").unwrap();
        assert_eq!(a.file_name().unwrap(), "Large & Old 2026-10-14.csv");
        assert_eq!(b.file_name().unwrap(), "Large & Old 2026-10-14 2.csv");
        crate::platform::remove_test_dir(&dir);
    }
}
