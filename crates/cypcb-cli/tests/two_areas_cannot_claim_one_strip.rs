//! Two stacks over one strip of board.
//!
//! `cargo test -p cypcb-cli --test two_areas_cannot_claim_one_strip`
//!
//! A rigid-flex build is several stacks on one panel, each stated against an
//! area: `core 1mm covers left`, `core 0.5mm covers right`. Where two such
//! areas overlap, the handoff document writes a `StackupGroup` for each and a
//! fabricator is told the board is two thicknesses in one place.
//!
//! There is no right answer to pick - which stack owns the contested strip is
//! the designer's decision - so the checker's job is to refuse to let it pass
//! quietly, the way `zone-overlap` does for two planes over one patch of
//! copper.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Check a board and hand back everything it printed.
///
/// The directory carries none of the rule's words: every report prints the
/// board's path, so a directory named after the rule would make
/// `contains("area-overlap")` true for a board with nothing wrong with it.
fn check(source: &str, who: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-two-stacks-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", board.to_str().expect("a path that is text")])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// A board with two areas, whose stack and rectangles are the variables.
fn board(stack: &str, right_starts_at: u32) -> String {
    format!(
        "version 1\n\n\
         board panel {{\n    size 60mm x 16mm\n    layers 2\n    stackup {{\n{stack}\n    }}\n}}\n\n\
         region left {{\n    bounds 0mm, 0mm to 30mm, 16mm\n    layer all\n}}\n\n\
         region right {{\n    bounds {right_starts_at}mm, 0mm to 60mm, 16mm\n    layer all\n}}\n"
    )
}

/// Two builds, one per end, differing by half a millimetre of core.
const TWO_STACKS: &str = "        copper 0.035mm\n        core 1mm covers left\n        copper 0.035mm\n        core 0.5mm covers right";

#[test]
fn the_contested_strip_is_measured_and_both_builds_are_named() {
    let said = check(&board(TWO_STACKS, 20), "overlap");
    assert!(said.contains("area-overlap"), "{said}");
    assert!(
        said.contains(
            "'left' and 'right' both cover 10.000mm by 16.000mm of board from (20.000mm, 0.000mm)"
        ),
        "the strip itself, measured:\n{said}"
    );
    assert!(
        said.contains("one stack is 1.070mm thick and the other 0.570mm"),
        "and what each build says the board is there:\n{said}"
    );
}

#[test]
fn two_areas_that_meet_at_an_edge_are_not_overlapping() {
    // `left` ends at 30mm and `right` starts at 30mm: they share an edge and
    // no board. This is how a rigid-flex panel is ordinarily drawn, so a rule
    // that reported it would fire on every correct design.
    let said = check(&board(TWO_STACKS, 30), "abutting");
    assert!(
        !said.contains("area-overlap"),
        "an edge is not a strip:\n{said}"
    );
    assert!(said.contains("passed DRC"), "{said}");
}

#[test]
fn areas_no_stack_points_at_may_overlap_freely() {
    // Same two rectangles, overlapping by 10mm, and a stack that names
    // neither. Nothing downstream writes a second group for them, so there is
    // nothing to be ambiguous about - and reporting it would be noise about a
    // fact no document carries.
    let plain = "        copper 0.035mm\n        core 1mm\n        copper 0.035mm";
    let said = check(&board(plain, 20), "unnamed");
    assert!(
        !said.contains("area-overlap"),
        "two rectangles nothing points at are two rectangles:\n{said}"
    );
}

#[test]
fn areas_beside_the_ones_the_stack_names_are_left_alone() {
    // The stack names two areas that meet at an edge and nothing else, and the
    // board draws two more that overlap by 10mm. Nothing writes a second stack
    // for those two, so nothing is ambiguous about them - and a rule that
    // measured every rectangle on the board would report a design that is
    // right.
    let source = format!(
        "{}\nregion scratch_a {{\n    bounds 0mm, 0mm to 40mm, 8mm\n    layer all\n}}\n\n         region scratch_b {{\n    bounds 30mm, 0mm to 60mm, 8mm\n    layer all\n}}\n",
        board(TWO_STACKS, 30)
    );
    let said = check(&source, "beside");
    assert!(
        !said.contains("area-overlap"),
        "only the areas a stack points at are measured:\n{said}"
    );
}

#[test]
fn two_builds_of_the_same_thickness_still_overlap_and_the_message_says_less() {
    // The strip is still described twice - two groups, two builds - but there
    // is no difference in thickness to report, and a message that invented one
    // would be wrong.
    let same = "        copper 0.035mm\n        core 1mm covers left\n        copper 0.035mm\n        core 1mm covers right";
    let said = check(&board(same, 20), "same-thickness");
    assert!(said.contains("area-overlap"), "{said}");
    assert!(
        !said.contains("thick and the other"),
        "two stacks of one thickness have no difference to name:\n{said}"
    );
}
