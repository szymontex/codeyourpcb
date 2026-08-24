//! A board KiCad 10 saved comes back routed.
//!
//! `cargo test -p cypcb-cli --test a_kicad_10_board_comes_back_routed`
//!
//! It did not. `cypcb route` on `kicad10-slotted.kicad_pcb` - a file KiCad
//! 10.0.5 itself wrote - ended at:
//!
//! ```text
//! net NetId(0) is not one of the file's nets, so a segment on it could not be
//! written
//! ```
//!
//! True and useless. Nothing the router did was wrong: **KiCad 10 writes no
//! `(net N "name")` table.** Its pads carry `(net "VBUS")`, the name, so the
//! file has nowhere for a segment to find a number. The routed copy is given
//! the table it lacked, in the place this crate's own board writer puts one.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-kicad10-route-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb");
    let board = dir.join("board.kicad_pcb");
    std::fs::copy(&source, &board).expect("the fixture is copyable");
    board
}

/// Route it, returning the routed file's text.
fn route(board: &Path, out: &Path) -> String {
    let output = cypcb()
        .arg("route")
        .arg(board)
        .arg("--fast")
        .arg("-o")
        .arg(out)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "routing a board KiCad wrote has to work:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(out).expect("the routed board is on disk")
}

#[test]
fn the_routed_copy_declares_the_nets_the_original_did_not() {
    let board = scratch("declares");
    let original = std::fs::read_to_string(&board).expect("the fixture reads");
    assert!(
        !original.contains("(net 0 "),
        "the point of this fixture is that KiCad 10 declared no nets"
    );

    let out = board.with_extension("routed.kicad_pcb");
    let routed = route(&board, &out);

    // Every net the pads name, plus KiCad's unconnected net zero.
    for declaration in ["(net 0 \"\")", "(net 1 \"GND\")", "(net 2 \"VBUS\")"] {
        assert!(
            routed.contains(declaration),
            "the routed copy has to declare `{declaration}`:\n{routed}"
        );
    }

    // Ahead of the first footprint, so every pad that names a net comes after
    // the net exists - which is where this crate's own writer puts a table.
    let table = routed.find("(net 0 \"\")").expect("the table is there");
    let footprint = routed.find("(footprint ").expect("the board has parts");
    assert!(
        table < footprint,
        "the table has to come before the parts that use it"
    );

    assert!(
        routed.contains("(segment "),
        "and the copper the router laid has to be in it"
    );
}

#[test]
fn the_routed_copy_reads_back_as_the_same_board() {
    // The other half: a file this project cannot read again is not a fix.
    let board = scratch("readback");
    let out = board.with_extension("routed.kicad_pcb");
    route(&board, &out);

    let output = cypcb()
        .arg("check")
        .arg(&out)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the routed board has to check clean:\n{said}"
    );

    // The nets came back by name rather than as nets called "1" and "2",
    // which is what reading a numbered `(net 1)` as a name would produce.
    let metadata = cypcb()
        .arg("parse-kicad")
        .arg(&out)
        .output()
        .expect("the binary runs");
    let json = String::from_utf8_lossy(&metadata.stdout).to_string();
    assert!(
        json.contains("\"net_count\""),
        "parse-kicad prints what it found:\n{json}"
    );
    assert!(
        !json.contains("\"1\""),
        "a net named after its own number means the table was read as names:\n{json}"
    );
}
