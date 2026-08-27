//! `--ipc356` writes the file a flying probe reads.
//!
//! `cargo test -p cypcb-cli --test the_netlist_a_bare_board_is_tested_against`
//!
//! Item 6 of the KiCad parity audit. Before anything is soldered, a fabricator
//! probes the bare board and checks it against the design's own netlist: every
//! point that should be connected, and no two that should not be. IPC-D-356A
//! is how that netlist travels, and it is fixed-column - a tester reads by
//! position, so a field one column out is a file that describes a different
//! board. That is what these assertions are: the columns, by number.

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
    let dir = std::env::temp_dir().join(format!("cypcb-ipc356-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export the board with the netlist and read the netlist back.
fn netlist(board: &Path, out: &Path) -> String {
    let status = cypcb()
        .arg("export")
        .arg(board)
        .arg("-o")
        .arg(out)
        .arg("--ipc356")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let dir = out.join("netlist");
    let file = std::fs::read_dir(&dir)
        .expect("the netlist directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-IPC-D-356.ipc"))
        })
        .expect("a netlist was written");
    std::fs::read_to_string(file).expect("the netlist is readable")
}

/// Columns as the format numbers them: 1-based, inclusive.
fn columns(line: &str, from: usize, to: usize) -> &str {
    &line[from - 1..to]
}

#[test]
fn the_file_has_the_header_the_format_requires() {
    let text = netlist(&example("usb-diff-pair.cypcb"), &scratch("header"));
    let lines: Vec<&str> = text.lines().collect();

    assert!(
        lines[0].starts_with("P  JOB   "),
        "the header starts with the job: {}",
        lines[0]
    );
    assert!(
        text.contains("P  UNITS CUST 1"),
        "the units are declared, and they are millimetres:\n{text}"
    );
    assert!(
        text.contains("P  IMAGE PRIMARY"),
        "the netlist section is opened:\n{text}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("999"),
        "the file ends with the end-of-job record"
    );
}

#[test]
fn every_pad_on_a_net_is_a_record_in_the_right_columns() {
    let text = netlist(&example("usb-diff-pair.cypcb"), &scratch("columns"));
    let records: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("317") || line.starts_with("327"))
        .collect();

    // Four headers, one pin each on a net: J1.1, J2.1, J3.1, J4.1.
    assert_eq!(
        records.len(),
        4,
        "one record per pad that is on a net:\n{text}"
    );

    for line in &records {
        assert!(
            line.len() <= 80,
            "a record is at most eighty columns and this is {}: {line}",
            line.len()
        );
    }

    // The first record, read field by field at the positions the format fixes.
    let first = records[0];
    assert_eq!(columns(first, 1, 3), "317", "a through-hole pad is a 317");
    assert_eq!(
        columns(first, 4, 17),
        "USB_DM        ",
        "the net name is fourteen columns, left justified"
    );
    assert_eq!(columns(first, 18, 20), "   ", "columns 18-20 are blank");
    assert_eq!(columns(first, 21, 26), "J3    ", "the reference designator");
    assert_eq!(columns(first, 27, 27), "-", "the dash the format requires");
    assert_eq!(columns(first, 28, 31), "1   ", "the pin");
    assert_eq!(columns(first, 33, 33), "D", "a drilled hole says so");
    assert_eq!(
        columns(first, 34, 37),
        "1000",
        "the hole is 1.000mm, in microns"
    );
    assert_eq!(columns(first, 38, 38), "P", "and it is plated");
    assert_eq!(
        columns(first, 39, 41),
        "A00",
        "a through-hole pad is reachable from both sides"
    );
    assert_eq!(columns(first, 42, 42), "X");
    assert_eq!(
        columns(first, 43, 49),
        "+005000",
        "5mm in thousandths of a millimetre, signed"
    );
    assert_eq!(columns(first, 50, 50), "Y");
    assert_eq!(columns(first, 51, 57), "+011730");
    assert_eq!(columns(first, 58, 58), "X");
    assert_eq!(columns(first, 59, 62), "1700", "the pad is 1.7mm across");
    assert_eq!(columns(first, 63, 63), "Y");
    assert_eq!(columns(first, 64, 67), "1700");
    assert_eq!(columns(first, 68, 68), "R");
    assert_eq!(
        columns(first, 69, 71),
        "090",
        "the part is rotated ninety degrees"
    );
    assert_eq!(columns(first, 73, 74), "S3", "soldermasked on both sides");
}

#[test]
fn the_records_are_sorted_by_net() {
    let text = netlist(&example("usb-diff-pair.cypcb"), &scratch("sorted"));
    let nets: Vec<String> = text
        .lines()
        .filter(|line| line.starts_with("317") || line.starts_with("327"))
        .map(|line| columns(line, 4, 17).trim_end().to_string())
        .collect();
    let mut sorted = nets.clone();
    sorted.sort();
    assert_eq!(
        nets, sorted,
        "the netlist section is sorted by net: {nets:?}"
    );
}

#[test]
fn a_board_that_does_not_ask_gets_no_netlist() {
    let out = scratch("silent");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert!(
        !out.join("netlist").exists(),
        "the file set a house receives is unchanged unless the netlist is asked for"
    );
}
