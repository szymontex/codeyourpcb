//! A board may carry two footprints under one library name.
//!
//! `cargo test -p cypcb-kicad --test two_parts_one_name_two_geometries`
//!
//! The importer stored the first geometry it saw under a library name and gave
//! it to every later part naming that library:
//!
//! ```ignore
//! if !library.contains(&library_key) { library.register(...) }
//! ```
//!
//! So a board with a header laid along x beside the same header laid along y -
//! or with a footprint someone edited in place, which KiCad allows and writes
//! into the board file - imported as two copies of whichever came first, and
//! the model then disagreed with the file about where the copper is. It was
//! found on `qfp_fanout`, where two of four headers ran off the board in the
//! model and nowhere near the edge in the file.

use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::{FootprintRef, Position};

use std::io::Write;

/// Two parts naming `Test:TwoPin`, with their pads in different places.
const ONE_NAME_TWO_SHAPES: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "A")
  (net 2 "B")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (footprint "Test:TwoPin"
    (layer "F.Cu")
    (at 110 110)
    (property "Reference" "J1")
    (property "Value" "along y")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
    (pad "2" thru_hole oval (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 2 "B"))
  )

  (footprint "Test:TwoPin"
    (layer "F.Cu")
    (at 125 110)
    (property "Reference" "J2")
    (property "Value" "along x")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
    (pad "2" thru_hole oval (at 2.54 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 2 "B"))
  )
)
"#;

/// The same name used twice for the *same* geometry, which must stay one entry.
const ONE_NAME_ONE_SHAPE: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "A")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (footprint "Test:TwoPin"
    (layer "F.Cu")
    (at 110 110)
    (property "Reference" "J1")
    (property "Value" "first")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
    (pad "2" thru_hole oval (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
  )

  (footprint "Test:TwoPin"
    (layer "F.Cu")
    (at 125 110)
    (property "Reference" "J2")
    (property "Value" "second")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
    (pad "2" thru_hole oval (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "A"))
  )
)
"#;

fn parse(name: &str, source: &str) -> cypcb_kicad::KicadPcbParseResult {
    let dir = std::env::temp_dir().join("cypcb-kicad-library-keys");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join(format!("{name}.kicad_pcb"));
    let mut file = std::fs::File::create(&path).expect("the board is writable");
    file.write_all(source.as_bytes())
        .expect("the board is written");
    drop(file);

    parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("{name} must parse: {e:?}"))
}

/// Where each part's pads end up on the board, in nanometres.
fn pad_positions(result: &mut cypcb_kicad::KicadPcbParseResult) -> Vec<(String, Vec<(i64, i64)>)> {
    let library = result.library.clone();
    let ecs = result.world.ecs_mut();
    let mut query = ecs.query::<(&cypcb_world::RefDes, &Position, &FootprintRef)>();
    let mut out: Vec<(String, Vec<(i64, i64)>)> = query
        .iter(ecs)
        .map(|(refdes, position, footprint)| {
            let pads = library
                .get(footprint.as_str())
                .map(|f| {
                    f.pads
                        .iter()
                        .map(|pad| {
                            (
                                position.0.x.raw() + pad.position.x.raw(),
                                position.0.y.raw() + pad.position.y.raw(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            (refdes.as_str().to_string(), pads)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn two_geometries_under_one_name_stay_two_geometries() {
    let mut result = parse("two-shapes", ONE_NAME_TWO_SHAPES);

    // Only the entries this board contributed: the library ships with the
    // built-in footprints, and counting those was the first version of this
    // assertion getting the wrong answer for the right reason.
    let ours: Vec<&str> = result
        .library
        .iter()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("Test:TwoPin"))
        .collect();
    assert_eq!(
        ours.len(),
        2,
        "two different geometries need two library entries, got: {ours:?}"
    );

    let placed = pad_positions(&mut result);
    let j1 = &placed[0].1;
    let j2 = &placed[1].1;

    // J1's second pad is 2.54mm below its first; J2's is 2.54mm to the right.
    assert_eq!(j1[1].0 - j1[0].0, 0, "J1 runs along y: its pads share an x");
    assert_eq!(
        j1[1].1 - j1[0].1,
        2_540_000,
        "J1's pads are a pitch apart in y"
    );
    assert_eq!(j2[1].1 - j2[0].1, 0, "J2 runs along x: its pads share a y");
    assert_eq!(
        j2[1].0 - j2[0].0,
        2_540_000,
        "J2's pads are a pitch apart in x"
    );
}

#[test]
fn one_geometry_named_twice_stays_one_entry() {
    // The other direction, which is what a real board almost always has: a
    // hundred 0402s naming one library. Splitting those into a hundred
    // entries would be its own defect.
    let result = parse("one-shape", ONE_NAME_ONE_SHAPE);

    let ours: Vec<&str> = result
        .library
        .iter()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("Test:TwoPin"))
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "the same geometry twice is one footprint, got: {ours:?}"
    );
}
