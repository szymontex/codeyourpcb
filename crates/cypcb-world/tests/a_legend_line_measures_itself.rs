//! How wide a string is before anything draws it.
//!
//! `cargo test -p cypcb-world --test a_legend_line_measures_itself`
//!
//! The second of the three helpers V1's census recorded as named and had not
//! named. `width_in_glyphs` is what the silkscreen exporter asks before it
//! places a designator or a line of board text: how many glyph widths this
//! string occupies, gaps included. Every centred label on every board is that
//! number divided by two, so an off-by-one-gap here moves text off its pad.

use cypcb_world::silk_text::{width_in_glyphs, ADVANCE_GAP};

#[test]
fn one_glyph_is_one_glyph_wide_and_no_gap() {
    // A gap belongs between glyphs, so a single character has none. The
    // arithmetic that adds a trailing gap looks right until a one-character
    // designator - `1` on a test point - sits a third of a glyph left of
    // centre.
    assert_eq!(width_in_glyphs("R"), 1.0);
}

#[test]
fn a_string_is_its_glyphs_and_the_gaps_between_them() {
    // Three glyphs, two gaps. `R10` is the shape every board has dozens of.
    assert_eq!(width_in_glyphs("R10"), 3.0 + 2.0 * ADVANCE_GAP);
    assert_eq!(width_in_glyphs("REV B"), 5.0 + 4.0 * ADVANCE_GAP);
}

#[test]
fn nothing_is_no_width_rather_than_a_negative_one() {
    // The count is unsigned and the gap count is one less, so an empty string
    // is where that subtraction would underflow. It returns zero instead,
    // which is what a label with no text should reserve.
    assert_eq!(width_in_glyphs(""), 0.0);
}

#[test]
fn a_character_the_font_cannot_draw_still_takes_its_place() {
    // The stroke font knows a subset of ASCII. A name with an unknown
    // character keeps its width, so the rest of the string stays where it was
    // rather than shuffling left into the pad beside it.
    assert_eq!(width_in_glyphs("R1"), width_in_glyphs("R\u{00e5}"));
}
