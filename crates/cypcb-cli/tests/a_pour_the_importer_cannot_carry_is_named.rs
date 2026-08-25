//! A pour the importer cannot carry is named, on the command line too.
//!
//! `cargo test -p cypcb-cli --test a_pour_the_importer_cannot_carry_is_named`
//!
//! `parse_kicad_pcb` refuses a pour it cannot state - a ground plane cut
//! around a connector is not a rectangle, and a bounding box would put copper
//! where the shape was drawn to avoid it - and it says why, in
//! `metadata.zone_refusals`. `cypcb-render` has printed those reasons in the
//! browser since they existed. `cypcb from-kicad` did not: the fixture below
//! came through with two of its three pours and the command said
//! `Wrote ... 2 component(s), 3 net(s), 2 footprint definition(s)` and nothing
//! else.
//!
//! A plane lost in silence is the failure this project keeps writing tests
//! about, one command at a time.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Import the fixture, returning what the command said and what it wrote.
fn import(who: &str) -> (String, String) {
    let dir = std::env::temp_dir().join(format!("cypcb-refused-pour-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join("board.cypcb");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "from-kicad",
            "tests/fixtures/usb_c_named_pads.kicad_pcb",
            "-o",
            out.to_str().expect("a path that is text"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a board with a shape this importer will not guess at is still a board \
         it imports:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    let source = std::fs::read_to_string(&out).expect("the design was written");
    (said, source)
}

#[test]
fn the_command_says_which_pour_it_could_not_carry_and_why() {
    let (said, _) = import("named");

    assert!(
        said.contains("the zone on net GND is a 6-point outline"),
        "the fixture's L-shaped ground plane has to be named:\n{said}"
    );
    assert!(
        said.contains("bounding box would put copper where the shape was drawn to avoid it"),
        "and the reason has to travel with it, because it is the reason not to \
         approximate:\n{said}"
    );
}

#[test]
fn the_pours_it_can_carry_still_arrive() {
    let (_, source) = import("carried");

    // One refused pour does not cost the board the other two.
    assert!(
        source.contains("zone GND {") && source.contains("zone \"VBUS+\" {"),
        "the two rectangular pours are still in the design:\n{source}"
    );
    assert_eq!(
        source.matches("zone ").count(),
        2,
        "two pours are carried and one is refused, so the design has exactly \
         two:\n{source}"
    );
}
