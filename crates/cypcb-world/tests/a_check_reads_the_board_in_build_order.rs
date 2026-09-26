//! What `check` reads from the board, it reads in build order.
//!
//! `cargo test -p cypcb-world --test a_check_reads_the_board_in_build_order`
//!
//! A bevy query visits its tables in the order of a hash map that is seeded
//! afresh in every process, so anything built from `query.iter` comes out in
//! a different order from one run to the next. `in_build_order` sorts the rows
//! by entity, which is the order the design file spawned them in, and that is
//! a property of the board alone. This holds every source on the path from a
//! file to the report - the board model and every rule - to that one door.
//!
//! The router, the exporters, the viewer and the KiCad writer still walk
//! queries of their own. They are outside what `check` prints, and are not
//! held here.

use std::path::{Path, PathBuf};

/// Every file on the check path allowed a query of its own, how many, and why.
const ALLOWED: &[(&str, usize, &str)] = &[("cypcb-world/src/world.rs", 1, "in_build_order itself")];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir)
        .expect("a source directory reads")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// Queries per file, in the code a build ships: a file's test module is
/// left out, since a test reads the world it just made and prints nothing,
/// and so is a comment, where an example query runs nowhere.
fn queries() -> Vec<(String, usize)> {
    let root = crates_dir();
    let mut files = Vec::new();
    rust_files(&root.join("cypcb-world/src"), &mut files);
    rust_files(&root.join("cypcb-drc/src"), &mut files);
    files.push(root.join("cypcb-cli/src/commands/check.rs"));
    files.sort();

    let mut counts = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a source file reads");
        let shipped = text.split("#[cfg(test)]").next().unwrap_or_default();
        let calls = shipped
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| {
                [".query::<", ".query_filtered::<"]
                    .iter()
                    .map(|call| line.matches(call).count())
                    .sum::<usize>()
            })
            .sum::<usize>();
        if calls > 0 {
            let name = file
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            counts.push((name, calls));
        }
    }
    counts
}

#[test]
fn every_query_on_the_check_path_goes_through_in_build_order() {
    let found = queries();
    let unlisted: Vec<String> = found
        .iter()
        .filter(|(file, count)| {
            !ALLOWED
                .iter()
                .any(|(allowed, most, _)| allowed == file && count <= most)
        })
        .map(|(file, count)| format!("{file}: {count}"))
        .collect();
    assert!(
        unlisted.is_empty(),
        "a query walked in bevy's order, which changes from run to run; \
         use in_build_order:\n{}",
        unlisted.join("\n")
    );

    for (allowed, most, why) in ALLOWED {
        let count = found
            .iter()
            .find(|(file, _)| file == allowed)
            .map_or(0, |(_, count)| *count);
        assert_eq!(
            count, *most,
            "{allowed} is listed for {most} ({why}) and has {count}: \
             bring the list down with the code"
        );
    }
}
