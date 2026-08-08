//! A number the importer cannot read is an error, not a default.
//!
//! `cargo test -p cypcb-kicad --test a_malformed_number_is_refused`
//!
//! Every coordinate, size and drill was read with `unwrap_or`, so a board file
//! with a typo imported as a board with parts, pads and copper somewhere else.
//! `multi_ic.kicad_pcb` carried `(at 105, 80)` - one comma - and its ferrite
//! bead and Ethernet transformer sat 50mm to the left of the board for as long
//! as the fixture existed. Every routing number measured on it was measured
//! with them out there.
//!
//! A board file is written by machines and edited by people. Reading it wrong
//! and saying nothing is worse than refusing to read it.

use cypcb_kicad::parse_kicad_pcb;

use std::io::Write;

fn board_with(footprint_body: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "A")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
{footprint_body}
)
"#
    )
}

fn parse_fails(name: &str, source: &str) -> String {
    let dir = std::env::temp_dir().join("cypcb-kicad-malformed");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join(format!("{name}.kicad_pcb"));
    let mut file = std::fs::File::create(&path).expect("the board is writable");
    file.write_all(source.as_bytes())
        .expect("the board is written");
    drop(file);

    match parse_kicad_pcb(&path) {
        Ok(_) => panic!("{name}: a malformed number has to be refused, not defaulted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_comma_in_a_footprint_position_is_refused() {
    let report = parse_fails(
        "fp-position",
        &board_with(
            r#"  (footprint "T:P" (layer "F.Cu") (at 110, 110)
    (property "Reference" "J1")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
  )"#,
        ),
    );
    assert!(report.contains("footprint position x"), "got: {report}");
    assert!(
        report.contains("one comma away"),
        "the message has to say what to look for; got: {report}"
    );
}

#[test]
fn a_malformed_pad_position_size_or_drill_is_refused() {
    for (label, pad, expected) in [
        (
            "pad-position",
            r#"(pad "1" thru_hole rect (at 0, 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))"#,
            "pad position x",
        ),
        (
            "pad-size",
            r#"(pad "1" thru_hole rect (at 0 0) (size 1.7mm 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))"#,
            "pad width",
        ),
        (
            "pad-drill",
            r#"(pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill ~) (layers "*.Cu") (net 1 "A"))"#,
            "pad drill",
        ),
    ] {
        let report = parse_fails(
            label,
            &board_with(&format!(
                "  (footprint \"T:P\" (layer \"F.Cu\") (at 110 110)\n    (property \"Reference\" \"J1\")\n    {pad}\n  )"
            )),
        );
        assert!(report.contains(expected), "{label} got: {report}");
    }
}

#[test]
fn a_board_whose_numbers_are_all_numbers_still_parses() {
    // The other direction, and the one that matters more: refusing a good
    // board would be a worse defect than reading a bad one.
    let dir = std::env::temp_dir().join("cypcb-kicad-malformed");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join("good.kicad_pcb");
    std::fs::write(
        &path,
        board_with(
            r#"  (footprint "T:P" (layer "F.Cu") (at 110 110)
    (property "Reference" "J1")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
  )"#,
        ),
    )
    .expect("the board is writable");

    let result = parse_kicad_pcb(&path).expect("a well-formed board parses");
    assert_eq!(result.metadata.component_count, 1);
}
