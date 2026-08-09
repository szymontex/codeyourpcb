//! The name of a footprint you wrote has to hover like one you did not.
//!
//! `cargo test -p cypcb-lsp --test hovering_your_own_footprint_says_something`
//!
//! Hovering the footprint name inside a component built a fresh
//! `FootprintLibrary::new()` and looked the name up in it, so it only ever
//! found the **built-in** parts. A design's own footprint fell through to a
//! card that guessed: *"Not in built-in library. May be a custom footprint
//! defined in this file."* The document has the parsed file sitting in it. It
//! did not look.
//!
//! Two things were wrong with that, and the second is worse than the first. A
//! footprint you wrote gave you nothing while an 0402 gave you every pad - and
//! a footprint you **mistyped** gave you the same reassuring sentence, so the
//! one case where the editor should say "there is no such thing" was the case
//! it hedged.

use cypcb_lsp::document::{DocumentState, Position};
use cypcb_lsp::hover::hover_at_position;

const BOARD: &str = r#"version 1

board anchors {
    size 30mm x 20mm
    layers 2
}

footprint USB_ANCHOR {
    description "one shell anchor and one pin"
    courtyard 12mm x 6mm

    pad 1 oblong at -4mm, 0mm size 3.2mm x 1.8mm drill 2.4mm x 1.0mm
    pad 2 circle at 0mm, 0mm size 1.6mm x 1.6mm drill 0.9mm
}

component J1 connector "USB_ANCHOR" {
    value "USB-C receptacle"
    at 15mm, 10mm
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 5mm
}

component R2 resistor "TYPO_0402" {
    value "10k"
    at 8mm, 5mm
}
"#;

/// The hover card over the footprint name in the given component's line.
fn card_over_footprint_of(refdes: &str) -> String {
    let mut doc = DocumentState::new("test://own".into(), BOARD.to_string(), 1);
    doc.parse();

    let (line, text) = BOARD
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with(&format!("component {refdes} ")))
        .expect("the fixture places the component");
    // Inside the quoted footprint name.
    let character = text.find('"').expect("the line names a footprint") as u32 + 2;

    hover_at_position(
        &doc,
        &Position {
            line: line as u32,
            character,
        },
    )
    .expect("hovering a footprint name says something")
    .content
}

#[test]
fn a_footprint_written_in_the_file_lists_its_pads() {
    let card = card_over_footprint_of("J1");

    assert!(
        card.contains("slot 2.4mm x 1mm"),
        "the anchor's own geometry, from the file the document already parsed:\n{card}"
    );
    assert!(
        card.contains("drill 0.9mm"),
        "and the ordinary pin beside it:\n{card}"
    );
}

#[test]
fn a_footprint_written_in_the_file_is_not_called_unknown() {
    let card = card_over_footprint_of("J1");

    assert!(
        !card.contains("unknown") && !card.contains("May be a custom footprint"),
        "the file defines it, so nothing here is a guess:\n{card}"
    );
}

#[test]
fn a_built_in_footprint_still_hovers() {
    // The half that must not change.
    let card = card_over_footprint_of("R1");

    assert!(card.contains("0402"), "{card}");
    assert!(card.contains("Pads:"), "{card}");
}

#[test]
fn a_name_nothing_defines_is_said_to_be_one() {
    // The case the old card hedged on: neither the built-in library nor this
    // file has `TYPO_0402`, and a designer reading "may be a custom footprint
    // defined in this file" has been told the opposite of what is true.
    let card = card_over_footprint_of("R2");

    assert!(
        card.contains("TYPO_0402"),
        "the card names what was asked for:\n{card}"
    );
    assert!(
        !card.contains("May be a custom footprint defined in this file"),
        "the file does not define it, and the card must not suggest it might:\n{card}"
    );
}
