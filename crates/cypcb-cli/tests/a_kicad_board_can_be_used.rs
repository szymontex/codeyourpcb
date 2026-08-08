//! The importer served tests and nothing else.
//!
//! `cargo test -p cypcb-cli --test a_kicad_board_can_be_used`
//!
//! `parse_kicad_pcb` had been fixed five times - footprint libraries keyed by
//! geometry rather than by name, malformed coordinates refused instead of read
//! as zero, `np_thru_hole` understood so a mounting hole is not plated, copper
//! pours carried - and every one of those fixes served benchmarks. The product
//! could not open a KiCad board at all:
//!
//! ```text
//! $ cypcb check board.kicad_pcb
//! cypcb::parse::missing
//!   × Missing a definition
//!    ╭─[1:1]
//!  1 │ (kicad_pcb (version 20240108) (generator "pcbnew") ...
//! ```
//!
//! The file went to the DSL parser, which said the only thing it could about a
//! language it was not written for.
//!
//! These run the real binary rather than the library, because the thing that
//! was broken was the command, not the reader underneath it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(name)
}

#[test]
fn check_reads_a_kicad_board_instead_of_failing_to_parse_it() {
    let output = cypcb()
        .arg("check")
        .arg(fixture("plane_board.kicad_pcb"))
        .output()
        .expect("the binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !combined.contains("Missing a definition"),
        "the KiCad board went to the DSL parser:\n{combined}"
    );
    assert!(
        combined.contains("DRC violation") || combined.contains("passed DRC"),
        "check has to reach the design rule check on a KiCad board:\n{combined}"
    );
    // The board's own parts, named in the report - proof the model came from
    // this file rather than from an empty world that trivially passes.
    assert!(
        combined.contains("unrouted-pin") && combined.contains("U1"),
        "the report has to be about the parts in the file:\n{combined}"
    );
}

#[test]
fn the_copper_already_in_the_file_is_kept() {
    // `led_blink.kicad_pcb` carries three segments and two vias. Dropping them
    // on import would read as an unrouted board, and every pin they connect
    // would be reported as unreached - a checker lying about a board that is
    // partly routed already.
    let output = cypcb()
        .arg("check")
        .arg(fixture("led_blink.kicad_pcb"))
        .output()
        .expect("the binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let unreached = combined
        .lines()
        .filter(|line| line.contains("unrouted-pin at"))
        .count();

    // Seven parts, fourteen pads. With the file's copper applied some pins are
    // reached; with it dropped every single one would be unreached.
    assert!(
        unreached > 0,
        "this board is only partly routed, so some pins are unreached:\n{combined}"
    );
    assert!(
        unreached < 14,
        "every pin came back unreached, so the copper in the file was dropped: \
         {unreached} of 14\n{combined}"
    );
}

#[test]
fn a_kicad_board_exports_to_fabrication_files() {
    let out_dir = std::env::temp_dir().join("cypcb-kicad-export");
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = cypcb()
        .arg("export")
        .arg(fixture("plane_board.kicad_pcb"))
        .arg("-o")
        .arg(&out_dir)
        .arg("--preset")
        .arg("jlcpcb")
        .output()
        .expect("the binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "exporting a KiCad board failed:\n{combined}"
    );

    let bottom = out_dir.join("gerber").join("plane_board-B_Cu.gbr");
    let gerber = std::fs::read_to_string(&bottom)
        .unwrap_or_else(|e| panic!("{} is readable: {e}\n{combined}", bottom.display()));

    // The pour the file carries has to reach the layer it was poured on. `G36`
    // opens a filled region; a bottom layer with none of them is a board whose
    // ground plane was lost somewhere between the file and the fabricator.
    let regions = gerber
        .lines()
        .filter(|line| line.starts_with("G36"))
        .count();
    assert!(
        regions > 0,
        "the imported ground plane did not reach the bottom copper:\n{gerber}"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}
