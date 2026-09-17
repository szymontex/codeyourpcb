//! Every test-point record sits in its columns.
//!
//! IPC-D-356A is fixed-column text: a tester's software reads a field by where
//! it is, not by what is around it, so a field one character out of place is a
//! file the machine reads as something else. `export --ipc356` writes one
//! record per pad and via and the fields are built by pushing fixed-width
//! pieces - which means a width changed anywhere moves every field after it,
//! in every record at once, with nothing visibly broken.
//!
//! Trailing blanks are not written, so a record is at most 80 columns rather
//! than exactly 80: today they are 74.
//!
//! `cargo test -p cypcb-cli --test every_test_point_record_sits_in_its_columns`

use std::path::{Path, PathBuf};
use std::process::Command;

/// Records a two-layer example writes, today 24.
const RECORDS_FLOOR: usize = 20;

/// The layout's own width. A record may stop short of it and may not pass it.
const LAYOUT_COLUMNS: usize = 80;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The character the layout puts at a column, counted from one.
///
/// Taken from the writer's own column comments in `cypcb-export`'s IPC-D-356
/// module - `A` opens the access field at 39, the two coordinates at 42 and
/// 50, the pad's size at 58 and 63, and its rotation at 68. **These are
/// absolute**, on purpose: a check that only asked whether the records agree
/// with each other would pass a field width changed once, because every
/// record shifts together.
const ANCHORS: &[(usize, char)] = &[
    (39, 'A'),
    (42, 'X'),
    (50, 'Y'),
    (58, 'X'),
    (63, 'Y'),
    (68, 'R'),
];

#[test]
fn every_test_point_record_sits_in_its_columns() {
    let root = repo_root();
    let out = root.join("target/tmp-ipc356-columns");
    let _ = std::fs::remove_dir_all(&out);

    let run = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .current_dir(&root)
        .args([
            "export",
            "examples/blink.cypcb",
            "--output",
            out.to_str().expect("a path this test made"),
            "--ipc356",
        ])
        .output()
        .expect("the binary runs");
    assert!(
        run.status.success(),
        "the netlist did not export: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let file = std::fs::read_dir(out.join("netlist"))
        .expect("the netlist folder is there")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|e| e == "ipc"))
        .expect("a netlist file");
    let text = std::fs::read_to_string(&file).expect("the netlist reads");
    let _ = std::fs::remove_dir_all(&out);

    // A data record is the `3xx` operation code; `P` lines are the header the
    // parameters go in, and `999` ends the file.
    let records: Vec<&str> = text.lines().filter(|line| line.starts_with('3')).collect();

    let longest = records.iter().map(|record| record.len()).max().unwrap_or(0);

    let mut misplaced = Vec::new();
    for record in &records {
        for (column, expected) in ANCHORS {
            let found = record.chars().nth(column - 1);
            if found != Some(*expected) {
                misplaced.push(format!(
                    "column {column} holds {found:?} rather than {expected:?}: {record}"
                ));
            }
        }
    }

    println!(
        "test-point records: {} (floor {RECORDS_FLOOR}); anchors checked per record: {}; \
         fields out of place: {}; longest record {longest} of {LAYOUT_COLUMNS}",
        records.len(),
        ANCHORS.len(),
        misplaced.len()
    );

    assert!(
        records.len() >= RECORDS_FLOOR,
        "the export wrote {} test-point records and the example had {RECORDS_FLOOR} when \
         this was written - a file that stops holding them passes for the wrong reason",
        records.len()
    );
    assert!(
        longest <= LAYOUT_COLUMNS,
        "a record is {longest} columns, past the {LAYOUT_COLUMNS} the layout gives it"
    );

    assert!(
        misplaced.is_empty(),
        "a field is not at the column the layout gives it:\n  {}\n  A tester reads a \
         fixed-column file by where a field is, so one width changed moves every field \
         after it in every record at once.",
        misplaced.join("\n  ")
    );
}
