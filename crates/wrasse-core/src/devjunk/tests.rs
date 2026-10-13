use super::*;

macro_rules! ask {
    ($name:expr, $children:expr, $parents:expr, $parent:expr, $grand:expr, $age:expr, $mods:expr, $wrasse:expr) => {
        project_item(&ProjectSpot {
            name: $name,
            children: $children,
            parent_children: $parents,
            parent_name: $parent,
            grandparent_name: $grand,
            age_days: $age,
            in_modules: $mods,
            in_wrasse: $wrasse,
        })
    };
}

#[test]
fn wrasse_and_its_toolchain_are_never_project_junk() {
    assert!(is_wrasse_root(&["src", ".toolchain", "app"]));
    assert!(!is_wrasse_root(&["src", "app"]));
    assert!(wrasse_path("/Users/me/Documents/Wrasse/.toolchain/rust"));
    assert!(wrasse_path("/Users/me/Documents/Wrasse/target/debug"));
    assert!(wrasse_path("/Users/me/Documents/Wrasse"));
    assert!(!wrasse_path("/Users/me/Library/Developer/Toolchains/x"));
    let kids = ["Cargo.toml", "src"];
    assert_eq!(
        ask!(
            "target",
            &[],
            &kids,
            "Wrasse",
            "Documents",
            400,
            false,
            true
        ),
        None
    );
    assert_eq!(
        ask!(
            "node_modules",
            &[],
            &[],
            "Wrasse",
            "Documents",
            400,
            false,
            true
        ),
        None
    );
    assert_eq!(
        ask!(
            ".toolchain",
            &["rust"],
            &[],
            "Wrasse",
            "Documents",
            400,
            false,
            false
        ),
        None
    );
}

#[test]
fn stale_modules_and_targets_are_claimed_and_recent_ones_are_not() {
    assert_eq!(
        ask!(
            "node_modules",
            &[],
            &[],
            "web",
            "Projects",
            90,
            false,
            false
        ),
        Some(ProjectItem::Modules)
    );
    assert_eq!(
        ask!(
            "node_modules",
            &[],
            &[],
            "web",
            "Projects",
            89,
            false,
            false
        ),
        None
    );
    // A nested copy stays inside the outer one.
    assert_eq!(
        ask!(
            "node_modules",
            &[],
            &[],
            "pkg",
            "node_modules",
            400,
            true,
            false
        ),
        None
    );
    assert_eq!(
        ask!(
            "target",
            &[],
            &["Cargo.toml", "src"],
            "api",
            "code",
            90,
            false,
            false
        ),
        Some(ProjectItem::Target)
    );
    assert_eq!(
        ask!(
            "target",
            &[],
            &["Makefile"],
            "api",
            "code",
            400,
            false,
            false
        ),
        None
    );
    assert_eq!(
        project_label(ProjectItem::Modules, "web"),
        "web · node_modules"
    );
}

#[test]
fn venvs_need_a_pyvenv_and_conda_envs_need_their_root() {
    assert_eq!(
        ask!(
            ".venv",
            &["pyvenv.cfg", "bin"],
            &[],
            "app",
            "code",
            1,
            false,
            false
        ),
        Some(ProjectItem::Venv)
    );
    assert_eq!(
        ask!(
            "env",
            &["pyvenv.cfg"],
            &[],
            "vyakta",
            "Documents",
            10,
            false,
            false
        ),
        Some(ProjectItem::Venv)
    );
    assert_eq!(
        ask!(
            "env",
            &["data"],
            &[],
            "vyakta",
            "Documents",
            10,
            false,
            false
        ),
        None
    );
    assert_eq!(
        ask!("ml", &[], &[], "envs", "miniconda3", 10, false, false),
        Some(ProjectItem::Conda)
    );
    assert_eq!(
        ask!("ml", &[], &[], "envs", "projects", 10, false, false),
        None
    );
}

#[test]
fn docker_and_homebrew_are_shown_and_not_removable() {
    let docker = policy_for("/Users/me/Library/Containers/com.docker.docker", 10).unwrap();
    assert!(docker.read_only && docker.safety == Safety::Keep && !docker.rebuildable);
    let brew = policy_for("/opt/homebrew", 10).unwrap();
    assert!(brew.read_only && brew.safety == Safety::Keep);
    let derived = policy_for("/Users/me/Library/Developer/Xcode/DerivedData", 1).unwrap();
    assert!(derived.rebuildable && derived.safety == Safety::Safe);
    let archive = policy_for("/Users/me/Library/Developer/Xcode/Archives/2024-03-01", 100).unwrap();
    assert_eq!(archive.dev_kind, Some("archive"));
    assert!(!archive.rebuildable);
    assert_eq!(rank_why(false, Some("archive"), 100), Some(RankWhy::Unused));
    assert_eq!(rank_why(false, Some("archive"), 10), None);
}

#[test]
fn a_project_cache_in_documents_stays_your_call() {
    let p = policy_for("/Users/me/Documents/app/node_modules", 200).unwrap();
    assert_eq!(p.safety, Safety::YourCall);
    assert!(!p.rebuildable);
    let outside = policy_for("/Users/me/Projects/web/node_modules", 200).unwrap();
    assert_eq!(outside.safety, Safety::Safe);
    assert!(outside.rebuildable);
    let fresh = policy_for("/Users/me/Projects/web/node_modules", 3).unwrap();
    assert!(!fresh.rebuildable);
}

#[test]
fn previews_parse_without_running_the_tools() {
    let brew = "Would remove: /opt/homebrew/Cellar/wget/1.21 (1.2MB)\n==> This operation would free approximately 12.5MB of disk space.\n";
    assert_eq!(parse_brew_cleanup(brew), 12_500_000);
    let df = "\
TYPE            TOTAL     ACTIVE    SIZE      RECLAIMABLE
Images          2         1         2.0GB     1.5GB (75%)
Containers      1         0         100MB     100MB (100%)
Local Volumes   1         1         9.0GB     9.0GB (100%)
Build Cache     4         0         512MB     512MB
";
    // Volumes are not part of `docker system prune` without `--volumes`.
    assert_eq!(
        parse_docker_df(df),
        1_500_000_000 + 100_000_000 + 512_000_000
    );
    let sims = "\
-- iOS 17.0 --
iPhone 15 (A1B2C3D4-E5F6-7890-ABCD-EF1234567890) (unavailable)
iPhone 14 (11111111-2222-3333-4444-555555555555) (Shutdown)
";
    assert_eq!(
        parse_unavailable_simulators(sims),
        vec!["A1B2C3D4-E5F6-7890-ABCD-EF1234567890".to_string()]
    );
}

/// A folded Developer tile comes back without changing the volume total.
/// A tile that is already showing lists the preview and does not add its bytes again.
#[test]
fn live_previews_keep_the_volume_total() {
    use crate::scan::breakdown::Summary;
    let mut root = UiNode::plain(
        0,
        "Macintosh HD",
        "/",
        -1,
        0,
        Category::Other,
        NodeKind::Volume,
        Safety::Keep,
        "",
    );
    root.size = 100;
    root.children = vec![1];
    let mut system = UiNode::plain(
        1,
        "System data",
        "/",
        0,
        1,
        Category::System,
        NodeKind::File,
        Safety::Keep,
        "",
    );
    system.size = 40;
    let dev = UiNode::plain(
        2,
        "Developer",
        "/Users/me/Library",
        -1,
        1,
        Category::Developer,
        NodeKind::Dir,
        Safety::Safe,
        "",
    );
    let mut disk = UiDisk {
        nodes: vec![root, system, dev],
        root: 0,
        capacity: 200,
        ghost_id: -1,
        leaf_count: 1,
        summary: Summary::default(),
    };
    let extra = LiveExtra {
        name: "Homebrew cleanup".into(),
        path: "homebrew:cleanup".into(),
        bytes: 15,
        why: "preview".into(),
        action: "brewCleanup".into(),
    };
    attach_live(&mut disk, std::slice::from_ref(&extra));
    assert!(disk.nodes[0].children.contains(&2));
    assert_eq!(disk.nodes[0].size, 100);
    assert_eq!(disk.nodes[1].size, 25);
    assert_eq!(disk.nodes[2].size, 15);
    let child = *disk.nodes[2].children.last().unwrap();
    assert_eq!(
        disk.nodes[child as usize].delegate_action.as_deref(),
        Some("brewCleanup")
    );
    assert_eq!(disk.nodes[child as usize].size, 15);

    let kids = disk.nodes[2].children.len();
    attach_live(&mut disk, &[extra]);
    assert_eq!(disk.nodes[0].size, 100);
    assert_eq!(disk.nodes[1].size, 25);
    assert_eq!(disk.nodes[2].size, 15);
    assert_eq!(disk.nodes[2].children.len(), kids + 1);
}
