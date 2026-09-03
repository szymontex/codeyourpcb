//! A rounded pad reaches the Gerber round.
//!
//! `cargo test -p cypcb-cli --test a_rounded_pad_reaches_the_gerber_round`
//!
//! Gerber has no rounded-rectangle aperture, and the writer used to answer
//! that by flashing a hard-cornered `R` with the corner written after it:
//!
//! ```text
//! %ADD10R,2.000000X1.000000*% G04 RoundRect corner_ratio=25%
//! ```
//!
//! Two faults in one line. The corners are gone from the copper, so the board
//! a fab builds is not the board that was drawn. And the trailing text is not
//! a comment: a `G04` ends at a `*`, and that final `%` opens an extended
//! command nothing closes.
//!
//! The format's own answer is the aperture macro, which is what KiCad writes
//! for the same pad. These cases hold both halves: the macro is there with the
//! geometry worked out, and nothing in the file is an unterminated statement.

use std::path::PathBuf;
use std::process::Command;

/// A 2mm by 1mm pad with quarter-rounded corners, so the radius is
/// `min(2, 1) * 25% = 0.25mm` and every number below is checkable by hand.
const BOARD: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
     footprint RR {\n    courtyard 3mm x 2mm\n    \
     pad 1 roundrect at 0mm, 0mm size 2mm x 1mm corner 25%\n}\n\n\
     net SIG {\n}\n\ncomponent U1 connector \"RR\" {\n    at 10mm, 10mm\n    pin.1 = SIG\n}\n";

/// The same board with a plain rectangle, as the control: the writer has to
/// keep answering `R` for a pad that really is one.
const SQUARE: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
     footprint SQ {\n    courtyard 3mm x 2mm\n    \
     pad 1 rect at 0mm, 0mm size 2mm x 1mm\n}\n\n\
     net SIG {\n}\n\ncomponent U1 connector \"SQ\" {\n    at 10mm, 10mm\n    pin.1 = SIG\n}\n";

/// Export one design and hand back its top copper layer.
fn top_copper(who: &str, source: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-round-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let design = dir.join("b.cypcb");
    std::fs::write(&design, source).expect("the design is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg(&design)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let layer: PathBuf = dir.join("out/gerber/b-F_Cu.gbr");
    std::fs::read_to_string(&layer).unwrap_or_else(|error| panic!("{}: {error}", layer.display()))
}

#[test]
fn the_corner_reaches_the_copper_as_a_macro() {
    let gerber = top_copper("macro", BOARD);

    assert!(
        gerber.contains("%AMRR10*\n"),
        "the rounded pad has to be an aperture macro:\n{gerber}"
    );
    // The body: two rectangles that overlap in the middle. 2 by (1 - 2 x 0.25)
    // and (2 - 2 x 0.25) by 1.
    assert!(
        gerber.contains("21,1,2.000000,0.500000,0,0,0*\n"),
        "the wide half of the body is missing:\n{gerber}"
    );
    assert!(
        gerber.contains("21,1,1.500000,1.000000,0,0,0*\n"),
        "the tall half of the body is missing:\n{gerber}"
    );
    // Four circles of diameter 0.5mm, a radius in from both edges.
    for (x, y) in [
        ("0.750000", "0.250000"),
        ("-0.750000", "0.250000"),
        ("0.750000", "-0.250000"),
        ("-0.750000", "-0.250000"),
    ] {
        assert!(
            gerber.contains(&format!("1,1,0.500000,{x},{y},0*\n")),
            "no corner circle at ({x}, {y}):\n{gerber}"
        );
    }
    assert!(
        gerber.contains("%ADD10RR10*%\n"),
        "the aperture has to name the macro:\n{gerber}"
    );
}

#[test]
fn every_statement_in_the_file_is_terminated() {
    // The old line ran a `G04` on past the `*` that should have ended it and
    // left a `%` open. Neither can appear again, in any file the export
    // writes.
    let gerber = top_copper("terminated", BOARD);

    for line in gerber.lines() {
        if line.starts_with("G04") {
            assert!(
                line.ends_with('*'),
                "a comment has to end at a `*`: {line:?}"
            );
        }
        assert!(
            !line.contains("*% "),
            "nothing may follow a closed extended command on its line: {line:?}"
        );
    }
    assert_eq!(
        gerber.matches('%').count() % 2,
        0,
        "the extended-command delimiters have to pair up:\n{gerber}"
    );
}

#[test]
fn a_pad_that_is_a_rectangle_is_still_a_rectangle() {
    // The control. A macro for every pad would be a different kind of wrong.
    let gerber = top_copper("square", SQUARE);

    assert!(
        gerber.contains("%ADD10R,2.000000X1.000000*%\n"),
        "a plain rect pad has to stay a standard `R` aperture:\n{gerber}"
    );
    assert!(
        !gerber.contains("%AM"),
        "and it must not drag a macro in with it:\n{gerber}"
    );
}
