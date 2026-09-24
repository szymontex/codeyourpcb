//! Does a board come out as the same files in every run of the program?
//!
//! `cargo test -p cypcb-cli --test every_file_is_the_same_in_every_run`
//!
//! bevy matches a query to its archetypes through a hash map whose seed is
//! drawn once per process. Rows of one kind that do not all carry the same
//! components sit in more than one archetype - a part with a `spec` block and
//! one without, a trace with a curve and one without - and every writer that
//! followed the order of its query wrote them in a different order from one
//! run to the next. Across ten runs of the binary `examples/v2-constraints.cypcb`
//! exported to two different sets of Gerbers, IPC-2581 documents and KiCad
//! boards, and `examples/curved-track.cypcb` to two of each as well. The DSN
//! file had a second cause beside it: its placement and its footprint images
//! were walked out of a std hash map, and one board came out ten different
//! ways in ten runs.
//!
//! Inside one process the order never moves, so a test that exports twice in
//! one process sees nothing. This runs the binary itself, once per run, and
//! compares what each run wrote. The seed cannot be set from outside, which is
//! why the count is high enough that a board with two outcomes shows both.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

/// One board with parts in two archetypes, one with traces in two.
const BOARDS: &[&str] = &["v2-constraints", "curved-track"];

/// Runs of the program. On these two boards the rarer of two outcomes came up
/// three times in ten, so sixteen runs all landing on the same one by chance
/// is about one in three hundred.
const RUNS: usize = 16;

/// Lines that carry the moment a file was written, which differ by design.
const STAMPS: &[&str] = &["CreationDate", "export_date", "origination="];

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(format!("{name}.cypcb"))
}

/// Run `cypcb` with `args` and insist it succeeded.
fn run(args: &[&std::ffi::OsStr]) {
    let output = cypcb().args(args).output().expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb {}` failed:\n{}",
        args.iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Every file under `dir`, as a path relative to it and a hash of its text
/// with the stamp lines left out.
fn hashes_under(dir: &Path) -> BTreeMap<String, u64> {
    let mut found = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).expect("the output directory is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("every file written is text");
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            for line in text
                .lines()
                .filter(|line| !STAMPS.iter().any(|stamp| line.contains(stamp)))
            {
                line.hash(&mut hash);
            }
            let relative = path
                .strip_prefix(dir)
                .expect("everything found is under the directory")
                .to_string_lossy()
                .to_string();
            found.insert(relative, hash.finish());
        }
    }
    found
}

/// One run of the program per command: the fabrication files with the
/// IPC-2581 document, the DSN file, and the KiCad board.
fn write_everything(name: &str, run_index: usize) -> BTreeMap<String, u64> {
    let dir = std::env::temp_dir().join(format!("cypcb-every-run-{name}-{run_index}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory can be made");

    // `route --dry-run` writes the DSN file beside the design, so the design
    // is copied in first and taken out again before the files are hashed.
    let source = dir.join(format!("{name}.cypcb"));
    std::fs::copy(example(name), &source).expect("the example can be copied");
    let out = dir.join("out");
    let kicad = dir.join("board.kicad_pcb");

    run(&[
        "export".as_ref(),
        source.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
        "--ipc2581".as_ref(),
        "--force".as_ref(),
    ]);
    run(&["route".as_ref(), "--dry-run".as_ref(), source.as_os_str()]);
    run(&[
        "to-kicad".as_ref(),
        source.as_os_str(),
        "-o".as_ref(),
        kicad.as_os_str(),
    ]);

    std::fs::remove_file(&source).expect("the copied example can be removed");
    let hashes = hashes_under(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    hashes
}

#[test]
fn every_file_is_the_same_in_every_run() {
    let mut unstable = Vec::new();
    for name in BOARDS {
        let runs: Vec<BTreeMap<String, u64>> = (0..RUNS)
            .map(|index| write_everything(name, index))
            .collect();
        assert!(
            runs.iter().all(|run| run.keys().eq(runs[0].keys())),
            "{name} wrote a different set of files from one run to the next"
        );
        for file in runs[0].keys() {
            let mut seen: Vec<u64> = runs.iter().map(|run| run[file]).collect();
            seen.sort_unstable();
            seen.dedup();
            if seen.len() > 1 {
                unstable.push(format!("{name}: {file} ({} versions)", seen.len()));
            }
        }
    }
    assert!(
        unstable.is_empty(),
        "the same board written {RUNS} times by separate runs of the program has to \
         give the same files; these were not:\n  {}",
        unstable.join("\n  ")
    );
}
