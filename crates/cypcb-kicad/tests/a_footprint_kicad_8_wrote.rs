//! Footprints written by KiCad 6 and later, which is most of what is on disk.
//!
//! This crate reads a `.kicad_mod` through the same reader it reads boards
//! with. It did not: the footprint path used `kicad_parse_gen`, which knows
//! the KiCad 5 spelling and nothing since. KiCad 6 renamed the head of the
//! list from `module` to `footprint` and put the format's version and the
//! writing program beside it, 7 turned the reference and value into
//! `property` lists, and `roundrect` is the shape almost every generated pad
//! uses. Each of those is refused whole - `unknown element in module: version`
//! - so a person's own library imported as nothing at all.
//!
//! The fixtures are real files that two vendored projects keep for their own
//! tests, named one by one rather than walked: a directory walk made an
//! earlier test in this repository pass or fail depending on the order the
//! filesystem returned.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

/// Every KiCad 6-or-later footprint this repository carries, with the name and
/// the pad count each file states.
const FOOTPRINTS: [(&str, &str, usize); 6] = [
    (
        "tests/fixtures/kicad-tools/tests/fixtures/Test_Library.pretty/SOT-23-5.kicad_mod",
        "SOT-23-5",
        5,
    ),
    (
        "tests/fixtures/kicad-tools/tests/fixtures/Test_Library.pretty/C_0402_1005Metric.kicad_mod",
        "C_0402_1005Metric",
        2,
    ),
    (
        "tests/fixtures/kicad-tools/tests/fixtures/Test_Library.pretty/R_0603_1608Metric.kicad_mod",
        "R_0603_1608Metric",
        2,
    ),
    (
        "tests/fixtures/faebryk/test/common/resources/test.kicad_mod",
        "LED_0201_0603Metric",
        4,
    ),
    (
        "tests/fixtures/faebryk/test/common/libs/footprints/logos.pretty/faebryk_logo.kicad_mod",
        "faebryk_logo",
        0,
    ),
    (
        "tests/fixtures/kicad-tools/tests/fixtures/purge_test_project/my_project_lib.pretty/UsedFootprint.kicad_mod",
        "UsedFootprint",
        1,
    ),
];

#[test]
fn a_footprint_kicad_8_wrote_is_read() {
    let root = repo_root();
    for (path, name, pads) in FOOTPRINTS {
        let footprint = cypcb_kicad::import_footprint(&root.join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(footprint.name, name, "the name {path} states");
        assert_eq!(footprint.pads.len(), pads, "the pads {path} states");
    }
}

#[test]
fn a_pad_this_project_has_no_shape_for_is_refused_by_name() {
    // KiCad's `custom` pad is a polygon somebody drew. The board reader falls
    // back to a rectangle for a shape it does not know, which is the right
    // answer for a board - one strange pad should not cost a person the other
    // nine hundred - and the wrong one for a single part read into a library,
    // where a rectangle is not a conservative reading of a drawn outline.
    let custom = r#"(footprint "Antenna"
  (version 20240108)
  (generator "pcbnew")
  (layer "F.Cu")
  (pad "1" smd custom (at 0 0) (size 1 1) (layers "F.Cu"))
)"#;
    let refused = cypcb_kicad::import_footprint_from_str(custom).unwrap_err();
    assert!(
        format!("{refused}").contains("pad shape `custom`"),
        "the refusal names the shape: {refused}"
    );
}

#[test]
fn a_hole_the_file_does_not_state_is_not_invented() {
    // The board reader gives a through-hole pad a 0.8mm drill when the file
    // names none, so a board stays routable. A footprint is read to be
    // measured, and a drill nobody wrote is a number nobody wrote.
    let no_drill = r#"(module Pin_Header (layer F.Cu)
  (pad 1 thru_hole circle (at 0 0) (size 1.7 1.7) (layers *.Cu))
)"#;
    let footprint = cypcb_kicad::import_footprint_from_str(no_drill).unwrap();
    assert_eq!(footprint.pads.len(), 1);
    assert!(
        footprint.pads[0].drill.is_none(),
        "the file states no drill and the footprint carries one: {:?}",
        footprint.pads[0].drill
    );
}
