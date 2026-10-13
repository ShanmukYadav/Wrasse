//! make-fixture <folder> <files>: writes a benchmark tree of empty files (see `many_files`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(root), Some(files)) = (
        args.first(),
        args.get(1).and_then(|n| n.parse::<u64>().ok()),
    ) else {
        eprintln!("usage: make-fixture <folder> <files>");
        std::process::exit(2);
    };
    let started = std::time::Instant::now();
    let dirs = match wrasse_fixtures::many_files(std::path::Path::new(root), files) {
        Ok(dirs) => dirs,
        Err(e) => {
            eprintln!("couldn't write the fixture: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "{files} files in {dirs} folders, {:.1}s",
        started.elapsed().as_secs_f64()
    );
}
