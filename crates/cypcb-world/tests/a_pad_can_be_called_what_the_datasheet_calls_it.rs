//! `pad A1`, `pad "S1"`, and a USB-C receptacle that finally parses.
//!
//! `cargo test -p cypcb-world --test a_pad_can_be_called_what_the_datasheet_calls_it`
//!
//! A pad's name is a name, not a count. A USB-C receptacle names its pads A1
//! and B4, a BGA names them by row and column, an edge connector whatever the
//! datasheet says. `PadDef.number` in `cypcb-world` has been a `String` since
//! it was written - its own doc comment names `"A1"` and `"VCC"` - and the
//! parser insisted on a number, which kept most boards worth importing out of
//! the language entirely.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Read a design the way the CLI does.
fn read(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "{:?}", sync.errors);
    (world, library)
}

/// The shape that could not be written down before: letter-and-digit pad
/// names, and one that needs quotes to survive the grammar.
const RECEPTACLE: &str = r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

footprint USBC {
    pad A1 rect at -2mm, 0mm size 0.3mm x 1mm
    pad B4 rect at -1mm, 0mm size 0.3mm x 1mm
    pad "S1" rect at 2mm, 0mm size 1mm x 2mm drill 0.6mm
    pad 1 rect at 0mm, 0mm size 0.3mm x 1mm
}

component J1 connector "USBC" {
    value "USB-C"
    at 10mm, 10mm
}
"#;

/// Every name the datasheet uses, kept as it was written.
fn pad_names(library: &FootprintLibrary) -> Vec<String> {
    let footprint = library.get("USBC").expect("the footprint is registered");
    footprint
        .pads
        .iter()
        .map(|pad| pad.number.clone())
        .collect()
}

#[test]
fn a_pad_keeps_the_name_the_design_gave_it() {
    let (_world, library) = read(RECEPTACLE);
    assert_eq!(
        pad_names(&library),
        vec!["A1".to_string(), "B4".into(), "S1".into(), "1".into()],
    );
}

/// A bare number must not pick up a decimal point on the way in. Pin
/// references are matched against pad names by string, so `R1.1` looking for
/// a pad called `1.0` finds nothing.
#[test]
fn a_numbered_pad_is_still_called_by_its_number() {
    let (_world, library) = read(RECEPTACLE);
    let names = pad_names(&library);
    assert!(names.contains(&"1".to_string()), "{names:?}");
    assert!(!names.iter().any(|name| name.contains('.')), "{names:?}");
}

/// The DSL is this project's storage format, so a name it can hold and cannot
/// write down is a name that disappears on the first save.
#[test]
fn every_pad_name_survives_being_written_back_out() {
    let (mut world, _library) = read(RECEPTACLE);
    let written = board_as_dsl(&mut world);

    // `S1` starts with a letter and is written bare; the quoted form in the
    // source is one of three spellings the reader accepts for the same name.
    assert!(written.contains("pad A1 "), "{written}");
    assert!(written.contains("pad B4 "), "{written}");
    assert!(written.contains("pad S1 "), "{written}");
    assert!(written.contains("pad 1 "), "{written}");

    let (_again, library) = read(&written);
    assert_eq!(
        pad_names(&library),
        vec!["A1".to_string(), "B4".into(), "S1".into(), "1".into()],
        "what came out has to read back as the same four pads"
    );
}

/// A name the identifier rule refuses has to come back quoted, or the file
/// this project wrote is a file it cannot read.
#[test]
fn a_name_that_needs_quotes_gets_them() {
    let source = RECEPTACLE.replace("pad \"S1\"", "pad \"A1+\"");
    let (mut world, _library) = read(&source);
    let written = board_as_dsl(&mut world);

    assert!(written.contains("pad \"A1+\""), "{written}");
    let (_again, library) = read(&written);
    assert!(pad_names(&library).contains(&"A1+".to_string()));
}
