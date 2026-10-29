//! A pour reaches the screen and the island check on every layer it is on.
//!
//! `cargo test -p cypcb-render --features native --test a_pour_on_an_inner_layer_reaches_the_screen`
//!
//! The Gerber already carried a pour on an inner layer; the snapshot and the
//! island check looped over the two outer layers only, so the plane was sent to
//! the fabricator and shown to nobody. Each fill is one `PourInfo` and its
//! `layer_mask` is the one layer it is on, so the viewer can colour it and hide
//! it by that layer. A board of two layers is read as it always was.

use cypcb_render::{PcbEngine, PourInfo};

const TOP: u32 = 0b0001;
const BOTTOM: u32 = 0b0010;
const INNER1: u32 = 0b0100;
const INNER2: u32 = 0b1000;

fn four_layers(zone_layers: &str) -> String {
    format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\")
           (general (thickness 1.6)) (paper \"A4\")
           (layers (0 \"F.Cu\" signal) (1 \"In1.Cu\" signal) (2 \"In2.Cu\" signal)
                   (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
           (net 0 \"\") (net 1 \"GND\")
           (gr_rect (start 100 100) (end 140 130) (layer \"Edge.Cuts\") (width 0.05))
           (zone (net 1) (net_name \"GND\") {zone_layers}
             (polygon (pts (xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128)))))"
    )
}

/// The same board with the pour drawn as an L, which is a ring and not a box.
fn four_layers_l(zone_layers: &str) -> String {
    four_layers(zone_layers).replace(
        "(xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128)",
        "(xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128)",
    )
}

fn two_layers() -> String {
    "(kicad_pcb (version 20240108) (generator \"pcbnew\")
       (general (thickness 1.6)) (paper \"A4\")
       (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
       (net 0 \"\") (net 1 \"GND\")
       (gr_rect (start 100 100) (end 140 130) (layer \"Edge.Cuts\") (width 0.05))
       (zone (net 1) (net_name \"GND\") (layers \"F.Cu\" \"B.Cu\")
         (polygon (pts (xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128)))))"
        .to_string()
}

fn load(source: &str) -> (Vec<PourInfo>, Vec<String>) {
    let mut engine = PcbEngine::new();
    let said = engine.load_kicad(source);
    assert_eq!(said, "", "the board loads");
    let snapshot = engine.build_snapshot();
    let islands = snapshot
        .violations
        .iter()
        .filter(|v| v.kind == "pour-island")
        .map(|v| v.message.clone())
        .collect();
    (snapshot.pours, islands)
}

fn masks(pours: &[PourInfo]) -> Vec<u32> {
    let mut masks: Vec<u32> = pours.iter().map(|p| p.layer_mask).collect();
    masks.sort_unstable();
    masks
}

#[test]
fn a_pour_on_the_top_and_the_first_inner_layer_is_filled_on_both() {
    let (pours, _) = load(&four_layers("(layers \"F.Cu\" \"In1.Cu\")"));
    assert_eq!(
        masks(&pours),
        vec![TOP, INNER1],
        "one fill for each layer of its mask"
    );
    assert_eq!(pours[0].rects, pours[1].rects, "the same plane on both");
    assert!(!pours[0].rects.is_empty());
}

#[test]
fn a_pour_on_the_second_inner_layer_alone_is_drawn_as_that_layer() {
    let (pours, _) = load(&four_layers("(layer \"In2.Cu\")"));
    assert_eq!(masks(&pours), vec![INNER2]);
}

#[test]
fn control_a_pour_on_the_outer_layers_of_four_is_still_two_fills() {
    let (pours, _) = load(&four_layers("(layers \"F.Cu\" \"B.Cu\")"));
    assert_eq!(masks(&pours), vec![TOP, BOTTOM]);
}

#[test]
fn control_a_board_of_two_layers_gives_the_two_fills_it_always_did() {
    let (pours, _) = load(&two_layers());
    assert_eq!(masks(&pours), vec![TOP, BOTTOM]);
}

#[test]
fn a_plane_on_an_inner_layer_is_checked_for_islands_as_one_on_the_top_is() {
    // No pad of the pour's net anywhere: the whole plane reaches nothing.
    let (_, on_top) = load(&four_layers("(layer \"F.Cu\")"));
    assert_eq!(on_top.len(), 1, "the top plane is an island: {on_top:?}");
    let (_, on_inner) = load(&four_layers("(layer \"In1.Cu\")"));
    assert_eq!(on_inner.len(), 1, "so is the inner one: {on_inner:?}");
    let (_, on_both) = load(&four_layers("(layers \"F.Cu\" \"In1.Cu\")"));
    assert_eq!(on_both.len(), 2, "one for each layer it is on: {on_both:?}");
}

#[test]
fn a_pour_that_is_an_l_on_an_inner_layer_arrives_as_its_pieces() {
    let source = four_layers_l("(layers \"F.Cu\" \"In1.Cu\")");
    assert_ne!(
        source,
        four_layers("(layers \"F.Cu\" \"In1.Cu\")"),
        "the ring is in the board"
    );
    let (pours, islands) = load(&source);
    assert_eq!(masks(&pours), vec![TOP, INNER1]);
    for pour in &pours {
        assert!(
            pour.rects.is_empty() && pour.polygons.len() >= 2,
            "convex pieces of an L"
        );
    }
    assert_eq!(pours[0].polygons, pours[1].polygons, "the same L on both");
    assert_eq!(
        islands.len(),
        2,
        "and the island check reads the ring on both: {islands:?}"
    );
}
