//! A pad keeps the turn KiCad states for it.
//!
//! `cargo test -p cypcb-kicad --test a_pad_keeps_its_own_turn`
//!
//! KiCad writes a pad's turn as the third number of its `(at x y angle)`.
//! The importer dropped it. `fab-1X04` states four header pads 3.048 by
//! 1.524 turned 90, 2.54 apart along x: stood up they are 1.524 wide with
//! 1.016 between them, laid down they are 3.048 wide and each runs 0.508
//! into the next. `Microchip_RN4871` turns the ten pads down its two sides
//! the same way.
//!
//! A footprint file states the pad's turn inside the footprint. A board file
//! states the pad's turn on the board, and a pad that states none is axis
//! aligned whatever the part's turn is - KiCad's board reader says so in
//! `pcb_io_kicad_sexpr_parser.cpp` at a62d8cd4 (read 2026-09-26).
//!
//! The control is the same footprint with its turns taken out: its pads
//! overlap, so the check below can see an overlap when there is one.

use cypcb_core::{Nm, Point};
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_kicad::{import_footprint, import_footprint_from_str};
use cypcb_world::components::Rotation;
use cypcb_world::footprint::{Footprint, PadOutline};

fn fixture(name: &str) -> String {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/svg-pcb/kicad-components")
        .join(name);
    std::fs::read_to_string(file).expect("the fixture reads")
}

/// Every pad of the footprint as it lands on a square part at the origin.
fn outlines(footprint: &Footprint) -> Vec<(String, PadOutline)> {
    footprint
        .pads
        .iter()
        .map(|pad| {
            (
                pad.number.clone(),
                pad.outline(Point::ORIGIN, Rotation::ZERO),
            )
        })
        .collect()
}

/// Every pair of pads whose copper overlaps.
fn overlaps(footprint: &Footprint) -> Vec<(String, String)> {
    let pads = outlines(footprint);
    let mut out = Vec::new();
    for (i, (a, pa)) in pads.iter().enumerate() {
        for (b, pb) in &pads[i + 1..] {
            let apart_x = (pa.centre.x.0 - pb.centre.x.0).abs() * 2 >= pa.size.0 .0 + pb.size.0 .0;
            let apart_y = (pa.centre.y.0 - pb.centre.y.0).abs() * 2 >= pa.size.1 .0 + pb.size.1 .0;
            if !apart_x && !apart_y {
                out.push((a.clone(), b.clone()));
            }
        }
    }
    out
}

/// The footprint with every pad's third `at` number taken out.
fn without_turns(text: &str) -> Footprint {
    let text = text.replace(" 90)", ")");
    import_footprint_from_str(&text).expect("the footprint reads without its turns")
}

#[test]
fn a_header_s_pads_stand_up_as_kicad_drew_them() {
    let footprint = cypcb_kicad::import_footprint(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/svg-pcb/kicad-components/fab-1X04.kicad_mod"),
    )
    .expect("the fixture reads");

    for (number, outline) in outlines(&footprint) {
        assert_eq!(
            outline.size,
            (Nm::from_mm(1.524), Nm::from_mm(3.048)),
            "pad {number} stands 1.524 wide and 3.048 tall"
        );
    }
    assert_eq!(overlaps(&footprint), Vec::<(String, String)>::new());
}

#[test]
fn a_module_s_side_pads_lie_across_its_edge() {
    let footprint = import_footprint_from_str(&fixture("Microchip_RN4871.kicad_mod"))
        .expect("the fixture reads");

    let side: Vec<_> = outlines(&footprint)
        .into_iter()
        .filter(|(n, _)| !(6..=11).contains(&n.parse::<u32>().unwrap()))
        .collect();
    assert_eq!(side.len(), 10);
    for (number, outline) in side {
        assert_eq!(
            outline.size,
            (Nm::from_mm(1.5), Nm::from_mm(0.7)),
            "side pad {number} lies 1.5 wide and 0.7 tall"
        );
    }
    assert_eq!(overlaps(&footprint), Vec::<(String, String)>::new());
}

#[test]
fn without_their_turns_the_same_pads_run_into_each_other() {
    for name in ["fab-1X04.kicad_mod", "Microchip_RN4871.kicad_mod"] {
        let footprint = without_turns(&fixture(name));
        assert!(
            !overlaps(&footprint).is_empty(),
            "{name}: the control saw no overlap"
        );
    }
}

#[test]
fn a_footprint_file_s_turn_is_the_pad_s_turn_in_its_footprint() {
    let footprint = import_footprint(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/svg-pcb/kicad-components/fab-1X04.kicad_mod"),
    )
    .expect("the fixture reads");
    for pad in &footprint.pads {
        assert_eq!(pad.rotation, Rotation::DEG_90, "pad {}", pad.number);
    }
}

/// A part turned 90 holding a pad turned 90 on the board and a pad that
/// states no turn, and a square part holding a pad turned 90.
const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts") (width 0.05))
  (footprint "T:TURNED"
    (at 10 10 90)
    (property "Reference" "U1")
    (pad "A" smd rect (at -2 0 90) (size 1 2) (layers "F.Cu"))
    (pad "B" smd rect (at 2 0) (size 1 2) (layers "F.Cu"))
  )
  (footprint "T:SQUARE"
    (at 20 10)
    (property "Reference" "U2")
    (pad "A" smd rect (at 0 0 90) (size 1 2) (layers "F.Cu"))
  )
)
"#;

#[test]
fn a_board_file_s_turn_is_the_pad_s_turn_on_the_board() {
    let imported = parse_kicad_pcb_str(BOARD).expect("the board reads");
    let turned = imported.library.get("T:TURNED").expect("U1's footprint");
    let square = imported.library.get("T:SQUARE").expect("U2's footprint");

    // On the board: 90 for the pad that says 90, 0 for the one that says
    // nothing. Inside a footprint turned 90 that is 0 and 270.
    let turn = |fp: &Footprint, n: &str| fp.pads.iter().find(|p| p.number == n).unwrap().rotation;
    assert_eq!(turn(turned, "A"), Rotation::ZERO);
    assert_eq!(turn(turned, "B"), Rotation::DEG_270);
    assert_eq!(turn(square, "A"), Rotation::DEG_90);

    // Placed with the part's turn, each lands the way the file drew it.
    let size = |fp: &Footprint, n: &str, part: Rotation| {
        let pad = fp.pads.iter().find(|p| p.number == n).unwrap();
        pad.outline(Point::ORIGIN, part).size
    };
    let (one, two) = (Nm::from_mm(1.0), Nm::from_mm(2.0));
    assert_eq!(
        size(turned, "A", Rotation::DEG_90),
        (two, one),
        "turned 90 on the board"
    );
    assert_eq!(
        size(turned, "B", Rotation::DEG_90),
        (one, two),
        "axis aligned"
    );
    assert_eq!(
        size(square, "A", Rotation::ZERO),
        (two, one),
        "turned 90 on the board"
    );
}

/// One library name placed twice: turned 90 and square. The pad states no
/// turn, so it is axis aligned on the board both times, which makes it turned
/// 270 inside the one footprint and 0 inside the other.
const ONE_NAME_TWO_TURNS: &str = r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts") (width 0.05))
  (footprint "T:HDR"
    (at 10 10 90)
    (property "Reference" "J1")
    (pad "1" smd rect (at 0 0) (size 1 2) (layers "F.Cu"))
  )
  (footprint "T:HDR"
    (at 20 10)
    (property "Reference" "J2")
    (pad "1" smd rect (at 0 0) (size 1 2) (layers "F.Cu"))
  )
)
"#;

#[test]
fn one_footprint_name_with_two_pad_turns_is_two_footprints() {
    let imported = parse_kicad_pcb_str(ONE_NAME_TWO_TURNS).expect("the board reads");
    let mut turns: Vec<Rotation> = imported
        .library
        .iter()
        .filter(|(name, _)| name.starts_with("T:HDR"))
        .map(|(_, fp)| fp.pads[0].rotation)
        .collect();
    turns.sort_by_key(|r| r.0);
    assert_eq!(turns, vec![Rotation::ZERO, Rotation::DEG_270]);
}

/// The rectangle a footprint's pads span, in mm: `(min x, min y, max x, max y)`.
fn span(footprint: &Footprint) -> (f64, f64, f64, f64) {
    let b = footprint.bounds;
    (
        b.min.x.to_mm(),
        b.min.y.to_mm(),
        b.max.x.to_mm(),
        b.max.y.to_mm(),
    )
}

#[test]
fn a_footprint_spans_its_pads_as_they_stand() {
    let footprint = import_footprint_from_str(&fixture("fab-1X04.kicad_mod")).expect("reads");
    // Four pads 1.524 wide from -3.81 to 3.81, each 3.048 tall.
    assert_eq!(span(&footprint), (-4.572, -1.524, 4.572, 1.524));

    let imported = parse_kicad_pcb_str(BOARD).expect("the board reads");
    let square = imported.library.get("T:SQUARE").expect("U2's footprint");
    // One pad 1 by 2 turned 90: 2 wide and 1 tall.
    assert_eq!(span(square), (-1.0, -0.5, 1.0, 0.5));
}
