//! One contact, one row - and the counts do not move.
//!
//! `cargo test -p cypcb-cli --test one_contact_gets_one_row`
//!
//! `ClearanceRule` compares pairs of *segments* and a trace is a polyline, so
//! two traces running beside each other for 10mm are one fault and as many
//! rows as they have segments in that stretch. Measured on the shipped boards:
//! 759 rows for 484 contacts, and 24 rows for one `U1 <-> trace 'GND'` on
//! `qfp_fanout`. A designer reading that report sees one problem two dozen
//! times.
//!
//! What this test pins is the half of the fix that is safe to make: the
//! **listing** groups, and every **count** stays a count of rows. The header,
//! the per-kind summary and the shorts line feed nothing, but the same numbers
//! appear in `benchmark_validation`'s ratchets, in
//! `cypcb_autoroute::noise_band` and in every table in `docs/routing.md` - a
//! display change that quietly moved them would be a re-baseline pretending to
//! be a tidy-up.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(name)
}

/// Route a benchmark and return what `cypcb check` says about the result.
///
/// A routed board, because an unrouted one has no copper to touch anything -
/// the fixtures ship with none, and the first draft of this test asserted
/// against a report that was entirely `unrouted-pin`.
///
/// `qfp_fanout` rather than `led_blink` or `plane_board`: a fixture has to be
/// able to exhibit the thing this test checks, and a board whose faults are
/// one row each can never show a grouped row. `led_blink` routes to one
/// clearance fault; `plane_board` used to route to several rows over fewer
/// contacts and stopped on 2026-08-28, when the router's pad obstacles became
/// rectangles and its faults fell to eight rows over eight contacts. The
/// fine-pitch board is the one whose copper runs beside itself for millimetres
/// at a time - this file's own opening measured 24 rows for one `U1 <-> trace
/// 'GND'` on it.
fn check_a_routed_board(tag: &str, name: &str) -> String {
    // A file per caller. These run in parallel and the first draft gave them
    // one shared path, so a test could route into a file another test had just
    // deleted - it failed and passed on the same code depending on timing.
    // The same collision this crate already hit once, in
    // `the_table_matches_the_board_not_just_the_house`.
    let out = std::env::temp_dir().join(format!("cypcb-one-row-{tag}-{name}"));
    let routed = Command::new(cypcb_binary())
        .arg("route")
        .arg(fixture(name))
        .arg("--in-house")
        .arg("-o")
        .arg(&out)
        .output()
        .expect("the CLI runs");
    assert!(
        routed.status.success(),
        "routing failed: {}",
        String::from_utf8_lossy(&routed.stderr)
    );

    let checked = Command::new(cypcb_binary())
        .arg("check")
        .arg(&out)
        .output()
        .expect("the CLI runs");
    let _ = std::fs::remove_file(&out);
    format!(
        "{}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    )
}

/// What the summary says a kind's row count is.
fn summary_count(report: &str, kind: &str) -> usize {
    report
        .lines()
        .find_map(|line| line.trim().strip_prefix(&format!("{kind}: ")))
        .and_then(|rest| rest.trim().parse().ok())
        .unwrap_or_else(|| panic!("no summary line for {kind} in:\n{report}"))
}

#[test]
fn the_listing_groups_and_the_summary_does_not() {
    const TAG: &str = "listing";
    let report = check_a_routed_board(TAG, "qfp_fanout.kicad_pcb");

    let rows = summary_count(&report, "clearance");
    let printed = report
        .lines()
        .filter(|line| line.contains("clearance at ("))
        .count();
    let notes = report
        .lines()
        .filter(|line| line.contains("more place(s)"))
        .count();

    assert!(
        rows > 0,
        "the routed board should have clearance faults:\n{report}"
    );
    assert!(
        printed < rows,
        "the listing has to be shorter than the row count, or nothing was \
         grouped: {printed} rows printed against {rows} counted\n{report}"
    );
    assert!(
        notes > 0,
        "a grouped row has to say it stands for more than itself\n{report}"
    );
}

#[test]
fn every_grouped_row_says_how_many_it_stands_for() {
    const TAG: &str = "accounted";
    let report = check_a_routed_board(TAG, "qfp_fanout.kicad_pcb");

    let rows = summary_count(&report, "clearance");
    let printed = report
        .lines()
        .filter(|line| line.contains("clearance at ("))
        .count();
    let hidden: usize = report
        .lines()
        .filter_map(|line| line.trim().strip_prefix("and "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|count| count.parse::<usize>().ok())
        .sum();

    assert_eq!(
        printed + hidden,
        rows,
        "every row is either printed or counted in a note; {printed} printed \
         plus {hidden} noted is not {rows}\n{report}"
    );
}

#[test]
fn the_header_still_counts_rows() {
    // The number the ratchets and the bands are made of. If grouping ever
    // reaches it, every published figure in this project moves at once.
    const TAG: &str = "header";
    let report = check_a_routed_board(TAG, "qfp_fanout.kicad_pcb");
    let header: usize = report
        .lines()
        .find_map(|line| line.split(" DRC violation(s)").next()?.trim().parse().ok())
        .unwrap_or_else(|| panic!("no header count in:\n{report}"));

    // Only the per-kind lines. The block ends with a sentence about copper
    // touching copper, which also parses as `<something>: <number>` and would
    // be added twice - the first draft of this test did exactly that.
    let by_kind: usize = report
        .lines()
        .skip_while(|line| !line.starts_with("Summary:"))
        .filter_map(|line| line.split_once(": "))
        .filter(|(kind, _)| !kind.trim().contains(' '))
        .filter_map(|(_, count)| count.trim().parse::<usize>().ok())
        .sum();

    assert_eq!(
        header, by_kind,
        "the header and the per-kind summary are both row counts\n{report}"
    );
}

#[test]
fn the_summary_reconciles_the_two_numbers() {
    // The listing prints one clearance row per contact and the summary counts
    // rows, so on a routed board they disagree. A reader with no line
    // explaining that is left to work out which of the two is the board.
    const TAG: &str = "reconciled";
    let report = check_a_routed_board(TAG, "qfp_fanout.kicad_pcb");

    let rows = summary_count(&report, "clearance");
    let printed = report
        .lines()
        .filter(|line| line.contains("clearance at ("))
        .count();

    let line = report
        .lines()
        .find(|line| line.contains("clearance rows describe"))
        .unwrap_or_else(|| panic!("no reconciling line in:\n{report}"));

    assert!(
        line.contains(&format!("{rows} clearance rows")),
        "the line has to quote the summary's own count of {rows}: {line}"
    );
    assert!(
        line.contains(&format!("describe {printed} contacts")),
        "and the number of rows it actually listed, {printed}: {line}"
    );
}

#[test]
fn a_board_with_nothing_to_group_says_nothing() {
    // Every contact reported once means the two numbers agree, and a sentence
    // reconciling them would be noise. `led_blink` routes to one clearance
    // fault: one row, one contact.
    const TAG: &str = "quiet";
    let report = check_a_routed_board(TAG, "led_blink.kicad_pcb");
    assert!(
        !report.contains("clearance rows describe"),
        "nothing was grouped, so there is nothing to reconcile:\n{report}"
    );
}
