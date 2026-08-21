//! A board is checked against the table its layer count is built on.
//!
//! `cargo test -p cypcb-cli --test the_table_matches_the_board_not_just_the_house`
//!
//! `board b { fab jlcpcb }` names a **house**, and a house publishes one table
//! per layer count. `fab jlcpcb` on a four-layer board resolved to
//! `jlcpcb_standard_2layer` - checked against 0.127mm trace and space where
//! the page says 0.09mm for multilayer, and against a 0.18mm annular ring
//! where it says 0.20mm. Too strict on the copper and too loose on the ring,
//! in the fab's own name.
//!
//! A name that states a layer count is not touched: somebody who writes
//! `--preset jlcpcb_standard_2layer` on a four-layer board is asking a
//! specific question, the same way the flag already wins over the design.

use std::path::PathBuf;
use std::process::Command;

fn cypcb_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cypcb"))
}

fn board(layers: u8, fab: &str) -> String {
    format!(
        "version 1\n\n\
         board t {{\n    size 40mm x 20mm\n    layers {layers}\n    fab {fab}\n}}\n\n\
         component R1 resistor \"0402\" {{\n    value 10kohm\n    at 10mm, 10mm\n}}\n"
    )
}

/// The table `cypcb check` says it used.
fn table_used(source: &str) -> String {
    let dir = std::env::temp_dir().join("cypcb-preset-by-layers");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    // Named from the content, because these run in parallel and two boards of
    // the same length were sharing a file - which is how the first draft of
    // this test reported pcbway_standard for a jlcpcb board.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let path = dir.join(format!("{hash:016x}.cypcb"));
    std::fs::write(&path, source).expect("the board is writable");

    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&path)
        .output()
        .expect("the CLI runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    report
        .lines()
        .find_map(|line| line.split("against ").nth(1))
        .map(|rest| rest.trim_end_matches(':').to_string())
        .unwrap_or_else(|| panic!("no table named in:\n{report}"))
}

#[test]
fn a_house_name_resolves_to_the_table_the_board_is_built_on() {
    assert_eq!(table_used(&board(2, "jlcpcb")), "jlcpcb_standard_2layer");
    assert_eq!(table_used(&board(4, "jlcpcb")), "jlcpcb_standard_4layer");
}

#[test]
fn the_other_house_with_two_tables_follows_the_same_rule() {
    assert_eq!(table_used(&board(2, "oshpark")), "oshpark_2layer");
    assert_eq!(table_used(&board(4, "oshpark")), "oshpark_4layer");
}

#[test]
fn a_house_that_publishes_one_table_keeps_it_at_any_layer_count() {
    // PCBWay states no layer split, and neither do the IPC classes. Inventing
    // a four-layer sibling for them would be this tool making up a table.
    assert_eq!(table_used(&board(2, "pcbway")), "pcbway_standard");
    assert_eq!(table_used(&board(4, "pcbway")), "pcbway_standard");
    assert_eq!(table_used(&board(4, "ipc_class2")), "ipc_class2");
}

#[test]
fn a_name_that_states_a_layer_count_is_taken_as_written() {
    // The design asking for a specific table on a board of another size is a
    // question, not a mistake to correct.
    assert_eq!(
        table_used(&board(4, "jlcpcb_standard_2layer")),
        "jlcpcb_standard_2layer"
    );
}

#[test]
fn a_board_that_names_no_fab_still_gets_the_right_size_of_default() {
    // The default has always been JLCPCB. A four-layer board with no fab line
    // was getting the two-layer one, which is the same defect one step over.
    let plain = "version 1\n\nboard t {\n    size 40mm x 20mm\n    layers 4\n}\n\n\
                 component R1 resistor \"0402\" {\n    value 10kohm\n    at 10mm, 10mm\n}\n";
    assert_eq!(table_used(plain), "jlcpcb_standard_4layer");
}
