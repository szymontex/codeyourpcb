//! A KiCad teardrop zone arrives as teardrop copper, whole.
//!
//! `cargo test -p cypcb-kicad --test a_teardrop_zone_arrives_as_teardrop_copper`
//!
//! KiCad keeps a teardrop as a zone on its track's layer and net, marked
//! `(attr (teardrop (type padvia)))` (`PCB_IO_KICAD_SEXPR::format( const ZONE* )`
//! and its parser, read 2026-10-11 from the KiCad master branch). The reader
//! used to take every zone for a pour: a teardrop is not a rectangle, so it was
//! refused by name when it was not skipped, and the copper it is went missing
//! from the model, the checker and the Gerber.
//!
//! What is held here: the polygon arrives as the polygon it is - a curved
//! teardrop has sides that bow inwards and is not convex, and a bounding box
//! would be copper where KiCad drew none - on the layer and the net the zone
//! names; the mask opening KiCad draws beside each teardrop is counted and said
//! once; a pour beside it is still a pour; and the builder that makes this
//! model's own teardrops leaves the copper from the file alone.

use std::io::Write;

use cypcb_core::{Nm, Point};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::Layer;
use cypcb_world::teardrop::{polygon_contains, teardrop_copper};

/// A board with one concave (curved-sided) teardrop on the top layer on net
/// GND, the solder mask zone KiCad makes beside it, and a rectangular pour.
const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (37 "F.Mask" user)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "GND")
  (net 2 "SIG")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (zone
    (net 1)
    (net_name "GND")
    (layer "F.Cu")
    (hatch none 0.1)
    (priority 30000)
    (attr (teardrop (type padvia)))
    (connect_pads yes (clearance 0))
    (min_thickness 0.0254)
    (fill yes (island_removal_mode 1))
    (polygon
      (pts (xy 110 109.2) (xy 110 110.8) (xy 110.8 110.35) (xy 112 110.1) (xy 112 109.9) (xy 110.8 109.65))
    )
    (filled_polygon
      (layer "F.Cu")
      (pts (xy 110 109.2) (xy 110 110.8) (xy 110.8 110.35) (xy 112 110.1) (xy 112 109.9) (xy 110.8 109.65))
    )
  )

  (zone
    (net 2)
    (net_name "SIG")
    (layer "B.Cu")
    (hatch none 0.1)
    (priority 30002)
    (attr (teardrop (type padvia)))
    (connect_pads yes (clearance 0))
    (min_thickness 0.0254)
    (fill yes (island_removal_mode 1))
    (polygon
      (pts (xy 120 109.2) (xy 120 110.8) (xy 122 110.1) (xy 122 109.9))
    )
  )

  (zone
    (layer "F.Mask")
    (hatch none 0.1)
    (priority 30001)
    (attr (teardrop (type padvia)))
    (min_thickness 0.0254)
    (fill yes)
    (polygon
      (pts (xy 110 109.2) (xy 110 110.8) (xy 112 110.1) (xy 112 109.9))
    )
  )

  (zone
    (net 1)
    (net_name "GND")
    (layer "F.Cu")
    (hatch edge 0.5)
    (connect_pads (clearance 0.5))
    (min_thickness 0.25)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon
      (pts (xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128))
    )
  )
)
"#;

fn parse(who: &str, source: &str) -> cypcb_kicad::KicadPcbParseResult {
    let dir = tempfile::tempdir().expect("a place to put the board");
    let path = dir.path().join(format!("{who}.kicad_pcb"));
    let mut file = std::fs::File::create(&path).expect("the board is writable");
    file.write_all(source.as_bytes())
        .expect("the board is written");
    drop(file);
    parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("{who} must parse: {e:?}"))
}

#[test]
fn the_teardrop_is_copper_on_its_layer_and_net_and_the_pour_is_still_a_pour() {
    let mut parsed = parse("teardrop", BOARD);
    assert_eq!(parsed.metadata.teardrop_count, 2);
    assert_eq!(parsed.metadata.zone_count, 1, "the pour beside it");

    let held = teardrop_copper(&mut parsed.world);
    assert_eq!(held.len(), 2);
    let described: Vec<(Layer, String, usize)> = held
        .iter()
        .map(|(_, copper, net)| {
            (
                copper.layer,
                parsed.world.net_name(*net).unwrap_or_default().to_string(),
                copper.outline.len(),
            )
        })
        .collect();
    // Each on the layer its zone names and the net its zone carries, with every
    // corner of its polygon.
    assert!(
        described.contains(&(Layer::TopCopper, "GND".to_string(), 6)),
        "{described:?}"
    );
    assert!(
        described.contains(&(Layer::BottomCopper, "SIG".to_string(), 4)),
        "{described:?}"
    );
    assert!(
        held.iter().all(|(_, copper, _)| copper.land.is_none()),
        "it came from a file and names no land"
    );
}

#[test]
fn the_polygon_is_not_flattened_to_its_box_or_its_hull() {
    let mut parsed = parse("curved", BOARD);
    let held = teardrop_copper(&mut parsed.world);
    let copper = &held
        .iter()
        .find(|(_, c, _)| c.layer == Layer::TopCopper)
        .expect("the top one")
        .1;
    // The tip (corners 3 and 4) and the side that bows inwards (corner 2): the
    // point beside corner 2, away from the axis, is between the polygon's edge
    // and the straight line between its neighbours - inside the hull and the
    // box, outside the shape. Found from the outline so the board's frame does
    // not matter.
    let o = &copper.outline;
    let tip = Point::new(Nm((o[3].x.0 + o[4].x.0) / 2), Nm((o[3].y.0 + o[4].y.0) / 2));
    let inside = Point::new(Nm(tip.x.0 - 1_000_000), tip.y);
    let away = if o[2].y.0 > tip.y.0 { 1 } else { -1 };
    let in_the_box_not_the_shape = Point::new(o[2].x, Nm(o[2].y.0 + away * 100_000));
    assert!(
        polygon_contains(&copper.outline, inside),
        "the body is copper"
    );
    assert!(
        !polygon_contains(&copper.outline, in_the_box_not_the_shape),
        "a bounding box would call this copper and the polygon does not"
    );
}

#[test]
fn the_mask_opening_beside_it_is_counted_and_said_once() {
    let parsed = parse("mask", BOARD);
    let said: Vec<&String> = parsed
        .metadata
        .zone_refusals
        .iter()
        .filter(|line| line.contains("solder mask"))
        .collect();
    assert_eq!(said.len(), 1, "{:?}", parsed.metadata.zone_refusals);
    assert!(said[0].starts_with("1 teardrop zone(s)"), "{}", said[0]);
    assert_eq!(
        parsed.metadata.zone_refusals.len(),
        1,
        "nothing else is refused: {:?}",
        parsed.metadata.zone_refusals
    );
}

#[test]
fn the_builder_leaves_the_copper_from_the_file_alone() {
    let mut parsed = parse("rebuild", BOARD);
    let library = parsed.library.clone();
    let before = teardrop_copper(&mut parsed.world);
    // Asking for teardrops builds the board's own from its tracks; there are
    // none here, and the file's copper is neither removed nor doubled.
    assert!(parsed
        .world
        .set_teardrops(cypcb_world::components::Teardrops::default()));
    for _ in 0..3 {
        parsed.world.rebuild_spatial_index_from_library(&library);
    }
    let after = teardrop_copper(&mut parsed.world);
    assert_eq!(after.len(), before.len());
    assert_eq!(
        after.iter().map(|(_, c, _)| c.clone()).collect::<Vec<_>>(),
        before.iter().map(|(_, c, _)| c.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn control_a_zone_without_the_mark_is_not_a_teardrop() {
    let plain = BOARD.replace("(attr (teardrop (type padvia)))", "");
    let mut parsed = parse("unmarked", &plain);
    assert_eq!(parsed.metadata.teardrop_count, 0);
    assert!(teardrop_copper(&mut parsed.world).is_empty());
    // Both of the formerly marked zones are now ordinary zones: the first is a
    // seven-point outline this importer refuses by name, the second is on a
    // mask layer and names no copper.
    assert!(parsed
        .metadata
        .zone_refusals
        .iter()
        .any(|line| line.contains("6-point outline")));
}
