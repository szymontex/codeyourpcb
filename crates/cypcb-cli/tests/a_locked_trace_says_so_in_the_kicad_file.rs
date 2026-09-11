//! Does a locked trace stay locked in the file KiCad opens?
//!
//! `cargo test -p cypcb-cli --test a_locked_trace_says_so_in_the_kicad_file -- --nocapture`
//!
//! `locked` is the only field in this model that carries a person's instruction
//! rather than a measurement: it says do not move this copper. The KiCad writer
//! never emitted it - `grep -c locked` over the whole writer returned 0 - so a
//! design that pinned a trace was exported to a file that did not say so, and
//! anybody opening it in KiCad could drag the copper the designer had fixed.
//!
//! The token was read rather than assumed. The board file format defines it as
//! a bare `(locked)` for a track segment, a track arc and a via, in each case
//! after the layer and before the net, for every version from 6.0 (KiCad board
//! file format documentation, read 2026-09-11). This file once wrote a
//! `(setup (rules ...))` node pcbnew refused to open at all, which is why the
//! position is checked here and not only the presence.

use std::path::Path;

use cypcb_kicad::write_board;

fn board_from(example: &str) -> cypcb_world::BoardWorld {
    board_from_source(example, |source| source)
}

/// The same example with its text put through `edit` first.
///
/// The control below needs a board that has segments and pins none of them.
/// The obvious candidate was another example, and it turned out to write zero
/// segments - so "no `(locked)` in the file" held with nothing in the
/// denominator, which is not a control at all. Removing the one word from the
/// one design that has it gives a true pair: same board, same segment count,
/// one keyword apart.
fn board_from_source(example: &str, edit: impl Fn(String) -> String) -> cypcb_world::BoardWorld {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples")
        .join(example);
    let source = edit(std::fs::read_to_string(&path).expect("the example is there"));
    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{} does not parse", example);
    let mut import_errors = Vec::new();
    let ast = cypcb_parser::resolve_imports(&parsed.value, &path, &mut import_errors);
    assert!(import_errors.is_empty(), "{} has import errors", example);

    let mut world = cypcb_world::BoardWorld::new();
    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    let sync = cypcb_world::sync::sync_ast_to_world(&ast, &source, &mut world, &mut library);
    assert!(
        sync.errors.is_empty(),
        "{} does not sync: {:?}",
        example,
        sync.errors
    );
    world
}

/// Every `(segment ...)` line in the file.
fn segments(file: &str) -> Vec<&str> {
    file.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("(segment "))
        .collect()
}

#[test]
fn a_pinned_trace_is_pinned_in_the_file() {
    let mut world = board_from("uat-routing-locked.cypcb");
    let file = write_board(&mut world, "cypcb");

    let lines = segments(&file);
    assert!(
        !lines.is_empty(),
        "the example has a trace, so the file has segments"
    );
    let locked: Vec<&&str> = lines
        .iter()
        .filter(|line| line.contains("(locked)"))
        .collect();
    println!("{} segments, {} of them locked", lines.len(), locked.len());
    for line in &locked {
        println!("  {line}");
    }

    assert!(
        !locked.is_empty(),
        "the design pins a trace and the file says nothing about it.\n  \
         first segment written: {}",
        lines[0]
    );

    // Position, not just presence. The format puts the token after the layer
    // and before the net, and a token in the wrong place is a file pcbnew
    // refuses rather than a file that says less.
    for line in &locked {
        let layer = line.find("(layer").expect("a segment states its layer");
        let lock = line.find("(locked)").expect("checked above");
        let net = line.find("(net ").expect("a segment states its net");
        assert!(
            layer < lock && lock < net,
            "the token sits outside the place the format defines for it: {line}"
        );
    }
}

#[test]
fn a_trace_nobody_pinned_carries_no_token() {
    // The control. Without it the assertion above would pass against a writer
    // that emitted `(locked)` on every segment it wrote, which is the same
    // defect in the other direction - a file claiming the designer pinned
    // copper they never touched.
    //
    // The same design with the keyword removed, so the two runs differ by one
    // word and by nothing else. Both write the same segment, which is what
    // makes this a control rather than an empty file.
    let mut world = board_from_source("uat-routing-locked.cypcb", |source| {
        source.replace("\n    locked", "")
    });
    let file = write_board(&mut world, "cypcb");

    let lines = segments(&file);
    println!("{} segments with the keyword removed", lines.len());
    for line in &lines {
        println!("  {line}");
    }
    assert_eq!(
        lines.len(),
        1,
        "the control has to write the same copper as the test above, or it is \
         comparing two different boards"
    );
    assert!(
        !file.contains("(locked)"),
        "a design that pins nothing must not produce a file that pins something"
    );
}
