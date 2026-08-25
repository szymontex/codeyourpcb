//! A copper pour comes back, and so does the name it was given.
//!
//! `cargo test -p cypcb-cli --test a_pour_named_after_its_net_comes_back`
//!
//! A pour is usually named after the net it fills, so a board with a `VBUS+`
//! plane has a zone called `VBUS+` - and the grammar took an **identifier**
//! for a zone name. `zone_net` had already learned about quotes; the name had
//! not, so `from-kicad` wrote `// one zone named "VBUS+" is not written` and
//! the plane was gone. The `GND` pour on the same board came through, which is
//! what made it easy to miss.
//!
//! `zone_definition` takes `net_name` now - the same rule `net` uses - in the
//! grammar, in the hand-written reader and in the writer, so the two pours on
//! the fixture come back as `zone GND` and `zone "VBUS+"`, each in the form
//! the readers accept.
//!
//! The `README` feature table said a pour `has no syntax in the language yet`.
//! It has, and the table says so now.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.code(), said)
}

fn imported(who: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("cypcb-pour-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join("board.cypcb");

    let (code, said) = run(&[
        "from-kicad",
        "tests/fixtures/usb_c_named_pads.kicad_pcb",
        "-o",
        out.to_str().expect("a path that is text"),
    ]);
    assert_eq!(code, Some(0), "the board has to import:\n{said}");

    let source = std::fs::read_to_string(&out).expect("the design was written");
    (out, source)
}

#[test]
fn both_pours_are_in_the_design_that_came_back() {
    let (_, source) = imported("names");

    assert!(
        source.contains("zone GND {"),
        "the ground plane comes back with its name unquoted:\n{source}"
    );
    assert!(
        source.contains("zone \"VBUS+\" {"),
        "and the `VBUS+` plane comes back quoted, rather than as a comment \
         about what was dropped:\n{source}"
    );
    assert!(
        !source.contains("is not written"),
        "nothing on this board is beyond the language now:\n{source}"
    );
}

#[test]
fn the_pours_keep_the_net_and_the_layer_they_were_poured_on() {
    let (_, source) = imported("shape");

    // The ground plane is on the back of the board and the power plane on the
    // front: a zone that comes back on the wrong layer is copper in the wrong
    // place, which is worse than a zone that does not come back at all.
    let ground = source
        .split("zone GND {")
        .nth(1)
        .expect("the ground pour is there")
        .split('}')
        .next()
        .expect("the block closes");
    assert!(
        ground.contains("layer bottom") && ground.contains("net GND"),
        "the ground pour was on B.Cu and on GND:\n{ground}"
    );

    let power = source
        .split("zone \"VBUS+\" {")
        .nth(1)
        .expect("the power pour is there")
        .split('}')
        .next()
        .expect("the block closes");
    assert!(
        power.contains("layer top") && power.contains("net \"VBUS+\""),
        "the power pour was on F.Cu and on VBUS+:\n{power}"
    );
}

#[test]
fn the_design_with_both_pours_is_one_the_checker_reads() {
    let (out, _) = imported("parses");
    let (code, said) = run(&[
        "check",
        "--no-drc",
        out.to_str().expect("a path that is text"),
    ]);
    assert_eq!(
        code,
        Some(0),
        "a design this project wrote and cannot read is a defect in the \
         writer:\n{said}"
    );
}
