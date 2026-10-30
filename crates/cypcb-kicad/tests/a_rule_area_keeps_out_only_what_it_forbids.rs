//! A KiCad rule area keeps out what it forbids, and only that.
//!
//! `cargo test -p cypcb-kicad --test a_rule_area_keeps_out_only_what_it_forbids`
//!
//! A rule area has five switches - tracks, vias, pads, copper pour and
//! footprints - each `allowed` or `not_allowed`. The reader carried an area
//! that forbade any one of them as a keepout of every kind, so an area that
//! forbade only copper pour stopped the router, flagged the parts standing in
//! it, and was written back to KiCad forbidding everything. Measured on the
//! 132 boards of the test set: 41 areas on the board itself forbid something,
//! 25 of them copper pour alone and 11 a mix.
//!
//! What is held here: each switch is read as the kind it names; an area that
//! forbids all five carries no component, which is what a keepout without one
//! means; the writer states the real switches; and what is written comes back.
//! The 24 areas that sit inside a footprint are not read by this reader and are
//! not the subject.

use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbParseResult};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone};

fn board(zone: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
{zone}
)
"#
    )
}

const SWITCHES: [&str; 5] = ["tracks", "vias", "pads", "copperpour", "footprints"];

/// An area on `layer` with these switches `not_allowed` and the others
/// `allowed`: an L when `slanted_or_l`, a rectangle when not.
fn area(layer: &str, forbidden: &[&str], l_shape: bool) -> String {
    let switches: String = SWITCHES
        .iter()
        .map(|name| {
            let state = if forbidden.contains(name) {
                "not_allowed"
            } else {
                "allowed"
            };
            format!("({name} {state}) ")
        })
        .collect();
    let points = if l_shape {
        "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)"
    } else {
        "(xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115)"
    };
    format!(
        r#"(zone (net 0) (net_name "") (layer "{layer}") (name "area")
  (keepout {switches})
  (polygon (pts {points})))"#
    )
}

fn read(zone: &str) -> KicadPcbParseResult {
    parse_kicad_pcb_str(&board(zone)).expect("the board reads")
}

/// The component of each zone: `None` when the zone has none.
fn components(parsed: &mut KicadPcbParseResult) -> Vec<(Zone, Option<KeepoutKinds>)> {
    let ecs = parsed.world.ecs_mut();
    let mut query = ecs.query::<(&Zone, Option<&KeepoutKinds>)>();
    query
        .iter(ecs)
        .map(|(zone, kinds)| (zone.clone(), kinds.copied()))
        .collect()
}

/// What each keepout forbids, as the readers read it.
fn forbidden(parsed: &mut KicadPcbParseResult) -> Vec<Vec<KeepoutKind>> {
    components(parsed)
        .into_iter()
        .map(|(_, kinds)| {
            let kinds = KeepoutKinds::of(kinds.as_ref());
            KeepoutKind::ALL
                .into_iter()
                .filter(|kind| kinds.forbids(*kind))
                .collect()
        })
        .collect()
}

const KINDS: [KeepoutKind; 5] = [
    KeepoutKind::Tracks,
    KeepoutKind::Vias,
    KeepoutKind::Pads,
    KeepoutKind::Pour,
    KeepoutKind::Footprints,
];

#[test]
fn each_switch_alone_is_the_kind_it_names() {
    for (which, name) in SWITCHES.iter().enumerate() {
        for l_shape in [false, true] {
            let mut parsed = read(&area("F.Cu", &[name], l_shape));
            assert_eq!(
                forbidden(&mut parsed),
                vec![vec![KINDS[which]]],
                "{name} {l_shape}"
            );
        }
    }
}

#[test]
fn two_switches_are_two_kinds_and_not_the_rest() {
    let mut parsed = read(&area("B.Cu", &["copperpour", "tracks"], true));
    assert_eq!(
        forbidden(&mut parsed),
        vec![vec![KeepoutKind::Tracks, KeepoutKind::Pour]]
    );
}

#[test]
fn an_area_that_forbids_all_five_carries_no_component() {
    for l_shape in [false, true] {
        let mut parsed = read(&area("F.Cu", &SWITCHES, l_shape));
        let found = components(&mut parsed);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].1, None,
            "a keepout with no component forbids all five"
        );
        assert_eq!(forbidden(&mut parsed), vec![KINDS.to_vec()]);
    }
}

#[test]
fn a_switch_the_area_does_not_state_is_allowed() {
    // KiCad's two areas in the test set that state no `pads` switch.
    let text = r#"(zone (net 0) (net_name "") (layer "F.Cu")
  (keepout (tracks not_allowed) (vias allowed) (copperpour allowed) (footprints allowed))
  (polygon (pts (xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115))))"#;
    let mut parsed = read(text);
    assert_eq!(forbidden(&mut parsed), vec![vec![KeepoutKind::Tracks]]);
}

#[test]
fn what_is_written_comes_back() {
    // Every one of the 31 non-empty sets, rectangle and L, written to KiCad and
    // read again. A pour-only area is the case this exists for: it came back
    // forbidding everything.
    for bits in 1u32..32 {
        let names: Vec<&str> = SWITCHES
            .iter()
            .enumerate()
            .filter(|(i, _)| bits >> i & 1 == 1)
            .map(|(_, name)| *name)
            .collect();
        for l_shape in [false, true] {
            let mut parsed = read(&area("F.Cu", &names, l_shape));
            let before = forbidden(&mut parsed);
            let written = cypcb_kicad::write_board(&mut parsed.world, "test");
            let mut again = parse_kicad_pcb_str(&written).expect("what was written reads");
            assert_eq!(
                forbidden(&mut again),
                before,
                "{names:?} l={l_shape}\n{written}"
            );
        }
    }
}

#[test]
fn the_writer_states_the_real_switches() {
    let mut parsed = read(&area("F.Cu", &["copperpour"], true));
    let written = cypcb_kicad::write_board(&mut parsed.world, "test");
    assert!(
        written.contains(
            "(keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour not_allowed) (footprints allowed))"
        ),
        "{written}"
    );
}

#[test]
fn a_keepout_with_no_component_is_written_forbidding_all_five() {
    // What the language's `keepout` is, and every keepout before rule areas
    // could be carried one kind at a time.
    let mut parsed = read(&area("F.Cu", &SWITCHES, false));
    let written = cypcb_kicad::write_board(&mut parsed.world, "test");
    assert!(
        written.contains(
            "(keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed) (copperpour not_allowed) (footprints not_allowed))"
        ),
        "{written}"
    );
}

#[test]
fn the_zones_are_written_in_the_order_the_board_was_built() {
    // A keepout that forbids some kinds carries a component a pour does not, so
    // the two live in different archetypes, and walking the archetypes writes
    // the keepouts together and the pour after them. Built: area, pour, area.
    let text = format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "GND")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
{}
  (zone (net 1) (net_name "GND") (layer "B.Cu") (name "second")
    (polygon (pts (xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128))))
{}
)"#,
        area("F.Cu", &["copperpour"], true),
        area("F.Cu", &["tracks"], true),
    );
    let mut parsed = parse_kicad_pcb_str(&text).expect("the board reads");
    let written = cypcb_kicad::write_board(&mut parsed.world, "test");
    // The first keepout forbids pour alone, the pour is on GND, the third
    // keepout forbids tracks alone.
    let first = written.find("(copperpour not_allowed)").expect("the first");
    let second = written.find("(net_name \"GND\")").expect("the pour");
    let third = written.find("(tracks not_allowed)").expect("the third");
    assert!(first < second && second < third, "{written}");
}
