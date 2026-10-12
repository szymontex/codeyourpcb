//! A part soldered to the bottom face keeps each pad's hole where the part has
//! it.
//!
//! `cargo test -p cypcb-world --test a_flipped_pad_keeps_its_hole_where_it_was`
//!
//! `mirrored_to_bottom` runs a footprint the other way: x reverses, and a
//! pad's own turn with it. A pad whose hole is off the middle of its copper
//! ([`PadDef::hole_offset`]) states it in the pad's own frame, so the flip has
//! to run the offset the way it runs the position - dropped, the hole of every
//! such pad on the bottom lands in the middle of its land.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::{Layer, PadShape, Rotation};
use cypcb_world::footprint::{mirrored_to_bottom, Footprint, PadDef};

fn footprint(rotation: Rotation) -> Footprint {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(8.0), Nm::from_mm(8.0)));
    Footprint {
        placement_origin: None,
        name: "P".to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::from_mm(1.0, 2.0),
            size: (Nm::from_mm(2.0), Nm::from_mm(3.0)),
            drill: Some(Nm::from_mm(0.8)),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
            rotation,
            hole_offset: Some(Point::from_mm(0.5, 0.25)),
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

/// The mirror of a point: x reversed.
fn mirror(point: Point) -> Point {
    Point::new(Nm(-point.x.0), point.y)
}

#[test]
fn the_flipped_hole_is_the_original_flipped() {
    for rotation in [
        Rotation::ZERO,
        Rotation::DEG_90,
        Rotation::from_degrees(30.0),
    ] {
        let top = footprint(rotation);
        let bottom = mirrored_to_bottom(&top);
        let want = mirror(top.pads[0].hole_centre());
        let got = bottom.pads[0].hole_centre();
        assert!(
            (want.x.0 - got.x.0).abs() <= 2 && (want.y.0 - got.y.0).abs() <= 2,
            "pad turned {rotation:?}: the hole is {got:?}, the flip of the original is {want:?}"
        );
    }
}

#[test]
fn control_a_pad_with_its_hole_in_the_middle_flips_with_it() {
    let mut top = footprint(Rotation::ZERO);
    top.pads[0].hole_offset = None;
    let bottom = mirrored_to_bottom(&top);
    assert_eq!(bottom.pads[0].hole_offset, None);
    assert_eq!(bottom.pads[0].hole_centre(), mirror(top.pads[0].position));
}
