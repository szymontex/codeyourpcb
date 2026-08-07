//! A courtyard smaller than its own copper is a lie every consumer believes.
//!
//! `cargo test -p cypcb-world --test the_courtyard_encloses_the_land_pattern`
//!
//! IPC-7351 puts the courtyard around everything the part occupies - the body
//! and the land pattern - plus an excess. The built-in footprints each derived
//! theirs from the body alone, which is narrower than the pads on every
//! two-terminal chip part there is. Two consumers read that rectangle and
//! believed it: `courtyard-clearance` as the room a part needs, and the
//! designator layout as the height a printed name has to clear. Both
//! under-reported by whatever the pads overhung.
//!
//! This checks the property rather than the numbers: every footprint in the
//! library has to contain its own pads, whatever its dimensions are, so a
//! footprint added tomorrow cannot reintroduce this.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef, IPC_COURTYARD_EXCESS};

/// The box a pad's copper occupies, in footprint coordinates.
fn pad_extent(pad: &PadDef) -> Rect {
    Rect::from_center_size(pad.position, pad.size)
}

#[test]
fn every_built_in_footprint_keeps_its_pads_inside_its_courtyard() {
    let library = FootprintLibrary::new();
    let mut short = Vec::new();

    for (name, footprint) in library.iter() {
        let court = &footprint.courtyard;
        for pad in &footprint.pads {
            let pad_box = pad_extent(pad);
            if pad_box.min.x.raw() < court.min.x.raw()
                || pad_box.min.y.raw() < court.min.y.raw()
                || pad_box.max.x.raw() > court.max.x.raw()
                || pad_box.max.y.raw() > court.max.y.raw()
            {
                short.push(format!(
                    "{name} pad {}: copper spans x {}..{} y {}..{}, courtyard only x {}..{} y {}..{}",
                    pad.number,
                    pad_box.min.x.raw(),
                    pad_box.max.x.raw(),
                    pad_box.min.y.raw(),
                    pad_box.max.y.raw(),
                    court.min.x.raw(),
                    court.max.x.raw(),
                    court.min.y.raw(),
                    court.max.y.raw(),
                ));
            }
        }
    }

    assert!(
        short.is_empty(),
        "courtyards that do not enclose their own land pattern:\n{}",
        short.join("\n")
    );
}

#[test]
fn every_built_in_footprint_leaves_the_ipc_excess_around_its_copper() {
    // Containing the pads is not enough: a courtyard flush with the copper
    // gives a placement machine nothing to work with. Every footprint has to
    // stand off by the excess, in both axes.
    let library = FootprintLibrary::new();
    let excess = IPC_COURTYARD_EXCESS.raw();
    let mut tight = Vec::new();

    for (name, footprint) in library.iter() {
        if footprint.pads.is_empty() {
            continue;
        }
        let court = &footprint.courtyard;
        let mut min_x = i64::MAX;
        let mut min_y = i64::MAX;
        let mut max_x = i64::MIN;
        let mut max_y = i64::MIN;
        for pad in &footprint.pads {
            let pad_box = pad_extent(pad);
            min_x = min_x.min(pad_box.min.x.raw());
            min_y = min_y.min(pad_box.min.y.raw());
            max_x = max_x.max(pad_box.max.x.raw());
            max_y = max_y.max(pad_box.max.y.raw());
        }

        let gaps = [
            min_x - court.min.x.raw(),
            min_y - court.min.y.raw(),
            court.max.x.raw() - max_x,
            court.max.y.raw() - max_y,
        ];
        if gaps.iter().any(|gap| *gap < excess) {
            tight.push(format!(
                "{name}: gaps {gaps:?}, need {excess} on every side"
            ));
        }
    }

    assert!(
        tight.is_empty(),
        "courtyards standing off less than the IPC excess:\n{}",
        tight.join("\n")
    );
}

#[test]
fn a_part_with_no_pads_still_gets_a_courtyard_around_its_body() {
    // A mechanical part - a mounting hole, a fiducial - has a body and no
    // land pattern. It still occupies space on the board.
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(4.0), Nm::from_mm(4.0)));
    let mechanical = Footprint {
        name: "spacer".into(),
        description: "no copper".into(),
        pads: Vec::new(),
        bounds: body,
        courtyard: Rect::default(),
        silk: Vec::new(),
    }
    .with_ipc_courtyard();

    let excess = IPC_COURTYARD_EXCESS.raw();
    assert_eq!(mechanical.courtyard.min.x.raw(), body.min.x.raw() - excess);
    assert_eq!(mechanical.courtyard.max.y.raw(), body.max.y.raw() + excess);
}

#[test]
fn the_0805_courtyard_grew_to_cover_the_pads_it_used_to_cut_through() {
    // The case that started this: body 2.0 x 1.25mm, pads spanning 2.9mm.
    // The old courtyard was body + 0.25mm per side - 2.5mm wide - and the
    // copper stuck out 0.2mm each side of it.
    let library = FootprintLibrary::new();
    let chip = library.get("0805").expect("0805 is built in");

    assert_eq!(
        chip.courtyard.width().raw(),
        Nm::from_mm(3.4).raw(),
        "2.9mm of land pattern plus 0.25mm each side"
    );
    assert_eq!(
        chip.courtyard.height().raw(),
        Nm::from_mm(1.95).raw(),
        "1.45mm of pad height plus 0.25mm each side"
    );
    assert!(
        chip.courtyard.width().raw() > chip.bounds.width().raw(),
        "the pads are wider than the body, so the courtyard has to be too"
    );
}
