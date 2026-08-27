//! Where a stitching via may sit, and where it may not.
//!
//! `cargo test -p cypcb-world --test a_pour_can_be_stitched`
//!
//! A plane on a two-layer board is two planes, and a field of vias is what
//! makes them one. The rule is small: a via belongs where the pour is and
//! where nothing else is. A via on a track is a short; a via too close to a
//! foreign pad is a board a fabricator will make and a tester will fail.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::stitch::{stitching_vias, StitchSpec};
use cypcb_world::BoardWorld;

fn mm(value: f64) -> Nm {
    Nm((value * 1_000_000.0) as i64)
}

fn pour(world: &mut BoardWorld, min: (f64, f64), max: (f64, f64)) -> Zone {
    let net = world.intern_net("GND");
    Zone {
        bounds: Rect::new(
            Point::new(mm(min.0), mm(min.1)),
            Point::new(mm(max.0), mm(max.1)),
        ),
        kind: ZoneKind::CopperPour,
        layer_mask: 0b11,
        name: Some("gnd_pour".to_string()),
        net: Some(net),
    }
}

#[test]
fn an_empty_pour_is_stitched_on_its_pitch() {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let zone = pour(&mut world, (0.0, 0.0), (10.0, 10.0));

    let placed = stitching_vias(&mut world, &library, &zone, StitchSpec::at(mm(5.0)));

    // A 10mm square at a 5mm pitch, starting half a pitch in: 2 by 2.
    assert_eq!(placed.len(), 4, "the field is two by two: {placed:?}");
    for point in &placed {
        assert!(
            point.x.0 > 0 && point.x.0 < mm(10.0).0,
            "every via is inside the pour: {point:?}"
        );
    }
}

#[test]
fn a_finer_pitch_puts_more_of_them_in() {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let zone = pour(&mut world, (0.0, 0.0), (10.0, 10.0));

    let coarse = stitching_vias(&mut world, &library, &zone, StitchSpec::at(mm(5.0)));
    let fine = stitching_vias(&mut world, &library, &zone, StitchSpec::at(mm(2.0)));
    assert!(
        fine.len() > coarse.len(),
        "a 2mm pitch is denser than a 5mm one: {} against {}",
        fine.len(),
        coarse.len()
    );
}

#[test]
fn a_pour_smaller_than_one_via_gets_none() {
    // A via needs its ring and its clearance inside the copper. A pour that
    // cannot hold one is not stitched rather than stitched badly.
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let zone = pour(&mut world, (0.0, 0.0), (0.5, 0.5));

    let placed = stitching_vias(&mut world, &library, &zone, StitchSpec::at(mm(0.4)));
    assert!(placed.is_empty(), "nothing fits: {placed:?}");
}

#[test]
fn a_via_never_lands_on_foreign_copper() {
    // The rule that matters. A hole on somebody else's track is a short, and
    // the fabricator will drill it exactly where the file says.
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
    use cypcb_world::Layer;

    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let zone = pour(&mut world, (0.0, 0.0), (10.0, 10.0));

    let signal = world.intern_net("SIG");
    world.ecs_mut().spawn(Trace {
        segments: vec![TraceSegment {
            start: Point::new(mm(0.0), mm(5.0)),
            end: Point::new(mm(10.0), mm(5.0)),
            width: None,
        }],
        width: mm(0.5),
        layer: Layer::TopCopper,
        net_id: signal,
        locked: false,
        source: TraceSource::Manual,
    });

    let spec = StitchSpec::at(mm(2.0));
    let placed = stitching_vias(&mut world, &library, &zone, spec);
    assert!(!placed.is_empty(), "the rest of the pour is still stitched");

    // Nothing within the via's ring plus its clearance of that track.
    let keep = (spec.diameter.0 / 2 + spec.clearance.0) as f64;
    let track_half = mm(0.5).0 as f64 / 2.0;
    for point in &placed {
        let gap = (point.y.0 as f64 - mm(5.0).0 as f64).abs() - track_half;
        assert!(
            gap >= keep,
            "a via sits {gap} from the track and needs {keep}: {point:?}"
        );
    }
}

#[test]
fn a_keepout_is_not_stitched() {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let mut zone = pour(&mut world, (0.0, 0.0), (10.0, 10.0));
    zone.kind = ZoneKind::Keepout;
    zone.net = None;

    let placed = stitching_vias(&mut world, &library, &zone, StitchSpec::at(mm(5.0)));
    assert!(
        placed.is_empty(),
        "a keepout is the absence of copper, so there is nothing to tie together"
    );
}

#[test]
fn a_pitch_of_nothing_places_nothing() {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let zone = pour(&mut world, (0.0, 0.0), (10.0, 10.0));

    let placed = stitching_vias(&mut world, &library, &zone, StitchSpec::at(Nm(0)));
    assert!(
        placed.is_empty(),
        "a pitch of zero would loop for ever rather than fill the pour"
    );
}
