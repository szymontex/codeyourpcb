//! The snapshot the viewer draws from carries the copper of a polygon pour.
//!
//! `cargo test -p cypcb-render --features native --test a_pour_that_is_a_polygon_reaches_the_screen_as_copper`
//!
//! A pour as a rectangle arrives as `rects`. A pour drawn as an L is convex
//! pieces, `polygons`, each a ring of corners; together they hold exactly the L,
//! so the screen shows the copper the fabricator is sent. A rectangle sends what
//! it always sent: the field is left out.

use cypcb_render::PcbEngine;

fn board(points: &str) -> String {
    format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\")
           (general (thickness 1.6)) (paper \"A4\")
           (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
           (net 0 \"\") (net 1 \"GND\")
           (gr_rect (start 100 100) (end 140 130) (layer \"Edge.Cuts\") (width 0.05))
           (zone (net 1) (net_name \"GND\") (layer \"F.Cu\")
             (polygon (pts {points}))))"
    )
}

const L: &str = "(xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128)";
const RECTANGLE: &str = "(xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128)";

fn pours(points: &str) -> (Vec<cypcb_render::PourInfo>, String) {
    let mut engine = PcbEngine::new();
    let said = engine.load_kicad(&board(points));
    assert_eq!(said, "", "the board loads");
    let snapshot = engine.build_snapshot();
    let json = serde_json::to_string(&snapshot.pours).expect("pours are JSON");
    (snapshot.pours, json)
}

fn twice_area(piece: &[[i64; 2]]) -> i128 {
    (0..piece.len())
        .map(|i| {
            let (a, b) = (piece[i], piece[(i + 1) % piece.len()]);
            i128::from(a[0]) * i128::from(b[1]) - i128::from(b[0]) * i128::from(a[1])
        })
        .sum()
}

#[test]
fn a_pour_that_is_an_l_is_sent_as_convex_pieces_that_hold_the_l() {
    let (pours, json) = pours(L);
    assert_eq!(pours.len(), 1, "one pour on one layer: {json}");
    let pour = &pours[0];
    assert_eq!(pour.net, "GND");
    assert!(pour.rects.is_empty(), "no rectangles: {json}");
    assert!(
        pour.polygons.len() >= 2,
        "an L is more than one convex piece"
    );
    // 36 x 26 with 18 x 13 cut away is 702 mm2, in nm2 twice that.
    let area: i128 = pour.polygons.iter().map(|piece| twice_area(piece)).sum();
    assert_eq!(area, 2 * 702 * 1_000_000_000_000, "exactly the L");
}

#[test]
fn control_a_rectangle_is_sent_as_it_always_was() {
    let (pours, json) = pours(RECTANGLE);
    assert_eq!(pours.len(), 1);
    assert_eq!(pours[0].rects.len(), 1);
    assert!(pours[0].polygons.is_empty());
    assert!(!json.contains("polygons"), "the field is left out: {json}");
}
