//! Junk rules (PLAN.md §6.2): TOML files that say what can be cleaned and why.
//!
//! Bundled rules come from `rules/<os>/*.toml` (compiled in); users may add their own in
//! `<data>/rules/custom/*.toml`. Every rule is validated before use, and a rule that fails is
//! rejected on its own (the others still load):
//! - `schema_version` must be 1;
//! - roots are absolute (after `~` expansion), have no `..`, may not be or sit inside a protected
//!   location, and may not contain one; a root that is a link is judged by where it points;
//! - include/exclude patterns match names directly inside a root (`**` = everything), so they
//!   can't reach outside it: no `/`, `\`, `..` or `~`;
//! - custom rules can never be `safe` (forced to "your call") and can't delegate;
//! - `safety = "keep"` is meaningless for a cleanup rule and is rejected.
//!
//! Matching lists the entries directly inside each root (whole folders, not every file), skips
//! entries another rule handles more specifically, honors `min_age_days`, and holds back rules
//! whose app (`requires_app_closed`) is running. Removal still goes through `safety`.

use crate::platform::{Protected, Protection};
use crate::safety::{Delegated, Safety};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;

/// The bundled pack for this OS.
pub fn bundled_text() -> &'static str {
    #[cfg(target_os = "macos")]
    return include_str!("../../../../rules/macos/junk.toml");
    #[allow(unreachable_code)]
    ""
}

fn this_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "macos"
    }
}

#[derive(Deserialize)]
struct RuleFile {
    schema_version: u32,
    #[serde(default)]
    rule: Vec<RawRule>,
}

#[derive(Deserialize)]
struct RawRule {
    id: String,
    name: String,
    platform: String,
    category: String,
    roots: Vec<String>,
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    safety: String,
    #[serde(default)]
    regenerates: bool,
    #[serde(default)]
    min_age_days: u64,
    #[serde(default)]
    requires_app_closed: Option<String>,
    explanation: String,
    #[serde(default)]
    delegate: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: String,
    pub roots: Vec<PathBuf>,
    include: GlobSet,
    exclude: GlobSet,
    pub safety: Safety,
    pub regenerates: bool,
    pub min_age_days: u64,
    pub requires_app_closed: Option<String>,
    pub explanation: String,
    pub custom: bool,
    pub delegate: Option<Delegated>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleError {
    /// The rule id, or "" for file-level errors.
    pub id: String,
    pub why: String,
}

fn err(id: &str, why: impl Into<String>) -> RuleError {
    RuleError {
        id: id.to_string(),
        why: why.into(),
    }
}

/// Parses and validates one rule file. Valid rules load; each invalid one is reported.
pub fn load(
    text: &str,
    custom: bool,
    home: &Path,
    protected: &[Protected],
) -> (Vec<Rule>, Vec<RuleError>) {
    let file: RuleFile = match toml::from_str(text) {
        Ok(f) => f,
        Err(e) => {
            return (
                Vec::new(),
                vec![err("", format!("not a valid rule file: {e}"))],
            )
        }
    };
    if file.schema_version != SCHEMA_VERSION {
        return (
            Vec::new(),
            vec![err(
                "",
                format!("unknown schema_version {}", file.schema_version),
            )],
        );
    }
    let mut rules: Vec<Rule> = Vec::new();
    let mut errors = Vec::new();
    for raw in file.rule {
        if raw.platform != this_platform() {
            continue;
        }
        if rules.iter().any(|r| r.id == raw.id) {
            errors.push(err(&raw.id, "duplicate id"));
            continue;
        }
        match validate(raw, custom, home, protected) {
            Ok(rule) => rules.push(rule),
            Err(e) => errors.push(e),
        }
    }
    (rules, errors)
}

fn validate(
    raw: RawRule,
    custom: bool,
    home: &Path,
    protected: &[Protected],
) -> Result<Rule, RuleError> {
    let id = raw.id.clone();
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(err(&id, "id must be lowercase letters, digits and dashes"));
    }
    if raw.explanation.trim().is_empty() {
        return Err(err(&id, "every rule needs an explanation"));
    }
    let mut safety = match raw.safety.as_str() {
        "safe" => Safety::Safe,
        "your-call" => Safety::YourCall,
        "keep" => return Err(err(&id, "a cleanup rule can't be \"keep\"")),
        other => return Err(err(&id, format!("unknown safety \"{other}\""))),
    };
    if custom {
        safety = Safety::YourCall; // custom rules can never be safe (PLAN.md §6.2)
    }
    let delegate = match (raw.delegate.as_deref(), custom) {
        (None, _) => None,
        (Some(_), true) => return Err(err(&id, "custom rules can't delegate to the OS")),
        (Some("empty-trash"), false) => Some(Delegated::EmptyTrash),
        (Some(other), false) => return Err(err(&id, format!("unknown delegate \"{other}\""))),
    };
    if raw.roots.is_empty() {
        return Err(err(&id, "no roots"));
    }
    let mut roots = Vec::new();
    for r in &raw.roots {
        roots.push(check_root(&id, r, home, protected)?);
    }
    let include = globs(&id, &raw.include)?;
    let exclude = globs(&id, &raw.exclude)?;
    Ok(Rule {
        id,
        name: raw.name,
        category: raw.category,
        roots,
        include,
        exclude,
        safety,
        regenerates: raw.regenerates,
        min_age_days: raw.min_age_days,
        requires_app_closed: raw.requires_app_closed,
        explanation: raw.explanation,
        custom,
        delegate,
    })
}

fn expand(root: &str, home: &Path) -> PathBuf {
    match root.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if root == "~" => home.to_path_buf(),
        None => PathBuf::from(root),
    }
}

/// Firmlinked data-volume paths read as the user sees them.
fn user_path(p: PathBuf) -> PathBuf {
    p.strip_prefix("/System/Volumes/Data")
        .map(|r| Path::new("/").join(r))
        .unwrap_or(p)
}

fn check_root(
    id: &str,
    raw: &str,
    home: &Path,
    protected: &[Protected],
) -> Result<PathBuf, RuleError> {
    let root = expand(raw, home);
    if !root.is_absolute() {
        return Err(err(id, format!("root \"{raw}\" isn't absolute")));
    }
    if root.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(err(id, format!("root \"{raw}\" uses ..")));
    }
    // Judge a root by what it is now and, if it's a link, by where it points.
    let mut candidates = vec![root.clone()];
    if let Ok(real) = fs::canonicalize(&root) {
        candidates.push(user_path(real));
    }
    for c in &candidates {
        for p in protected {
            if p.protection == Protection::FolderOnly {
                continue; // a root may *be* a folder like Downloads; only its entries are removed
            }
            if c.starts_with(&p.path) {
                return Err(err(
                    id,
                    format!("root \"{raw}\" is inside protected {}", p.path.display()),
                ));
            }
            if p.path.starts_with(c) {
                return Err(err(
                    id,
                    format!("root \"{raw}\" contains protected {}", p.path.display()),
                ));
            }
        }
        if c.components().any(|part| {
            crate::platform::protected_name_suffixes()
                .iter()
                .any(|s| part.as_os_str().to_string_lossy().ends_with(s))
        }) {
            return Err(err(
                id,
                format!("root \"{raw}\" is inside an app-managed library"),
            ));
        }
    }
    Ok(root)
}

fn globs(id: &str, patterns: &[String]) -> Result<GlobSet, RuleError> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        if p != "**"
            && (p.contains('/')
                || p.contains('\\')
                || p.contains("..")
                || p.starts_with('~')
                || p.is_empty())
        {
            return Err(err(
                id,
                format!("pattern \"{p}\" must match names inside a root (no /, \\, .. or ~)"),
            ));
        }
        b.add(Glob::new(p).map_err(|e| err(id, format!("bad pattern \"{p}\": {e}")))?);
    }
    b.build().map_err(|e| err(id, e.to_string()))
}

/// One cleanable item a rule found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub rule_id: String,
    pub rule_name: String,
    pub category: String,
    pub path: PathBuf,
    pub root: PathBuf,
    pub safety: Safety,
    pub explanation: String,
    pub regenerates: bool,
    pub custom: bool,
    pub bytes: u64,
    /// Cleaned through the OS (e.g. emptying the Trash) rather than item by item.
    pub delegate: Option<Delegated>,
}

/// A rule that's on hold, e.g. because its app is open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Held {
    pub rule_id: String,
    pub rule_name: String,
    /// The app that has to be closed first.
    pub app: String,
}

/// What every rule matches right now. `running` are bundle ids (macOS) of open apps; `size_of`
/// gives an item's bytes (from the scan when available).
pub fn find(
    rules: &[Rule],
    running: &[String],
    size_of: &dyn Fn(&Path) -> u64,
) -> (Vec<Match>, Vec<Held>) {
    let all_roots: Vec<&PathBuf> = rules.iter().flat_map(|r| r.roots.iter()).collect();
    let now_days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    let mut out: Vec<Match> = Vec::new();
    let mut held = Vec::new();
    for rule in rules {
        if let Some(app) = &rule.requires_app_closed {
            if running.iter().any(|r| r == app) {
                held.push(Held {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    app: app.clone(),
                });
                continue;
            }
        }
        for root in &rule.roots {
            let Ok(md) = fs::symlink_metadata(root) else {
                continue;
            };
            if !md.is_dir() {
                continue;
            }
            let base = Match {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                category: rule.category.clone(),
                path: root.clone(),
                root: root.clone(),
                safety: rule.safety,
                explanation: rule.explanation.clone(),
                regenerates: rule.regenerates,
                custom: rule.custom,
                bytes: 0,
                delegate: rule.delegate,
            };
            if rule.delegate.is_some() {
                let bytes = size_of(root);
                if bytes > 0 {
                    out.push(Match { bytes, ..base });
                }
                continue;
            }
            let Ok(entries) = fs::read_dir(root) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                let name = e.file_name();
                let any = rule.include.is_match(Path::new(&name));
                if !any || rule.exclude.is_match(Path::new(&name)) {
                    continue;
                }
                // Another rule's root at or under this entry: that rule handles it.
                if all_roots.iter().any(|r| *r != root && r.starts_with(&path)) {
                    continue;
                }
                if rule.min_age_days > 0 {
                    let Ok(md) = e.metadata() else { continue };
                    let day = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map_or(now_days, |d| d.as_secs() / 86_400);
                    if now_days.saturating_sub(day) < rule.min_age_days {
                        continue;
                    }
                }
                let bytes = size_of(&path);
                out.push(Match {
                    path,
                    bytes,
                    ..base.clone()
                });
            }
        }
    }
    // The same item from two rules: the one with the more specific (deeper) root wins.
    out.sort_by(|a, b| {
        a.path.cmp(&b.path).then(
            b.root
                .components()
                .count()
                .cmp(&a.root.components().count()),
        )
    });
    out.dedup_by(|a, b| a.path == b.path);
    out.sort_by_key(|m| std::cmp::Reverse(m.bytes));
    (out, held)
}

/// Bundled rules plus the user's custom rules from `<data>/rules/custom/*.toml`.
pub fn load_all(
    home: &Path,
    data_dir: &Path,
    protected: &[Protected],
) -> (Vec<Rule>, Vec<RuleError>) {
    let (mut rules, mut errors) = load(bundled_text(), false, home, protected);
    let dir = data_dir.join("rules").join("custom");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|es| {
            es.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for f in files {
        let Ok(text) = fs::read_to_string(&f) else {
            continue;
        };
        let (more, errs) = load(&text, true, home, protected);
        for r in more {
            if rules.iter().any(|x| x.id == r.id) {
                errors.push(err(&r.id, "a custom rule can't reuse a bundled rule's id"));
            } else {
                rules.push(r);
            }
        }
        errors.extend(errs);
    }
    (rules, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;

    struct Fixture {
        base: PathBuf,
        home: PathBuf,
        protected: Vec<Protected>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            platform::remove_test_dir(&self.base);
        }
    }

    fn fixture(tag: &str) -> Fixture {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("wrasse-rules-{tag}-{nanos}"));
        let home = base.join("home");
        fs::create_dir_all(&home).unwrap();
        let home = fs::canonicalize(&home).unwrap();
        let protected = platform::protected_paths(&home, &base.join("data"));
        Fixture {
            base: fs::canonicalize(&base).unwrap(),
            home,
            protected,
        }
    }

    fn put(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    fn age(path: &Path, days: u64) {
        let t = SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn rule_file(body: &str) -> String {
        format!("schema_version = 1\n{body}")
    }

    fn one_rule(roots: &str, include: &str, extra: &str) -> String {
        rule_file(&format!(
            "[[rule]]\nid = \"t\"\nname = \"T\"\nplatform = \"macos\"\ncategory = \"temp\"\nroots = [{roots}]\ninclude = [{include}]\nsafety = \"safe\"\nexplanation = \"x\"\n{extra}"
        ))
    }

    #[test]
    fn bundled_pack_is_valid_on_this_mac() {
        let home = platform::home_dir().unwrap();
        let protected = platform::protected_paths(&home, Path::new("/nonexistent-wrasse-data"));
        let (rules, errors) = load(bundled_text(), false, &home, &protected);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(rules.len(), 16);
    }

    #[test]
    fn each_macos_rule_matches_its_fixture() {
        let f = fixture("pack");
        let h = &f.home;
        put(&h.join("Library/Caches/com.spotify.client/data"), 3000);
        put(&h.join("Library/Caches/com.apple.bird/x"), 10); // excluded (iCloud)
        put(
            &h.join("Library/Caches/Google/Chrome/Default/Cache/a"),
            2000,
        );
        put(&h.join("Library/Caches/Firefox/Profiles/a"), 100);
        put(&h.join("Library/Caches/com.apple.Safari/Cache.db"), 100);
        put(&h.join("Library/Caches/Arc/a"), 100);
        put(&h.join("Library/Caches/BraveSoftware/a"), 100);
        put(&h.join("Library/Caches/Microsoft Edge/a"), 100);
        put(&h.join("Library/Logs/Spotify/log.txt"), 100);
        put(&h.join("Library/Logs/DiagnosticReports/App.ips"), 100);
        put(
            &h.join("Library/Developer/Xcode/DerivedData/MyApp-abc/Build/x"),
            100,
        );
        put(
            &h.join("Library/Developer/Xcode/Archives/2025-01-01/a.xcarchive"),
            100,
        );
        age(
            &h.join("Library/Developer/Xcode/Archives/2025-01-01/a.xcarchive"),
            200,
        );
        put(
            &h.join("Library/Developer/Xcode/iOS DeviceSupport/17.0/x"),
            100,
        );
        put(
            &h.join("Library/Application Support/MobileSync/Backup/abc/Manifest.db"),
            100,
        );
        put(
            &h.join("Library/Containers/com.apple.mail/Data/Library/Mail Downloads/1/a.pdf"),
            100,
        );
        put(&h.join("Downloads/OldApp.dmg"), 100);
        put(&h.join("Downloads/NewApp.dmg"), 100);
        put(&h.join("Downloads/notes.txt"), 100);
        put(&h.join(".Trash/old.txt"), 100);
        age(&h.join("Downloads/OldApp.dmg"), 45);
        // Age the folders whose rules have a minimum age (a folder's own date is what counts).
        for d in [
            "Library/Developer/Xcode/Archives/2025-01-01",
            "Library/Developer/Xcode/iOS DeviceSupport/17.0",
        ] {
            let t = SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
            fs::File::open(h.join(d)).unwrap().set_modified(t).unwrap();
        }

        let (rules, errors) = load(bundled_text(), false, h, &f.protected);
        assert!(errors.is_empty(), "{errors:?}");
        let (found, held) = find(&rules, &["com.apple.Safari".to_string()], &|p| {
            crate::quarantine::size_on_disk(p)
        });
        let by = |id: &str| {
            found
                .iter()
                .filter(|m| m.rule_id == id)
                .map(|m| m.path.strip_prefix(h).unwrap().to_path_buf())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            by("app-caches"),
            [PathBuf::from("Library/Caches/com.spotify.client")]
        );
        assert_eq!(
            by("chrome-cache"),
            [PathBuf::from("Library/Caches/Google/Chrome/Default")]
        );
        assert_eq!(
            by("firefox-cache"),
            [PathBuf::from("Library/Caches/Firefox/Profiles")]
        );
        assert!(
            by("safari-cache").is_empty(),
            "Safari is open, so its rule is held"
        );
        assert_eq!(held[0].rule_id, "safari-cache");
        assert_eq!(by("arc-cache").len(), 1);
        assert_eq!(by("brave-cache").len(), 1);
        assert_eq!(by("edge-cache").len(), 1);
        assert_eq!(by("app-logs"), [PathBuf::from("Library/Logs/Spotify")]);
        assert_eq!(
            by("crash-reports"),
            [PathBuf::from("Library/Logs/DiagnosticReports/App.ips")]
        );
        assert_eq!(
            by("xcode-derived-data"),
            [PathBuf::from(
                "Library/Developer/Xcode/DerivedData/MyApp-abc"
            )]
        );
        assert_eq!(
            by("xcode-old-archives"),
            [PathBuf::from("Library/Developer/Xcode/Archives/2025-01-01")]
        );
        assert_eq!(
            by("xcode-device-support"),
            [PathBuf::from(
                "Library/Developer/Xcode/iOS DeviceSupport/17.0"
            )]
        );
        assert_eq!(
            by("ios-backups"),
            [PathBuf::from(
                "Library/Application Support/MobileSync/Backup/abc"
            )]
        );
        assert_eq!(by("mail-downloads").len(), 1);
        assert_eq!(
            by("old-installers"),
            [PathBuf::from("Downloads/OldApp.dmg")],
            "only installers older than 30 days"
        );
        assert_eq!(by("trash"), [PathBuf::from(".Trash")]);
        assert!(found
            .iter()
            .find(|m| m.rule_id == "trash")
            .unwrap()
            .delegate
            .is_some());
    }

    #[test]
    fn malicious_rules_are_rejected() {
        let f = fixture("evil");
        let h = &f.home;
        fs::create_dir_all(h.join("Library/Keychains")).unwrap();
        fs::create_dir_all(h.join("Library/Caches")).unwrap();
        std::os::unix::fs::symlink(h.join("Library/Keychains"), h.join("Library/Caches/evil"))
            .unwrap();
        let cases = [
            (
                "root inside Documents",
                one_rule("\"~/Documents\"", "\"**\"", ""),
            ),
            ("root is home", one_rule("\"~\"", "\"**\"", "")),
            (
                "root contains protected",
                one_rule("\"~/Library\"", "\"**\"", ""),
            ),
            ("system root", one_rule("\"/System/Library\"", "\"**\"", "")),
            ("whole disk", one_rule("\"/\"", "\"**\"", "")),
            (
                "escape with ..",
                one_rule("\"~/Library/Caches/../Keychains\"", "\"**\"", ""),
            ),
            (
                "symlink escape",
                one_rule("\"~/Library/Caches/evil\"", "\"**\"", ""),
            ),
            (
                "relative root",
                one_rule("\"Library/Caches\"", "\"**\"", ""),
            ),
            (
                "glob escape ..",
                one_rule("\"~/Library/Caches\"", "\"../*\"", ""),
            ),
            (
                "glob absolute",
                one_rule("\"~/Library/Caches\"", "\"/etc/*\"", ""),
            ),
            (
                "glob nested path",
                one_rule("\"~/Library/Caches\"", "\"a/b\"", ""),
            ),
            (
                "keep rule",
                one_rule("\"~/Library/Caches\"", "\"**\"", "")
                    .replace("safety = \"safe\"", "safety = \"keep\""),
            ),
            (
                "unknown delegate",
                one_rule("\"~/Library/Caches\"", "\"**\"", "delegate = \"rm-rf\""),
            ),
            (
                "photos library",
                one_rule("\"~/Pictures/Photos Library.photoslibrary\"", "\"**\"", ""),
            ),
        ];
        for (what, text) in cases {
            let (rules, errors) = load(&text, false, h, &f.protected);
            assert!(
                rules.is_empty() && !errors.is_empty(),
                "{what} should be rejected"
            );
        }
        // Wrong schema version: the whole file is refused.
        let (rules, errors) = load(
            &one_rule("\"~/Library/Caches\"", "\"**\"", "")
                .replace("schema_version = 1", "schema_version = 2"),
            false,
            h,
            &f.protected,
        );
        assert!(rules.is_empty() && errors[0].why.contains("schema_version"));
        // Custom rules can't be safe and can't delegate.
        let (rules, _) = load(
            &one_rule("\"~/Library/Caches\"", "\"**\"", ""),
            true,
            h,
            &f.protected,
        );
        assert_eq!(rules[0].safety, Safety::YourCall);
        let (rules, errors) = load(
            &one_rule("\"~/.Trash\"", "\"**\"", "delegate = \"empty-trash\""),
            true,
            h,
            &f.protected,
        );
        assert!(rules.is_empty() && errors[0].why.contains("delegate"));
    }
}
