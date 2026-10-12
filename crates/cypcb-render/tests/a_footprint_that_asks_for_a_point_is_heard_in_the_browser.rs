//! A `FT Origin` the browser cannot take is said to the person who opened it.
//!
//! `cargo test -p cypcb-render --features native --test a_footprint_that_asks_for_a_point_is_heard_in_the_browser`
//!
//! The command line prints a warning when a KiCad board's footprint states a
//! point to be placed by that is neither `Anchor` nor `Center`. The browser has
//! no stderr: its notes are the diagnostics the engine holds.

use cypcb_render::PcbEngine;

fn board(value: Option<&str>) -> String {
    let field = value
        .map(|v| {
            format!(
                "(property \"FT Origin\" \"{v}\" (at 0 0 0) (layer \"F.Fab\") (hide yes) \
                 (effects (font (size 1 1))))"
            )
        })
        .unwrap_or_default();
    format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (generator_version \"8.0.0\")
           (general (thickness 1.6)) (paper \"A4\")
           (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
           (net 0 \"\")
           (gr_line (start 90 90) (end 150 90) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 150 90) (end 150 120) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 150 120) (end 90 120) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 90 120) (end 90 90) (layer \"Edge.Cuts\") (width 0.05))
           (footprint \"Conn:Header1\" (layer \"F.Cu\") (at 100 100)
             (property \"Reference\" \"J1\" (at 0 -2 0) (layer \"F.SilkS\"))
             (property \"Value\" \"hdr\" (at 0 2 0) (layer \"F.Fab\"))
             {field}
             (pad \"1\" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers \"*.Cu\" \"*.Mask\"))))"
    )
}

fn heard(value: Option<&str>) -> String {
    let mut engine = PcbEngine::new();
    engine.load_kicad(&board(value));
    engine.get_diagnostics_json()
}

#[test]
fn a_value_that_is_neither_word_reaches_the_diagnostics() {
    let said = heard(Some("Middle"));
    assert!(
        said.contains("Header1") && said.contains("Middle"),
        "{said}"
    );
}

#[test]
fn control_a_word_or_nothing_says_nothing() {
    for value in [None, Some("Anchor"), Some("Center")] {
        let said = heard(value);
        assert!(!said.contains("FT Origin"), "{value:?}: {said}");
    }
}
