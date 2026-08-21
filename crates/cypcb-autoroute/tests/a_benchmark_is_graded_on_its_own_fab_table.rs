//! No new harness may grade a benchmark on a table that benchmark does not use.
//!
//! `cargo test -p cypcb-autoroute --test a_benchmark_is_graded_on_its_own_fab_table`
//!
//! `multi_ic.kicad_pcb` has four copper layers, so `cypcb check` reads it
//! against `jlcpcb_standard_4layer` while the other five benchmarks read
//! against the two-layer table. Harnesses in this crate built a fixed
//! two-layer `DesignRules` and handed it every board, which on `multi_ic` does
//! not even rank the same routing variant first: the wrong table puts
//! `PathFinder Eager Pads Priced Ring` first at 172 / 84 and the board's own
//! table puts `PathFinder Eager Pads` first at 371 / 128, which is what
//! `cypcb route --variants` prints.
//!
//! Twenty-two files were in that position when it was found. Converting them
//! in one pass would be a sweep nobody could review, and `benchmark_validation`
//! cannot be converted at all without re-baselining every ratchet it holds. So
//! the list below is the work that remains, and this test is what stops it
//! growing: a file may leave the list, and no file may join it.
//!
//! The list is not a count in prose, which is the point - it is the count
//! itself, and `cargo test -p cypcb-autoroute --test
//! a_benchmark_is_graded_on_its_own_fab_table` reads it out of the source
//! rather than out of a sentence somebody forgot to update.

use std::fs;
use std::path::Path;

/// Harnesses that still hand every benchmark one fixed table.
///
/// This list shrinks and never grows. Most entries are simply not converted
/// yet. `benchmark_validation.rs` is the one with a reason to stay: its
/// thresholds were recorded against the two-layer table, so moving its
/// yardstick moves every ratchet at once and wants a deliberate re-baseline
/// rather than a drive-by. Whichever it is, converting a file means deleting
/// its name here in the same commit - the test below fails if the two disagree
/// in either direction.
const STILL_ON_A_FIXED_TABLE: &[&str] = &["benchmark_validation.rs"];

/// This file has to name the very things it forbids, so it cannot judge itself.
const SELF: &str = "a_benchmark_is_graded_on_its_own_fab_table.rs";

/// The marker for a harness that reaches the benchmark set at all.
///
/// `multi_ic` is named as well as `BENCHMARKS` because a file may reach the
/// four-layer board by name without walking the whole set.
fn touches_the_benchmarks(source: &str) -> bool {
    source.contains("BENCHMARKS") || source.contains("multi_ic")
}

/// The marker for one table handed to every board regardless of its layers.
///
/// Any named `DesignRules` constructor counts, not only the two-layer one: a
/// harness that handed every benchmark `jlcpcb_4layer()` would be wrong in the
/// other direction and just as invisible. No test in this crate uses one of
/// the others today, which is why the list below is all two-layer.
fn builds_a_fixed_table(source: &str) -> bool {
    const FIXED: &[&str] = &[
        "jlcpcb_2layer()",
        "jlcpcb_4layer()",
        "jlcpcb_advanced_2layer()",
        "jlcpcb_advanced_4layer()",
        "oshpark_2layer()",
        "oshpark_4layer()",
        "pcbway_standard()",
        "prototype()",
    ];
    FIXED.iter().any(|marker| source.contains(marker))
}

#[test]
fn no_new_harness_grades_a_benchmark_on_a_fixed_table() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

    let mut found = Vec::new();
    for entry in fs::read_dir(&dir).expect("the crate has a tests directory") {
        let path = entry.expect("the directory entry reads").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a test file has a name")
            .to_string();
        if name == SELF {
            continue;
        }
        let source = fs::read_to_string(&path).expect("a test file reads");
        if touches_the_benchmarks(&source) && builds_a_fixed_table(&source) {
            found.push(name);
        }
    }
    found.sort();

    let mut joined: Vec<&str> = found
        .iter()
        .map(String::as_str)
        .filter(|n| !STILL_ON_A_FIXED_TABLE.contains(n))
        .collect();
    joined.sort();
    assert!(
        joined.is_empty(),
        "these harnesses reach the benchmark set and grade it on one fixed \
         table: {joined:?}. `multi_ic` has four copper layers, so resolve the \
         board's own table with `cypcb_drc::preset_for_world` rather than \
         adding a name to STILL_ON_A_FIXED_TABLE"
    );

    let mut left: Vec<&str> = STILL_ON_A_FIXED_TABLE
        .iter()
        .copied()
        .filter(|n| !found.iter().any(|f| f == n))
        .collect();
    left.sort();
    assert!(
        left.is_empty(),
        "these names are in STILL_ON_A_FIXED_TABLE but no longer grade a \
         benchmark on a fixed table: {left:?}. Delete them from the list - it \
         is the count of what is left to do, so a stale entry overstates it"
    );
}
