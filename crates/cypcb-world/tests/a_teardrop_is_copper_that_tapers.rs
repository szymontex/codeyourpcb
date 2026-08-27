//! The fillet where a track meets a pad.
//!
//! `cargo test -p cypcb-world --test a_teardrop_is_copper_that_tapers`
//!
//! A track meeting a pad at a right angle tears away there when the board is
//! drilled or flexed, which is why a fabricator asks for teardrops. The shape
//! is only worth having if it really spans from the pad's edge to the track's
//! own width some way along it, so that is what these read.

use cypcb_core::{Nm, Point};
use cypcb_world::teardrop::{inscribed_radius, teardrop, TeardropRatios};

fn mm(value: f64) -> Nm {
    Nm((value * 1_000_000.0) as i64)
}

fn at(x: f64, y: f64) -> Point {
    Point { x: mm(x), y: mm(y) }
}

/// Distance from the pad centre, in nanometres.
fn from_centre(centre: Point, p: Point) -> f64 {
    let dx = (p.x.0 - centre.x.0) as f64;
    let dy = (p.y.0 - centre.y.0) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn it_starts_on_the_pad_and_ends_at_the_track() {
    let centre = at(10.0, 10.0);
    let radius = inscribed_radius(mm(1.0), mm(1.6));
    assert_eq!(
        radius,
        mm(0.5),
        "the inscribed radius is half the short side"
    );

    let shape = teardrop(
        centre,
        radius,
        at(14.0, 10.0),
        mm(0.2),
        TeardropRatios::default(),
    )
    .expect("a 0.2mm track on a 1mm pad has room for a fillet");

    // The two pad-side corners sit on the pad's edge, and the two track-side
    // corners sit past it - that is what makes this a fillet rather than a
    // decoration inside the pad.
    let anchors = [from_centre(centre, shape[0]), from_centre(centre, shape[3])];
    for reach in anchors {
        assert!(
            (reach - radius.0 as f64).abs() < 1_000.0,
            "a pad-side corner is on the edge: {reach} against {}",
            radius.0
        );
    }

    let tips = [from_centre(centre, shape[1]), from_centre(centre, shape[2])];
    for reach in tips {
        assert!(
            reach > radius.0 as f64,
            "a track-side corner is past the pad edge: {reach} against {}",
            radius.0
        );
    }

    // The tip is the width of the track it supports.
    let dx = (shape[1].x.0 - shape[2].x.0) as f64;
    let dy = (shape[1].y.0 - shape[2].y.0) as f64;
    let tip_width = (dx * dx + dy * dy).sqrt();
    assert!(
        (tip_width - mm(0.2).0 as f64).abs() < 1_000.0,
        "the tip is the track's own width: {tip_width}"
    );
}

#[test]
fn it_follows_the_track_wherever_it_leaves() {
    // The same pad, a track leaving upwards: the fillet turns with it.
    let centre = at(0.0, 0.0);
    let radius = mm(0.5);
    let shape = teardrop(
        centre,
        radius,
        at(0.0, 3.0),
        mm(0.2),
        TeardropRatios::default(),
    )
    .expect("a fillet exists in this direction too");

    for corner in [shape[1], shape[2]] {
        assert!(
            corner.y.0 > radius.0,
            "the fillet reaches up the track, not sideways: {corner:?}"
        );
    }
}

#[test]
fn a_track_as_wide_as_the_pad_gets_nothing() {
    // A fillet narrower than its own track would pinch the track rather than
    // support it, so there is nothing honest to draw.
    let centre = at(0.0, 0.0);
    assert!(
        teardrop(
            centre,
            mm(0.5),
            at(2.0, 0.0),
            mm(1.0),
            TeardropRatios::default()
        )
        .is_none(),
        "a track the width of the pad has nothing to fillet"
    );
}

#[test]
fn a_track_that_states_no_direction_gets_nothing() {
    let centre = at(4.0, 4.0);
    assert!(
        teardrop(centre, mm(0.5), centre, mm(0.2), TeardropRatios::default()).is_none(),
        "a point on the pad centre names no direction"
    );
    assert!(
        teardrop(
            centre,
            Nm(0),
            at(9.0, 4.0),
            mm(0.2),
            TeardropRatios::default()
        )
        .is_none(),
        "a pad with no size has no edge to start from"
    );
}

#[test]
fn the_ratios_are_what_they_say() {
    let centre = at(0.0, 0.0);
    let radius = mm(0.5);
    let short = teardrop(
        centre,
        radius,
        at(3.0, 0.0),
        mm(0.2),
        TeardropRatios {
            length: 0.25,
            width: 0.9,
        },
    )
    .expect("a fillet at a quarter length");
    let long = teardrop(
        centre,
        radius,
        at(3.0, 0.0),
        mm(0.2),
        TeardropRatios {
            length: 1.0,
            width: 0.9,
        },
    )
    .expect("a fillet at full length");

    assert!(
        long[1].x.0 > short[1].x.0,
        "a longer ratio reaches further along the track: {} against {}",
        long[1].x.0,
        short[1].x.0
    );

    let narrow = teardrop(
        centre,
        radius,
        at(3.0, 0.0),
        mm(0.2),
        TeardropRatios {
            length: 0.5,
            width: 0.5,
        },
    )
    .expect("a fillet at half width");
    let wide = teardrop(
        centre,
        radius,
        at(3.0, 0.0),
        mm(0.2),
        TeardropRatios::default(),
    )
    .expect("a fillet at the default width");
    assert!(
        wide[0].y.0.abs() > narrow[0].y.0.abs(),
        "a wider ratio leaves the pad further from the track's line: {} against {}",
        wide[0].y.0,
        narrow[0].y.0
    );
}
