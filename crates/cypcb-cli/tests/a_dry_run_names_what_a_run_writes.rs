//! `--dry-run` names the files a run writes.
//!
//! `cargo test -p cypcb-cli --test a_dry_run_names_what_a_run_writes`
//!
//! Its whole purpose is to be read before a run happens - before spending
//! money at a fabricator - and it was wrong in two ways at once. The listing
//! hard-coded `output/...` fourteen times, so a run given `--output
//! /somewhere/else` was told paths nothing would write. And it left out the
//! Gerber job file: thirteen names listed against fourteen files written, and
//! the missing one is what a fab's software opens first.
//!
//! So this does not check the wording. It runs the dry run, runs the export
//! into the same directory, and asks that the two sets are the same.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-dry-run-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Every file under `dir`, as paths relative to it.
fn files_under(dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).expect("the output directory is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(dir)
                    .expect("everything found is under the directory")
                    .to_string_lossy()
                    .to_string();
                found.insert(relative);
            }
        }
    }
    found
}

/// The paths a dry run listed, relative to the directory it was given.
fn dry_run_listing(board: &Path, out: &Path) -> BTreeSet<String> {
    let output = cypcb()
        .arg("export")
        .arg(board)
        .arg("-o")
        .arg(out)
        .arg("--dry-run")
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "the dry run failed");
    // The paths come back on stdout. They used to arrive on stderr with the
    // progress lines, so `export --dry-run board.cypcb > set.txt` wrote an
    // empty file; the listing is the answer this command is asked for, so it
    // goes to the stream a pipe reads.
    let listing = String::from_utf8_lossy(&output.stdout).to_string();

    let prefix = format!("{}/", out.display());
    listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(str::to_string))
        .collect()
}

#[test]
fn the_listing_is_the_file_set() {
    let board = example("blink.cypcb");
    let out = scratch("blink");

    let listed = dry_run_listing(&board, &out);
    assert!(
        listed.len() >= 10,
        "nothing was listed under the directory the run was given: {listed:?}"
    );
    assert!(
        !out.exists(),
        "a dry run must not write anything, and it created {}",
        out.display()
    );

    let status = cypcb()
        .arg("export")
        .arg(&board)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let written = files_under(&out);
    assert_eq!(
        listed, written,
        "the dry run named a different set of files than the run wrote"
    );
}

#[test]
fn a_four_layer_board_is_listed_as_four_layers() {
    // The inner copper comes from the board rather than from the house's
    // profile, and the same has to hold in the listing - a two-layer set
    // promised for a four-layer board is the sentence a person reads before
    // paying for it.
    let board = example("four-layer.cypcb");
    let out = scratch("four-layer");

    let listed = dry_run_listing(&board, &out);
    let inner: Vec<&String> = listed.iter().filter(|name| name.contains("In")).collect();
    assert_eq!(
        inner.len(),
        2,
        "a four-layer board has two inner copper files: {listed:?}"
    );

    let status = cypcb()
        .arg("export")
        .arg(&board)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert_eq!(listed, files_under(&out), "listed against written");
}
