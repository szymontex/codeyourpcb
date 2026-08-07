//! DRC Integration Test Suite — Sandbox
//!
//! End-to-end tests that build realistic board scenarios and verify
//! all DRC rules fire correctly. Run with:
//!
//!   cargo test -p cypcb-drc --test drc_sandbox
//!
//! Each test group covers one DRC rule with both passing and failing
//! scenarios, plus edge cases.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::{run_drc, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{
    FootprintRef, NetConnections, NetId, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::{BoardWorld, Layer, SpatialEntry};

// ============================================================================
// Helpers
// ============================================================================

/// Create a board world with a 50x50mm board outline.
fn world_with_board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("test".into(), (Nm::from_mm(50.0), Nm::from_mm(50.0)), 2);
    world
}

/// Spawn a simple 2-pin component at a given position with net connections.
fn spawn_component(
    world: &mut BoardWorld,
    refdes: &str,
    pos_mm: (f64, f64),
    pin1_net: NetId,
    pin2_net: NetId,
) -> Entity {
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1", pin1_net));
    nets.add(PinConnection::new("2", pin2_net));
    world.spawn_component(
        RefDes::new(refdes),
        Value::new("10k"),
        Position::from_mm(pos_mm.0, pos_mm.1),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        nets,
    )
}

/// Spawn a through-hole component (2-pin header, 2.54mm pitch, 1.0mm drills).
fn spawn_tht_component(world: &mut BoardWorld, refdes: &str, pos_mm: (f64, f64)) -> Entity {
    let net = world.intern_net("THT");
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1", net));
    nets.add(PinConnection::new("2", net));
    world.spawn_component(
        RefDes::new(refdes),
        Value::new(""),
        Position::from_mm(pos_mm.0, pos_mm.1),
        Rotation::ZERO,
        FootprintRef::new("PIN-HDR-1x2"),
        nets,
    )
}

/// Spawn a trace on a given net/layer, returning entity.
fn spawn_trace(
    world: &mut BoardWorld,
    net_id: NetId,
    layer: Layer,
    width_mm: f64,
    start_mm: (f64, f64),
    end_mm: (f64, f64),
) -> Entity {
    let trace = Trace {
        segments: vec![TraceSegment::new(
            Point::from_mm(start_mm.0, start_mm.1),
            Point::from_mm(end_mm.0, end_mm.1),
        )],
        width: Nm::from_mm(width_mm),
        layer,
        net_id,
        locked: false,
        source: TraceSource::Manual,
    };
    world.spawn_entity((trace, net_id))
}

/// Spawn a via at a given position.
fn spawn_via(
    world: &mut BoardWorld,
    net_id: NetId,
    pos_mm: (f64, f64),
    drill_mm: f64,
    outer_mm: f64,
) -> Entity {
    let via = Via {
        position: Point::from_mm(pos_mm.0, pos_mm.1),
        drill: Nm::from_mm(drill_mm),
        outer_diameter: Nm::from_mm(outer_mm),
        start_layer: Layer::TopCopper,
        end_layer: Layer::BottomCopper,
        net_id,
        locked: false,
    };
    world.spawn_entity((via, net_id))
}

/// Rebuild spatial index from traces, vias, and component pad entries.
/// Simplified version — indexes traces and vias, plus any manually added entries.
fn rebuild_spatial(world: &mut BoardWorld, extra_entries: Vec<SpatialEntry>) {
    let mut entries = extra_entries;

    // Index traces
    {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
        let traces: Vec<_> = query
            .iter(ecs)
            .map(|(e, t)| {
                let segs: Vec<_> = t.segments.iter().map(|s| (s.start, s.end)).collect();
                (e, t.width.0, t.layer.to_copper_mask(), segs)
            })
            .collect();

        for (entity, width, layer_mask, segs) in &traces {
            let hw = width / 2;
            for (start, end) in segs {
                entries.push(SpatialEntry::from_raw(
                    *entity,
                    start.x.0.min(end.x.0) - hw,
                    start.y.0.min(end.y.0) - hw,
                    start.x.0.max(end.x.0) + hw,
                    start.y.0.max(end.y.0) + hw,
                    *layer_mask,
                ));
            }
        }
    }

    // Index vias
    {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
        let vias: Vec<_> = query
            .iter(ecs)
            .map(|(e, v)| {
                (
                    e,
                    v.position,
                    v.outer_diameter.0 / 2,
                    v.start_layer.to_copper_mask() | v.end_layer.to_copper_mask(),
                )
            })
            .collect();

        for (entity, pos, radius, layer_mask) in &vias {
            entries.push(SpatialEntry::from_raw(
                *entity,
                pos.x.0 - radius,
                pos.y.0 - radius,
                pos.x.0 + radius,
                pos.y.0 + radius,
                *layer_mask,
            ));
        }
    }

    world
        .ecs_mut()
        .resource_mut::<cypcb_world::SpatialIndex>()
        .rebuild(entries);
}

fn count_violations(world: &mut BoardWorld, rules: &DesignRules, kind: ViolationKind) -> usize {
    let result = run_drc(world, rules);
    result.violations.iter().filter(|v| v.kind == kind).count()
}

// ============================================================================
// 1. CLEARANCE — copper-to-copper distance
// ============================================================================

mod clearance {
    use super::*;

    #[test]
    fn same_net_traces_no_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Two traces on the same net overlapping — should be fine
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            (15.0, 10.0),
            (25.0, 10.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ),
            0
        );
    }

    #[test]
    fn different_net_traces_too_close() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        // Two traces 0.05mm apart — violates 0.15mm clearance
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        spawn_trace(
            &mut world,
            gnd,
            Layer::TopCopper,
            0.2,
            (10.0, 10.25),
            (20.0, 10.25),
        );
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ) > 0
        );
    }

    #[test]
    fn different_net_traces_far_apart_ok() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        // Two traces 5mm apart — plenty of room
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        spawn_trace(
            &mut world,
            gnd,
            Layer::TopCopper,
            0.2,
            (10.0, 15.0),
            (20.0, 15.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ),
            0
        );
    }

    #[test]
    fn different_layers_no_violation() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        // Overlapping traces on different layers — no copper interaction
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        spawn_trace(
            &mut world,
            gnd,
            Layer::BottomCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ),
            0
        );
    }

    #[test]
    fn trace_to_pad_same_net_no_violation() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Component with pin 1 on VCC
        let comp = spawn_component(&mut world, "R1", (10.0, 10.0), vcc, gnd);

        // Pad entity for pin 1 (simulating what PcbEngine does)
        let pad_entity = world.spawn_entity((
            cypcb_world::PadInstance::new(comp),
            vcc,
            Position::from_mm(10.0, 10.0),
        ));

        // Trace on VCC touching the pad — should NOT violate
        let _trace = spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );

        let pad_entry = SpatialEntry::from_raw(
            pad_entity,
            Nm::from_mm(9.5).0,
            Nm::from_mm(9.5).0,
            Nm::from_mm(10.5).0,
            Nm::from_mm(10.5).0,
            Layer::TopCopper.to_copper_mask(),
        );
        rebuild_spatial(&mut world, vec![pad_entry]);

        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ),
            0
        );
    }

    #[test]
    fn trace_to_pad_different_net_violation() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        let sig = world.intern_net("SIG");

        let comp = spawn_component(&mut world, "R1", (10.0, 10.0), vcc, gnd);

        // Pad entity for pin 1 (VCC)
        let pad_entity = world.spawn_entity((
            cypcb_world::PadInstance::new(comp),
            vcc,
            Position::from_mm(10.0, 10.0),
        ));

        // Trace on SIG touching the VCC pad — should violate
        spawn_trace(
            &mut world,
            sig,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );

        let pad_entry = SpatialEntry::from_raw(
            pad_entity,
            Nm::from_mm(9.5).0,
            Nm::from_mm(9.5).0,
            Nm::from_mm(10.5).0,
            Nm::from_mm(10.5).0,
            Layer::TopCopper.to_copper_mask(),
        );
        rebuild_spatial(&mut world, vec![pad_entry]);

        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::Clearance
            ) > 0
        );
    }
}

// ============================================================================
// 2. EDGE CLEARANCE — copper to board edge
// ============================================================================

mod edge_clearance {
    use super::*;

    #[test]
    fn trace_near_edge_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("SIG");
        // Trace at x=0.1mm — within 0.3mm of left edge
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.15,
            (0.1, 25.0),
            (0.1, 30.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::EdgeClearance
            ) > 0
        );
    }

    #[test]
    fn trace_centered_no_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("SIG");
        // Trace at center — 25mm from any edge
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.15,
            (20.0, 25.0),
            (30.0, 25.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::EdgeClearance
            ),
            0
        );
    }
}

// ============================================================================
// 3. TRACE WIDTH — minimum copper width
// ============================================================================

mod trace_width {
    use super::*;

    #[test]
    fn thin_trace_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("SIG");
        // 0.05mm trace — violates the 0.127mm minimum
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.05,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        rebuild_spatial(&mut world, vec![]);
        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());
        let tw = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::TraceWidth)
            .count();
        assert_eq!(tw, 1, "a 0.05mm trace is under every preset's minimum");
    }

    #[test]
    fn normal_trace_ok() {
        let mut world = world_with_board();
        let net = world.intern_net("SIG");
        // 0.2mm trace — above the 0.127mm minimum
        spawn_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::TraceWidth
            ),
            0
        );
    }
}

// ============================================================================
// 4. DRILL SIZE — minimum hole diameter
// ============================================================================

mod drill_size {
    use super::*;

    #[test]
    fn small_via_drill_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Via with 0.1mm drill — violates 0.2mm min_via_drill
        spawn_via(&mut world, net, (25.0, 25.0), 0.1, 0.4);
        rebuild_spatial(&mut world, vec![]);
        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());

        let drill_findings = result
            .violations
            .iter()
            .filter(|v| matches!(v.kind, ViolationKind::ViaDrill | ViolationKind::DrillSize))
            .count();
        assert_eq!(
            drill_findings, 1,
            "a 0.1mm drill is under the 0.2mm minimum and has to be reported: {:?}",
            result.violations
        );
    }

    #[test]
    fn normal_via_drill_ok() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Via with 0.3mm drill — above 0.2mm minimum
        spawn_via(&mut world, net, (25.0, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());

        assert!(
            !result
                .violations
                .iter()
                .any(|v| { matches!(v.kind, ViolationKind::ViaDrill | ViolationKind::DrillSize) }),
            "0.3mm clears the 0.2mm minimum: {:?}",
            result.violations
        );
    }
}

// ============================================================================
// 5. HOLE TO HOLE — minimum distance between drill holes
// ============================================================================

mod hole_to_hole {
    use super::*;

    #[test]
    fn vias_too_close_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Two vias 0.5mm apart center-to-center, 0.3mm drill each
        // Edge-to-edge = 0.5 - 0.15 - 0.15 = 0.2mm — violates 0.5mm hole-to-hole
        spawn_via(&mut world, net, (25.0, 25.0), 0.3, 0.6);
        spawn_via(&mut world, net, (25.5, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::HoleToHole
            ) > 0
        );
    }

    #[test]
    fn via_too_close_to_a_through_hole_pad_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // PIN-HDR-1x2 sits at (25, 25) with its first pad on the component
        // origin. A via 0.4mm away edge-to-edge is under the 0.5mm minimum.
        spawn_tht_component(&mut world, "J1", (25.0, 25.0));
        spawn_via(&mut world, net, (25.4, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::HoleToHole
            ) > 0,
            "a via next to a header pin is a hole-to-hole violation"
        );
    }

    #[test]
    fn a_footprints_own_pin_pitch_is_not_a_violation() {
        let mut world = world_with_board();
        // The 2.54mm pitch of a header is the footprint's business, not a board
        // defect - two pads of the same component must not report each other.
        spawn_tht_component(&mut world, "J1", (25.0, 25.0));
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::HoleToHole
            ),
            0
        );
    }

    #[test]
    fn vias_far_apart_ok() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Two vias 5mm apart — plenty of room
        spawn_via(&mut world, net, (20.0, 25.0), 0.3, 0.6);
        spawn_via(&mut world, net, (25.0, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::HoleToHole
            ),
            0
        );
    }

    #[test]
    fn three_vias_multiple_violations() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Three vias in a line, 0.4mm apart center-to-center
        // Edge-to-edge = 0.4 - 0.15 - 0.15 = 0.1mm each pair
        spawn_via(&mut world, net, (25.0, 25.0), 0.3, 0.6);
        spawn_via(&mut world, net, (25.4, 25.0), 0.3, 0.6);
        spawn_via(&mut world, net, (25.8, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        // At least 2 pairs should violate (A-B and B-C, possibly A-C too)
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::HoleToHole
            ) >= 2
        );
    }
}

// ============================================================================
// 6. VIA DIAMETER — minimum outer diameter
// ============================================================================

mod via_diameter {
    use super::*;

    #[test]
    fn small_via_violation() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Via with 0.3mm outer — violates 0.45mm minimum
        spawn_via(&mut world, net, (25.0, 25.0), 0.2, 0.3);
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::ViaDiameter
            ) > 0
        );
    }

    #[test]
    fn normal_via_ok() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        // Via with 0.6mm outer — above the 0.554mm minimum
        // (0.3mm drill + 2 x 0.127mm annular ring, from the JLCPCB constraints)
        spawn_via(&mut world, net, (25.0, 25.0), 0.3, 0.6);
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::ViaDiameter
            ),
            0
        );
    }

    #[test]
    fn multiple_vias_mixed() {
        let mut world = world_with_board();
        let net = world.intern_net("VCC");
        spawn_via(&mut world, net, (20.0, 25.0), 0.3, 0.6); // OK
        spawn_via(&mut world, net, (25.0, 25.0), 0.2, 0.3); // Too small
        spawn_via(&mut world, net, (30.0, 25.0), 0.2, 0.35); // Too small
        spawn_via(&mut world, net, (35.0, 25.0), 0.3, 0.5); // Too small: min is 0.554mm
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::ViaDiameter
            ),
            3
        );
    }
}

// ============================================================================
// 7. ANNULAR RING — minimum copper around drill
// ============================================================================

mod annular_ring {
    use super::*;

    #[test]
    fn empty_board_no_violation() {
        let mut world = world_with_board();
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(
            count_violations(&mut world, &rules, ViolationKind::AnnularRing),
            0
        );
    }

    // Note: annular ring rule checks footprint pads from the library,
    // which requires footprint data. Integration with PcbEngine is needed
    // for full coverage. Unit tests in annular_ring.rs cover the logic.
}

// ============================================================================
// 8. COURTYARD CLEARANCE — component overlap
// ============================================================================

mod courtyard_clearance {
    use super::*;

    #[test]
    fn overlapping_courtyards_violation() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Two components with overlapping courtyards (using spatial entries with layer_mask=0)
        let e1 = spawn_component(&mut world, "R1", (10.0, 10.0), vcc, gnd);
        let e2 = spawn_component(&mut world, "R2", (10.5, 10.0), vcc, gnd);

        // Courtyard entries (layer_mask=0 as set by rebuild_spatial_index_full)
        let entries = vec![
            SpatialEntry::new(e1, Point::from_mm(9.0, 9.0), Point::from_mm(11.0, 11.0), 0),
            SpatialEntry::new(e2, Point::from_mm(9.5, 9.0), Point::from_mm(11.5, 11.0), 0),
        ];
        rebuild_spatial(&mut world, entries);

        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::CourtyardClearance
            ) > 0
        );
    }

    #[test]
    fn separate_courtyards_ok() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        let e1 = spawn_component(&mut world, "R1", (10.0, 10.0), vcc, gnd);
        let e2 = spawn_component(&mut world, "R2", (20.0, 10.0), vcc, gnd);

        // Far apart courtyards
        let entries = vec![
            SpatialEntry::new(e1, Point::from_mm(9.0, 9.0), Point::from_mm(11.0, 11.0), 0),
            SpatialEntry::new(e2, Point::from_mm(19.0, 9.0), Point::from_mm(21.0, 11.0), 0),
        ];
        rebuild_spatial(&mut world, entries);

        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::CourtyardClearance
            ),
            0
        );
    }
}

// ============================================================================
// 9. CONNECTIVITY — unconnected pins
// ============================================================================

mod connectivity {
    use super::*;

    #[test]
    fn component_with_unconnected_pin() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");

        // Component with pin 1 on VCC, pin 2 unconnected (no net)
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", vcc));
        // Pin 2 intentionally not added — it's unconnected

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(25.0, 25.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );

        rebuild_spatial(&mut world, vec![]);
        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());
        let unconnected = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::UnconnectedPin)
            .count();
        assert_eq!(
            unconnected, 1,
            "pin 2 of an 0402 is on no net, and that is the whole point of this rule: {:?}",
            result.violations
        );
    }
}

// ============================================================================
// 10. KEEPOUT — components in forbidden zones
// ============================================================================

mod keepout {
    use super::*;

    #[test]
    fn component_in_keepout_violation() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: Some("antenna_area".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Component inside keepout
        spawn_component(&mut world, "R1", (15.0, 15.0), vcc, gnd);
        rebuild_spatial(&mut world, vec![]);

        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::KeepoutViolation
            ) > 0
        );
    }

    #[test]
    fn component_outside_keepout_ok() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: None,
            net: None,
        };
        world.ecs_mut().spawn(zone);

        spawn_component(&mut world, "R1", (30.0, 30.0), vcc, gnd);
        rebuild_spatial(&mut world, vec![]);

        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::KeepoutViolation
            ),
            0
        );
    }
}

// ============================================================================
// 11. SOLDER MASK BRIDGE (stub — infrastructure test only)
// ============================================================================

mod solder_mask {
    use super::*;

    #[test]
    fn empty_board_has_no_mask_bridges() {
        let mut world = world_with_board();
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::SolderMaskBridge
            ),
            0
        );
    }

    #[test]
    fn parts_placed_on_top_of_each_other_bridge() {
        let mut world = world_with_board();
        let net = world.intern_net("SIG");
        // 0402 pads are 0.6mm wide on a 1.0mm span; half a millimetre apart the
        // facing openings overlap outright.
        spawn_component(&mut world, "R1", (10.0, 15.0), net, net);
        spawn_component(&mut world, "R2", (10.5, 15.0), net, net);
        rebuild_spatial(&mut world, vec![]);
        assert!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::SolderMaskBridge
            ) > 0
        );
    }
}

// ============================================================================
// 12. SILK CLEARANCE (stub — infrastructure test only)
// ============================================================================

mod silk_clearance {
    use super::*;

    #[test]
    fn stub_returns_no_violations() {
        let mut world = world_with_board();
        rebuild_spatial(&mut world, vec![]);
        assert_eq!(
            count_violations(
                &mut world,
                &DesignRules::jlcpcb_2layer(),
                ViolationKind::SilkClearance
            ),
            0
        );
    }
}

// ============================================================================
// 13. PRESET COVERAGE — ensure all presets have sane values
// ============================================================================

mod presets {
    use super::*;
    use cypcb_drc::presets::Preset;

    #[test]
    fn all_presets_have_positive_values() {
        for preset in Preset::all() {
            let r = preset.rules();
            assert!(r.min_clearance.0 > 0, "{}: min_clearance", preset);
            assert!(r.min_trace_width.0 > 0, "{}: min_trace_width", preset);
            assert!(r.min_drill_size.0 > 0, "{}: min_drill_size", preset);
            assert!(r.min_via_drill.0 > 0, "{}: min_via_drill", preset);
            assert!(r.min_via_diameter.0 > 0, "{}: min_via_diameter", preset);
            assert!(r.min_annular_ring.0 > 0, "{}: min_annular_ring", preset);
            assert!(r.min_silk_width.0 > 0, "{}: min_silk_width", preset);
            assert!(r.min_edge_clearance.0 > 0, "{}: min_edge_clearance", preset);
            assert!(r.min_hole_to_hole.0 > 0, "{}: min_hole_to_hole", preset);
            assert!(
                r.min_solder_mask_bridge.0 > 0,
                "{}: min_solder_mask_bridge",
                preset
            );
            assert!(r.min_silk_clearance.0 > 0, "{}: min_silk_clearance", preset);
            assert!(
                r.min_courtyard_clearance.0 > 0,
                "{}: min_courtyard_clearance",
                preset
            );
        }
    }

    #[test]
    fn via_diameter_larger_than_via_drill() {
        for preset in Preset::all() {
            let r = preset.rules();
            assert!(
                r.min_via_diameter > r.min_via_drill,
                "{}: via_diameter ({:?}) must be > via_drill ({:?})",
                preset,
                r.min_via_diameter,
                r.min_via_drill,
            );
        }
    }

    #[test]
    fn prototype_most_relaxed() {
        let proto = DesignRules::prototype();
        for preset in Preset::all() {
            if *preset == Preset::Prototype {
                continue;
            }
            let r = preset.rules();
            assert!(
                proto.min_clearance >= r.min_clearance,
                "Prototype clearance should be >= {} ({})",
                r.min_clearance,
                preset,
            );
        }
    }
}

// ============================================================================
// 14. FULL BOARD SCENARIO — realistic design
// ============================================================================

mod full_board {
    use super::*;

    #[test]
    fn clean_board_no_violations() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Two components well-placed
        let c1 = spawn_component(&mut world, "R1", (15.0, 25.0), vcc, gnd);
        let c2 = spawn_component(&mut world, "C1", (35.0, 25.0), vcc, gnd);

        // Pad entities
        let pad1 = world.spawn_entity((
            cypcb_world::PadInstance::new(c1),
            vcc,
            Position::from_mm(15.0, 25.0),
        ));
        let pad2 = world.spawn_entity((
            cypcb_world::PadInstance::new(c2),
            vcc,
            Position::from_mm(35.0, 25.0),
        ));

        // Trace connecting them on VCC
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (15.0, 25.0),
            (35.0, 25.0),
        );

        // Well-placed via
        spawn_via(&mut world, gnd, (25.0, 15.0), 0.3, 0.6);

        // Spatial entries for pads + courtyard
        let entries = vec![
            SpatialEntry::from_raw(
                pad1,
                Nm::from_mm(14.5).0,
                Nm::from_mm(24.5).0,
                Nm::from_mm(15.5).0,
                Nm::from_mm(25.5).0,
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::from_raw(
                pad2,
                Nm::from_mm(34.5).0,
                Nm::from_mm(24.5).0,
                Nm::from_mm(35.5).0,
                Nm::from_mm(25.5).0,
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                c1,
                Point::from_mm(14.0, 24.0),
                Point::from_mm(16.0, 26.0),
                0,
            ),
            SpatialEntry::new(
                c2,
                Point::from_mm(34.0, 24.0),
                Point::from_mm(36.0, 26.0),
                0,
            ),
        ];
        rebuild_spatial(&mut world, entries);

        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());

        // Filter out the two rules that ask about the netlist and the copper
        // reaching it: this board is assembled by hand out of spatial entries,
        // with no footprint library behind it, so both answer about the
        // fixture rather than about the geometry under test.
        let real_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind != ViolationKind::UnconnectedPin)
            .filter(|v| v.kind != ViolationKind::UnroutedPin)
            .collect();

        assert!(
            real_violations.is_empty(),
            "Clean board should have no violations, got: {:?}",
            real_violations
                .iter()
                .map(|v| format!("{}: {}", v.kind, v.message))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn messy_board_multiple_violations() {
        let mut world = world_with_board();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Trace too close to edge
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.15,
            (0.1, 25.0),
            (0.1, 30.0),
        );

        // Two traces on different nets too close
        spawn_trace(
            &mut world,
            vcc,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (20.0, 10.0),
        );
        spawn_trace(
            &mut world,
            gnd,
            Layer::TopCopper,
            0.2,
            (10.0, 10.25),
            (20.0, 10.25),
        );

        // Undersized via
        spawn_via(&mut world, vcc, (30.0, 30.0), 0.2, 0.3);

        // Two vias too close
        spawn_via(&mut world, gnd, (40.0, 25.0), 0.3, 0.6);
        spawn_via(&mut world, gnd, (40.4, 25.0), 0.3, 0.6);

        rebuild_spatial(&mut world, vec![]);

        let result = run_drc(&mut world, &DesignRules::jlcpcb_2layer());

        // Should have at least: edge clearance + copper clearance + via diameter + hole-to-hole
        let edge = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::EdgeClearance)
            .count();
        let clearance = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::Clearance)
            .count();
        let via_dia = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::ViaDiameter)
            .count();
        let h2h = result
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::HoleToHole)
            .count();

        assert!(edge > 0, "Expected edge clearance violations");
        assert!(clearance > 0, "Expected clearance violations");
        assert!(via_dia > 0, "Expected via diameter violations");
        assert!(h2h > 0, "Expected hole-to-hole violations");
    }
}
