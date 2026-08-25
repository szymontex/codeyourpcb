//! A via's span is a hole somebody has to drill.
//!
//! `cargo test -p cypcb-cli --test a_via_span_the_build_does_not_drill`
//!
//! A board is drilled and plated once per lamination cycle. A through hole is
//! drilled after the last press and reaches everything; a **blind** via
//! reaches an outer layer and stops inside; a **buried** one touches neither
//! face. The last two mean the board is drilled and plated more than once,
//! with a press in between, so a house prices them separately and many refuse
//! them outright.
//!
//! `ViaSpanRule` asks two questions, and a design can be wrong about either:
//! whether this house drills such holes at all, and whether the span is one
//! the design's own `drill` pairs list. It was one of six rules nothing ran -
//! and it is the rule V8's drill-pair vocabulary exists for.
//!
//! `examples/blind-via.cypcb` is the board that keeps the other half honest:
//! `fab pcbway`, two `drill` pairs stated, and a via whose span is on the
//! list.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A four-layer board with one via from the top face to the first inner layer.
///
/// `{FAB}` names the house and `{DRILLS}` is what the stack says it drills.
const BOARD: &str = r#"version 1

board spans {
    size 30mm x 20mm
    layers 4
    fab {FAB}

    stackup {
        copper "F.Cu" 1oz
        prepreg "dielectric 1" 0.2mm material "7628" dk 4.5
        copper "In1.Cu" 0.5oz
        core "dielectric 2" 1.0mm material "FR4" dk 4.6
        copper "In2.Cu" 0.5oz
        prepreg "dielectric 3" 0.2mm material "7628" dk 4.5
        copper "B.Cu" 1oz
{DRILLS}    }
}

component J1 connector "PIN-HDR-1x2" {
    value "in"
    at 5mm, 10mm
    rotate 90
}

component J2 connector "PIN-HDR-1x2" {
    value "out"
    at 25mm, 10mm
    rotate 90
}

net SIG {
    J1.1
    J2.1
}

trace SIG {
    from J1.1
    via 15mm, 10mm layers Top to Inner1
    to J2.1
    layer Top
    width 0.2mm
}
"#;

fn check(who: &str, fab: &str, drills: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-spans-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    let source = BOARD.replace("{FAB}", fab).replace("{DRILLS}", drills);
    std::fs::write(&board, source).expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_house_that_does_not_drill_blind_vias_is_told_about_one() {
    // JLCPCB's standard process is through holes only, and this board asks for
    // a hole that stops inside it.
    let said = check("jlc", "jlcpcb", "        drill Top to Inner1\n");

    assert!(
        said.contains("via-span"),
        "a blind via on a table that drills none has to be reported:\n{said}"
    );
    assert!(
        said.contains("is a blind via and this table does not drill them"),
        "and the report has to say which kind of hole it is, because that is \
         what the house prices:\n{said}"
    );
}

#[test]
fn a_span_the_stack_does_not_list_is_reported_against_the_list() {
    // PCBWay does drill blind vias, so the only thing wrong here is that the
    // design's own stack says this build drills through holes and nothing
    // else.
    let said = check("pcbway", "pcbway", "        drill Top to Bottom\n");

    assert!(
        said.contains("is not a span this build drills"),
        "the stack lists the spans this build makes and this is not one \
         of them:\n{said}"
    );
    assert!(
        said.contains("the stackup states Top to Bottom"),
        "and the report has to quote the list, or a reader cannot tell what to \
         add:\n{said}"
    );
}

#[test]
fn a_span_on_the_list_at_a_house_that_drills_it_is_quiet() {
    // Both questions answered the right way: the house drills blind vias and
    // the stack states this span.
    let said = check("ok", "pcbway", "        drill Top to Inner1\n");
    assert!(
        !said.contains("via-span"),
        "this hole is one the build makes:\n{said}"
    );
}

#[test]
fn the_shipped_blind_via_example_stays_quiet() {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/blind-via.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("via-span"),
        "the example states its spans and uses one of them:\n{said}"
    );
}
