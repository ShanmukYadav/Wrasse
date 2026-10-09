//! Generates fake home folders (macOS and Windows layouts) in temp dirs for tests.
//! Destructive code is never tested against the real home folder (PLAN.md §4.3 rule 13).

use std::fs;
use std::io;
use std::path::Path;

/// A wide, deep tree of `files` empty files for scan benchmarks (PLAN.md §5.3: 2M files under
/// 300 MB). Files are spread 100 per folder, folders 100 per parent, so the shape resembles a
/// real disk (many folders, few huge ones). Empty files take no blocks, so the fixture needs
/// metadata space only. Returns the number of folders created.
pub fn many_files(root: &Path, files: u64) -> io::Result<u64> {
    const PER_DIR: u64 = 100;
    const FANOUT: u64 = 100;
    let dirs = files.div_ceil(PER_DIR);
    let mut made = 0;
    for d in 0..dirs {
        let dir = root
            .join(format!("g{:03}", d / (FANOUT * FANOUT)))
            .join(format!("p{:03}", (d / FANOUT) % FANOUT))
            .join(format!("d{:03}", d % FANOUT));
        fs::create_dir_all(&dir)?;
        made += 1;
        let start = d * PER_DIR;
        for f in start..(start + PER_DIR).min(files) {
            fs::File::create(dir.join(format!("f{f:07}.txt")))?;
        }
    }
    Ok(made)
}
