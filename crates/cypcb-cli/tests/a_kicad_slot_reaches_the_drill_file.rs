//! A slot in a board KiCad wrote reaches the drill file as a slot.
//!
//! `cargo test -p cypcb-cli --test a_kicad_slot_reaches_the_drill_file`
//!
//! `kicad10-slotted.kicad_pcb` exists because a slot is the shape this project
//! got wrong more than once: a milled opening is a hole with a length, and
//! naming its drill alone describes a round hole under half the size of the
//! one the part needs. The export crate holds the writer's end of that with a
//! world it builds itself; nothing held the whole way through - a file KiCad
//! saved, read, exported, and the slot still a slot at the far end.
//!
//! `cypcb export` on that board was measured and is sound. This is what keeps
//! it that way.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb")
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-kicad-export-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn export(out: &Path, extra: &[&str]) -> String {
    let output = cypcb()
        .arg("export")
        .arg(fixture())
        .arg("-o")
        .arg(out)
        .args(extra)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "exporting a board KiCad wrote has to work:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Both streams: the progress lines speak on stderr and the dry run's file
    // list on stdout, and this helper's callers read one or the other.
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn the_oval_hole_is_milled_rather_than_drilled() {
    let out = scratch("slot");
    export(&out, &[]);

    let drill = out.join("drill/kicad10-slotted-PTH.drl");
    let text = std::fs::read_to_string(&drill).expect("the plated drill file is on disk");

    // `(drill oval 2.4 1)` in the source: a 1mm tool travelling 1.4mm, which
    // Excellon writes as a route between two points rather than as a hit.
    assert!(
        text.contains("G85"),
        "the slot has to be milled, not punched as a round hole:\n{text}"
    );
    assert_eq!(
        text.matches("G85").count(),
        2,
        "the board carries two slotted pads:\n{text}"
    );
    assert!(
        text.contains("T1C1.000000"),
        "the tool is the slot's narrow dimension, which is what mills it:\n{text}"
    );

    // And the round holes are still hits rather than routes.
    assert!(
        text.contains("T2C0.900000"),
        "the two 0.9mm holes keep their own tool:\n{text}"
    );
}

#[test]
fn the_dry_run_names_this_boards_files_too() {
    // The census from the `.cypcb` side, asked of a KiCad input: the listing
    // and the file set have to agree here as well, and nothing had checked.
    let out = scratch("census");
    let said = export(&out, &["--dry-run"]);

    let prefix = format!("{}/", out.display());
    let mut listed: Vec<String> = said
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(str::to_string))
        .collect();
    listed.sort();
    assert!(
        !out.exists(),
        "a dry run must not write anything, and it created {}",
        out.display()
    );

    export(&out, &[]);
    let mut written = Vec::new();
    let mut stack = vec![out.clone()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).expect("the output directory is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                written.push(
                    path.strip_prefix(&out)
                        .expect("everything found is under it")
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    written.sort();

    assert_eq!(listed, written, "listed against written, for a KiCad board");
}
