//! A KiCad zone that names a set of layers is on every layer of the set.
//!
//! `cargo test -p cypcb-kicad --test a_zone_on_a_layer_set_is_on_every_layer_of_the_set`
//!
//! KiCad writes a zone's layers as names, and three of the names are sets:
//! `F&B.Cu` the two faces, `*.Cu` every copper layer of the board and `*In.Cu`
//! the ones between. The reader knew only `*.Cu`, and read it as the two faces,
//! so a zone on `F&B.Cu` named no copper layer and was refused: 13 zones on 7
//! of the 112 boards of KiCad's own tests. The count of layers is the file's, from
//! its `(layers ...)` header, because `*.Cu` means the layers the board has.

use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbParseResult};
use cypcb_world::components::zone::{Zone, ZoneKind};

const TOP: u32 = 0b01;
const BOTTOM: u32 = 0b10;
const INNER: fn(u32) -> u32 = |n| 1 << (1 + n);

fn stack(copper: usize) -> String {
    let mut text = String::from("(layers (0 \"F.Cu\" signal)");
    for n in 1..copper - 1 {
        text.push_str(&format!(" ({n} \"In{n}.Cu\" signal)"));
    }
    text.push_str(" (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))");
    text
}

fn board_with(header: &str, zones: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  {header}
  (net 0 "")
  (net 1 "GND")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
{zones}
)
"#
    )
}

fn pour(layer: &str) -> String {
    format!(
        r#"(zone (net 1) (net_name "GND") {layer}
  (polygon (pts (xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128))))"#
    )
}

fn l_pour(layer: &str) -> String {
    format!(
        r#"(zone (net 1) (net_name "GND") {layer}
  (polygon (pts (xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128))))"#
    )
}

fn l_keepout(layer: &str) -> String {
    format!(
        r#"(zone {layer} (keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed))
  (polygon (pts (xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115))))"#
    )
}

fn read(source: &str) -> KicadPcbParseResult {
    parse_kicad_pcb_str(source).expect("the board reads")
}

fn zones(parsed: &mut KicadPcbParseResult) -> Vec<Zone> {
    let ecs = parsed.world.ecs_mut();
    let mut query = ecs.query::<&Zone>();
    query.iter(ecs).cloned().collect()
}

fn masks(source: &str) -> Vec<u32> {
    let mut parsed = read(source);
    let mut masks: Vec<u32> = zones(&mut parsed).iter().map(|z| z.layer_mask).collect();
    masks.sort_unstable();
    masks
}

#[test]
fn a_pour_on_the_two_faces_is_read_on_both() {
    for (what, zone) in [
        ("a rectangle", pour(r#"(layer "F&B.Cu")"#)),
        ("an L", l_pour(r#"(layer "F&B.Cu")"#)),
    ] {
        let mut parsed = read(&board_with(&stack(2), &zone));
        assert!(
            parsed.metadata.zone_refusals.is_empty(),
            "{what}: {:?}",
            parsed.metadata.zone_refusals
        );
        let zones = zones(&mut parsed);
        assert_eq!(zones.len(), 1, "{what}");
        assert_eq!(zones[0].layer_mask, TOP | BOTTOM, "{what}");
        assert_eq!(zones[0].kind, ZoneKind::CopperPour, "{what}");
    }
}

#[test]
fn a_keepout_on_the_two_faces_is_read_on_both() {
    let mut parsed = read(&board_with(&stack(2), &l_keepout(r#"(layer "F&B.Cu")"#)));
    assert!(parsed.metadata.zone_refusals.is_empty());
    assert_eq!(parsed.metadata.outlined_zone_count, 1);
    let zones = zones(&mut parsed);
    assert_eq!(zones[0].kind, ZoneKind::Keepout);
    assert_eq!(zones[0].layer_mask, TOP | BOTTOM);
}

#[test]
fn the_two_faces_of_a_four_layer_board_are_the_two_faces() {
    // `F&B.Cu` does not reach the inner layers, whatever the board has.
    assert_eq!(
        masks(&board_with(&stack(4), &pour(r#"(layer "F&B.Cu")"#))),
        vec![TOP | BOTTOM]
    );
}

#[test]
fn every_copper_layer_is_every_layer_the_board_has() {
    let all = |n: u32| (1u32 << n) - 1;
    assert_eq!(
        masks(&board_with(&stack(4), &pour(r#"(layer "*.Cu")"#))),
        vec![all(4)]
    );
    assert_eq!(
        masks(&board_with(&stack(6), &pour(r#"(layers "*.Cu")"#))),
        vec![all(6)]
    );
    assert_eq!(
        masks(&board_with(&stack(2), &pour(r#"(layer "*.Cu")"#))),
        vec![all(2)]
    );
}

#[test]
fn a_file_with_no_layer_header_is_two_layers() {
    let no_header = board_with("", &pour(r#"(layer "*.Cu")"#));
    assert!(!no_header.contains("(layers (0"), "the header is absent");
    assert_eq!(masks(&no_header), vec![TOP | BOTTOM]);
    assert_eq!(
        masks(&board_with("", &pour(r#"(layer "F&B.Cu")"#))),
        vec![TOP | BOTTOM]
    );
}

#[test]
fn the_layers_between_the_faces_are_the_inner_ones() {
    // Six layers: four inner.
    let inner = INNER(1) | INNER(2) | INNER(3) | INNER(4);
    assert_eq!(inner, 0b111100);
    assert_eq!(
        masks(&board_with(&stack(6), &pour(r#"(layer "*In.Cu")"#))),
        vec![inner]
    );
    assert_eq!(
        masks(&board_with(&stack(4), &pour(r#"(layer "*In.Cu")"#))),
        vec![INNER(1) | INNER(2)]
    );
}

#[test]
fn a_zone_naming_a_set_and_a_layer_is_on_the_union() {
    let source = board_with(&stack(4), &pour(r#"(layers "F&B.Cu" "In1.Cu")"#));
    assert_eq!(masks(&source), vec![TOP | BOTTOM | INNER(1)]);
    // The other order is the same zone.
    let source = board_with(&stack(4), &pour(r#"(layers "In2.Cu" "F&B.Cu")"#));
    assert_eq!(masks(&source), vec![TOP | BOTTOM | INNER(2)]);
}

#[test]
fn control_a_zone_on_no_copper_layer_is_still_refused_and_says_which() {
    for (layer, zone) in [
        ("*.Mask", pour(r#"(layer "*.Mask")"#)),
        ("F.Mask", pour(r#"(layer "F.Mask")"#)),
        ("B.SilkS", l_keepout(r#"(layer "B.SilkS")"#)),
    ] {
        let mut parsed = read(&board_with(&stack(2), &zone));
        assert!(zones(&mut parsed).is_empty(), "{layer}: no zone");
        assert_eq!(parsed.metadata.zone_refusals.len(), 1, "{layer}");
        let said = &parsed.metadata.zone_refusals[0];
        assert!(
            said.contains("names no copper layer") && said.contains(&format!("\"{layer}\"")),
            "{layer}: {said}"
        );
    }
}

#[test]
fn a_zone_on_the_two_faces_keeps_its_copper_through_to_kicad_and_back() {
    let mut parsed = read(&board_with(&stack(2), &pour(r#"(layer "F&B.Cu")"#)));
    let before = zones(&mut parsed);
    assert_eq!(before.len(), 1);
    let text = write_board(&mut parsed.world, "cypcb");
    let mut back = read(&text);
    let after = zones(&mut back);
    // KiCad reads one zone for each layer, so the copper is the same and the
    // mask is split between two zones.
    let union = after.iter().fold(0, |m, z| m | z.layer_mask);
    assert_eq!(union, TOP | BOTTOM, "{after:?}");
    for zone in &after {
        assert_eq!(zone.bounds, before[0].bounds);
        assert_eq!(zone.net, before[0].net);
    }
}

#[test]
fn a_teardrop_zone_on_the_two_faces_is_carried() {
    // A teardrop is copper on one layer: the first copper layer of the set.
    let teardrop = r#"(zone (net 1) (net_name "GND") (layer "F&B.Cu")
  (attr (teardrop (type padvia)))
  (polygon (pts (xy 110 109.2) (xy 110 110.8) (xy 110.8 110.35) (xy 112 110.1) (xy 112 109.9) (xy 110.8 109.65))))"#;
    let parsed = read(&board_with(&stack(2), teardrop));
    assert!(
        parsed.metadata.zone_refusals.is_empty(),
        "{:?}",
        parsed.metadata.zone_refusals
    );
    assert_eq!(parsed.metadata.teardrop_count, 1);
}
