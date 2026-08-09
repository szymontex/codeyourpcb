//! Hovering a footprint has to say the hole is a slot, not half its size.
//!
//! `cargo test -p cypcb-lsp --test the_editor_is_told_it_is_a_slot`
//!
//! The hover card lists each pad as `shape W x H, drill D`, and for a slot `D`
//! is the **narrow** dimension - the bit that mills it. So a 2.4x1.0mm slot
//! read as `drill 1.00mm`: a round hole under half the length of the one the
//! part needs, stated with the same confidence as any other number on the
//! card. That is the last surface where a slot could still be described as
//! something it is not, and the editor is where a designer reads it.

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
"#;

/// The hover card over the footprint definition, which lists its pads.
fn card() -> String {
    let mut doc = DocumentState::new("test://slot".into(), BOARD.to_string(), 1);
    doc.parse();
    let line = BOARD
        .lines()
        .position(|line| line.starts_with("footprint USB_ANCHOR"))
        .expect("the fixture defines the footprint") as u32;

    hover_at_position(
        &doc,
        &Position {
            line,
            character: 12,
        },
    )
    .expect("hovering a footprint says something")
    .content
}

#[test]
fn a_slot_is_named_as_one_with_both_of_its_numbers() {
    let card = card();

    assert!(
        card.contains("slot 2.4mm x 1mm"),
        "the anchor is a slot and the card says so:\n{card}"
    );
}

#[test]
fn a_slot_is_not_described_as_its_narrow_dimension() {
    // The specific defect: `drill 1.00mm` on the anchor line reads as a round
    // hole a connector will not go into.
    let card = card();
    let anchor_line = card
        .lines()
        .find(|line| line.starts_with("- 1:"))
        .unwrap_or_default()
        .to_string();

    assert!(
        !anchor_line.contains("drill"),
        "the anchor is not drilled: {anchor_line}"
    );
}

#[test]
fn a_round_hole_is_still_called_a_drill() {
    // The half that must not change.
    let card = card();
    let pin_line = card
        .lines()
        .find(|line| line.starts_with("- 2:"))
        .unwrap_or_default()
        .to_string();

    assert!(pin_line.contains("drill 0.9mm"), "{pin_line}");
}
