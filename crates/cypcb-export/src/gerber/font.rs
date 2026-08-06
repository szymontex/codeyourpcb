//! A stroke font, so a legend can carry the names of the parts it labels.
//!
//! A fabricated board with no `R1` printed beside R1 cannot be assembled by
//! eye: the person holding the reel has to read the design file instead of the
//! board. Gerber has no text primitive that a fabricator is obliged to honour,
//! so a legend's letters are drawn the way its outlines are - as strokes.
//!
//! Glyphs live on a 0..1 box, four units wide and six tall before scaling, and
//! are described as polylines. Only what a reference designator needs is here:
//! capitals, digits, and the three separators that turn up in net and part
//! names. Anything else prints as nothing rather than as a wrong character.

/// A glyph as polylines over a unit box: x to the right, y up, both 0.0..1.0.
type Strokes = &'static [&'static [(f32, f32)]];

/// The strokes for one character, or `None` if this font cannot draw it.
pub fn glyph(c: char) -> Option<Strokes> {
    let upper = c.to_ascii_uppercase();
    Some(match upper {
        'A' => &[
            &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
            &[(0.2, 0.4), (0.8, 0.4)],
        ],
        'B' => &[
            &[
                (0.0, 0.0),
                (0.0, 1.0),
                (0.7, 1.0),
                (0.9, 0.8),
                (0.7, 0.5),
                (0.0, 0.5),
            ],
            &[(0.7, 0.5), (0.95, 0.25), (0.7, 0.0), (0.0, 0.0)],
        ],
        'C' => &[&[
            (1.0, 0.85),
            (0.7, 1.0),
            (0.2, 1.0),
            (0.0, 0.7),
            (0.0, 0.3),
            (0.2, 0.0),
            (0.7, 0.0),
            (1.0, 0.15),
        ]],
        'D' => &[&[
            (0.0, 0.0),
            (0.0, 1.0),
            (0.6, 1.0),
            (1.0, 0.7),
            (1.0, 0.3),
            (0.6, 0.0),
            (0.0, 0.0),
        ]],
        'E' => &[
            &[(1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)],
            &[(0.0, 0.5), (0.7, 0.5)],
        ],
        'F' => &[
            &[(1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            &[(0.0, 0.5), (0.7, 0.5)],
        ],
        'G' => &[&[
            (1.0, 0.85),
            (0.7, 1.0),
            (0.2, 1.0),
            (0.0, 0.7),
            (0.0, 0.3),
            (0.2, 0.0),
            (0.7, 0.0),
            (1.0, 0.2),
            (1.0, 0.45),
            (0.5, 0.45),
        ]],
        'H' => &[
            &[(0.0, 1.0), (0.0, 0.0)],
            &[(1.0, 1.0), (1.0, 0.0)],
            &[(0.0, 0.5), (1.0, 0.5)],
        ],
        'I' => &[
            &[(0.5, 1.0), (0.5, 0.0)],
            &[(0.2, 1.0), (0.8, 1.0)],
            &[(0.2, 0.0), (0.8, 0.0)],
        ],
        'J' => &[&[(1.0, 1.0), (1.0, 0.25), (0.8, 0.0), (0.3, 0.0), (0.0, 0.25)]],
        'K' => &[
            &[(0.0, 1.0), (0.0, 0.0)],
            &[(1.0, 1.0), (0.0, 0.45)],
            &[(0.25, 0.6), (1.0, 0.0)],
        ],
        'L' => &[&[(0.0, 1.0), (0.0, 0.0), (1.0, 0.0)]],
        'M' => &[&[(0.0, 0.0), (0.0, 1.0), (0.5, 0.5), (1.0, 1.0), (1.0, 0.0)]],
        'N' => &[&[(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)]],
        'O' => &[&[
            (0.2, 1.0),
            (0.8, 1.0),
            (1.0, 0.7),
            (1.0, 0.3),
            (0.8, 0.0),
            (0.2, 0.0),
            (0.0, 0.3),
            (0.0, 0.7),
            (0.2, 1.0),
        ]],
        'P' => &[&[
            (0.0, 0.0),
            (0.0, 1.0),
            (0.7, 1.0),
            (1.0, 0.75),
            (0.7, 0.5),
            (0.0, 0.5),
        ]],
        'Q' => &[
            &[
                (0.2, 1.0),
                (0.8, 1.0),
                (1.0, 0.7),
                (1.0, 0.3),
                (0.8, 0.0),
                (0.2, 0.0),
                (0.0, 0.3),
                (0.0, 0.7),
                (0.2, 1.0),
            ],
            &[(0.6, 0.3), (1.0, -0.1)],
        ],
        'R' => &[
            &[
                (0.0, 0.0),
                (0.0, 1.0),
                (0.7, 1.0),
                (1.0, 0.75),
                (0.7, 0.5),
                (0.0, 0.5),
            ],
            &[(0.5, 0.5), (1.0, 0.0)],
        ],
        'S' => &[&[
            (1.0, 0.85),
            (0.7, 1.0),
            (0.2, 1.0),
            (0.0, 0.8),
            (0.2, 0.55),
            (0.8, 0.45),
            (1.0, 0.2),
            (0.8, 0.0),
            (0.3, 0.0),
            (0.0, 0.15),
        ]],
        'T' => &[&[(0.0, 1.0), (1.0, 1.0)], &[(0.5, 1.0), (0.5, 0.0)]],
        'U' => &[&[
            (0.0, 1.0),
            (0.0, 0.25),
            (0.25, 0.0),
            (0.75, 0.0),
            (1.0, 0.25),
            (1.0, 1.0),
        ]],
        'V' => &[&[(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)]],
        'W' => &[&[(0.0, 1.0), (0.25, 0.0), (0.5, 0.6), (0.75, 0.0), (1.0, 1.0)]],
        'X' => &[&[(0.0, 1.0), (1.0, 0.0)], &[(0.0, 0.0), (1.0, 1.0)]],
        'Y' => &[
            &[(0.0, 1.0), (0.5, 0.5), (1.0, 1.0)],
            &[(0.5, 0.5), (0.5, 0.0)],
        ],
        'Z' => &[&[(0.0, 1.0), (1.0, 1.0), (0.0, 0.0), (1.0, 0.0)]],
        '0' => &[
            &[
                (0.2, 1.0),
                (0.8, 1.0),
                (1.0, 0.7),
                (1.0, 0.3),
                (0.8, 0.0),
                (0.2, 0.0),
                (0.0, 0.3),
                (0.0, 0.7),
                (0.2, 1.0),
            ],
            &[(0.0, 0.0), (1.0, 1.0)],
        ],
        '1' => &[
            &[(0.2, 0.8), (0.5, 1.0), (0.5, 0.0)],
            &[(0.2, 0.0), (0.8, 0.0)],
        ],
        '2' => &[&[
            (0.0, 0.8),
            (0.25, 1.0),
            (0.75, 1.0),
            (1.0, 0.75),
            (0.0, 0.0),
            (1.0, 0.0),
        ]],
        '3' => &[
            &[(0.0, 1.0), (1.0, 1.0), (0.45, 0.55)],
            &[
                (0.45, 0.55),
                (1.0, 0.3),
                (0.75, 0.0),
                (0.2, 0.0),
                (0.0, 0.15),
            ],
        ],
        '4' => &[&[(0.75, 0.0), (0.75, 1.0), (0.0, 0.3), (1.0, 0.3)]],
        '5' => &[&[
            (1.0, 1.0),
            (0.0, 1.0),
            (0.0, 0.55),
            (0.7, 0.6),
            (1.0, 0.35),
            (0.8, 0.0),
            (0.2, 0.0),
            (0.0, 0.15),
        ]],
        '6' => &[&[
            (1.0, 0.85),
            (0.6, 1.0),
            (0.2, 0.8),
            (0.0, 0.35),
            (0.2, 0.0),
            (0.7, 0.0),
            (1.0, 0.25),
            (0.8, 0.55),
            (0.2, 0.6),
            (0.0, 0.35),
        ]],
        '7' => &[&[(0.0, 1.0), (1.0, 1.0), (0.35, 0.0)]],
        '8' => &[
            &[
                (0.3, 0.55),
                (0.05, 0.75),
                (0.25, 1.0),
                (0.75, 1.0),
                (0.95, 0.75),
                (0.7, 0.55),
                (0.3, 0.55),
            ],
            &[
                (0.3, 0.55),
                (0.0, 0.3),
                (0.25, 0.0),
                (0.75, 0.0),
                (1.0, 0.3),
                (0.7, 0.55),
            ],
        ],
        '9' => &[&[
            (0.0, 0.15),
            (0.35, 0.0),
            (0.85, 0.2),
            (1.0, 0.65),
            (0.8, 1.0),
            (0.3, 1.0),
            (0.0, 0.75),
            (0.25, 0.45),
            (0.85, 0.4),
            (1.0, 0.65),
        ]],
        '-' => &[&[(0.1, 0.5), (0.9, 0.5)]],
        '_' => &[&[(0.0, 0.0), (1.0, 0.0)]],
        '.' => &[&[(0.4, 0.0), (0.6, 0.0)]],
        ' ' => &[],
        _ => return None,
    })
}

/// How many glyph widths a string occupies, spacing included.
///
/// Characters this font cannot draw still take their place, so a name with one
/// unknown character stays aligned rather than shuffling left.
pub fn width_in_glyphs(text: &str) -> f32 {
    let count = text.chars().count();
    if count == 0 {
        0.0
    } else {
        count as f32 * 1.0 + (count - 1) as f32 * ADVANCE_GAP
    }
}

/// Space between glyph boxes, as a fraction of a glyph's width.
pub const ADVANCE_GAP: f32 = 0.35;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_a_designator_can_hold_has_a_glyph() {
        // A refdes is letters and digits, and net names bring the separators.
        // A missing glyph prints nothing, which is worse than an ugly one: the
        // board says `R` where the design says `R1`.
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_. ".chars() {
            assert!(glyph(c).is_some(), "no glyph for {c:?}");
        }
    }

    #[test]
    fn a_character_outside_the_font_draws_nothing_rather_than_something_wrong() {
        assert!(glyph('#').is_none());
        assert!(glyph('%').is_none());
    }

    #[test]
    fn strokes_stay_inside_the_box_they_are_measured_against() {
        // Placement assumes a glyph fits 0..1 in both axes. The tail of a Q is
        // the one deliberate exception and it hangs below the baseline.
        for c in "ABCDEFGHIJKLMNOPRSTUVWXYZ0123456789-_.".chars() {
            for stroke in glyph(c).expect("a glyph") {
                for (x, y) in *stroke {
                    assert!(
                        (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y),
                        "{c:?} has a point at ({x}, {y}) outside its box"
                    );
                }
            }
        }
    }
}
