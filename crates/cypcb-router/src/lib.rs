//! Autorouting Integration
//!
//! Integrates with FreeRouting autorouter via DSN/SES file exchange.
//!
//! # Workflow
//!
//! 1. Export board to DSN format using [`export_dsn`]
//! 2. Run FreeRouting CLI using [`FreeRoutingRunner`]
//! 3. Import routes from SES format using [`import_ses`]
//! 4. Apply routes to board using [`apply_routes`]
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_router::{export_dsn, import_ses, apply_routes, FreeRoutingRunner, RoutingConfig};
//! use std::fs::File;
//!
//! // Export board to DSN
//! let mut dsn_file = File::create("board.dsn")?;
//! export_dsn(&world, &library, &mut dsn_file)?;
//!
//! // Run FreeRouting
//! let config = RoutingConfig::new("freerouting.jar".into());
//! let runner = FreeRoutingRunner::new(config);
//! let result = runner.route(&dsn_path, &ses_path, &net_lookup)?;
//!
//! // Apply routes to board
//! apply_routes(&mut world, &result);
//! ```
//!
//! # DSN Format
//!
//! The Specctra DSN (Design) format is the standard input format for
//! FreeRouting and other autorouters. It contains:
//!
//! - Board boundary and layer definitions
//! - Component placements
//! - Footprint library (padstacks)
//! - Network (nets and pin connections)
//! - Design rules (clearances, widths)
//! - Existing wiring (locked traces)
//!
//! # SES Format
//!
//! The Specctra SES (Session) format contains routing results:
//!
//! - Routed wire paths
//! - Via placements
//! - Routing statistics

pub mod dsn;
pub mod freerouting;
pub mod ses;
pub mod types;

use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::{BoardWorld, Entity};

pub use dsn::{export_dsn, DsnExportError};
pub use freerouting::{FreeRoutingRunner, RoutingConfig, RoutingError, RoutingProgress};
pub use ses::{import_ses, import_ses_from_str, SesImportError};
pub use types::{calculate_metrics, RouteSegment, RoutingMetrics, RoutingResult, RoutingStatus, ViaPlacement};

/// Apply routing results to a BoardWorld.
///
/// This function:
/// 1. Removes existing autorouted traces (where source == Autorouted)
/// 2. Creates new Trace entities from the routing result
/// 3. Creates new Via entities from the routing result
///
/// Locked traces (where locked == true) are preserved.
///
/// # Arguments
///
/// * `world` - The BoardWorld to modify
/// * `result` - The routing result to apply
///
/// # Example
///
/// ```rust,ignore
/// use cypcb_router::{apply_routes, RoutingResult};
///
/// let result = import_ses(&ses_path, &net_lookup)?;
/// apply_routes(&mut world, &result);
/// ```
pub fn apply_routes(world: &mut BoardWorld, result: &RoutingResult) {
    // Remove existing autorouted traces (not locked)
    let entities_to_remove: Vec<Entity> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Trace)>();
        query
            .iter(ecs)
            .filter(|(_, trace)| trace.source == TraceSource::Autorouted && !trace.locked)
            .map(|(entity, _)| entity)
            .collect()
    };

    // Also remove existing autorouted vias (not locked)
    let via_entities_to_remove: Vec<Entity> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Via)>();
        query
            .iter(ecs)
            .filter(|(_, via)| !via.locked)
            .map(|(entity, _)| entity)
            .collect()
    };

    // Despawn old entities
    let ecs = world.ecs_mut();
    for entity in entities_to_remove {
        ecs.despawn(entity);
    }
    for entity in via_entities_to_remove {
        ecs.despawn(entity);
    }

    // Group route segments by net and layer to create Trace entities
    use std::collections::HashMap;

    let mut traces_by_key: HashMap<(cypcb_world::NetId, cypcb_world::Layer), Vec<TraceSegment>> =
        HashMap::new();
    let mut trace_widths: HashMap<(cypcb_world::NetId, cypcb_world::Layer), cypcb_core::Nm> =
        HashMap::new();

    for segment in &result.routes {
        let key = (segment.net_id, segment.layer);

        traces_by_key.entry(key).or_default().push(TraceSegment::new(
            segment.start,
            segment.end,
        ));

        trace_widths.entry(key).or_insert(segment.width);
    }

    // Create Trace entities
    for ((net_id, layer), segments) in traces_by_key {
        let width = trace_widths[&(net_id, layer)];

        let trace = Trace {
            segments,
            width,
            layer,
            net_id,
            locked: false,
            source: TraceSource::Autorouted,
        };

        world.spawn_entity(trace);
    }

    // Create Via entities
    for via_placement in &result.vias {
        let via = Via {
            position: via_placement.position,
            drill: via_placement.drill,
            outer_diameter: cypcb_core::Nm(via_placement.drill.0 * 2), // Default annular ring
            start_layer: via_placement.start_layer,
            end_layer: via_placement.end_layer,
            net_id: via_placement.net_id,
            locked: false,
        };

        world.spawn_entity(via);
    }
}

/// Get all locked trace entities from a BoardWorld.
///
/// Returns entities that should be preserved during re-routing.
/// These traces have their `locked` field set to `true`.
///
/// # Example
///
/// ```rust,ignore
/// let locked = preserve_locked_traces(&mut world);
/// println!("Preserving {} locked traces", locked.len());
/// ```
pub fn preserve_locked_traces(world: &mut BoardWorld) -> Vec<Entity> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(Entity, &Trace)>();

    query
        .iter(ecs)
        .filter(|(_, trace)| trace.locked)
        .map(|(entity, _)| entity)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::{Nm, Point};
    use cypcb_world::{Layer, NetId};

    #[test]
    fn test_apply_routes_empty_world() {
        let mut world = BoardWorld::new();

        let result = RoutingResult::default();
        apply_routes(&mut world, &result);

        // No traces should exist
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        assert_eq!(query.iter(ecs).count(), 0);
    }

    #[test]
    fn test_apply_routes_adds_traces() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        let routes = vec![
            RouteSegment::new(
                vcc,
                Layer::TopCopper,
                Nm::from_mm(0.2),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
        ];

        let result = RoutingResult::complete(routes, Vec::new());
        apply_routes(&mut world, &result);

        // Should have 1 trace
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        let traces: Vec<_> = query.iter(ecs).collect();
        assert_eq!(traces.len(), 1);

        let trace = traces[0];
        assert_eq!(trace.net_id, vcc);
        assert_eq!(trace.layer, Layer::TopCopper);
        assert_eq!(trace.source, TraceSource::Autorouted);
        assert!(!trace.locked);
    }

    #[test]
    fn test_apply_routes_adds_vias() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        let vias = vec![ViaPlacement::through_hole(
            vcc,
            Point::from_mm(5.0, 5.0),
            Nm::from_mm(0.3),
        )];

        let result = RoutingResult::complete(Vec::new(), vias);
        apply_routes(&mut world, &result);

        // Should have 1 via
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        let vias: Vec<_> = query.iter(ecs).collect();
        assert_eq!(vias.len(), 1);

        let via = vias[0];
        assert_eq!(via.net_id, vcc);
        assert_eq!(via.position, Point::from_mm(5.0, 5.0));
        assert!(!via.locked);
    }

    #[test]
    fn test_apply_routes_replaces_autorouted() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        // Add an existing autorouted trace
        let existing = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(100.0, 100.0),
                Point::from_mm(200.0, 100.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: false,
            source: TraceSource::Autorouted,
        };
        world.spawn_entity(existing);

        // Apply new routes
        let routes = vec![RouteSegment::new(
            vcc,
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 0.0),
        )];

        let result = RoutingResult::complete(routes, Vec::new());
        apply_routes(&mut world, &result);

        // Should still have only 1 trace (old one replaced)
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        let traces: Vec<_> = query.iter(ecs).collect();
        assert_eq!(traces.len(), 1);

        // Verify it's the new trace (starts at 0,0)
        let trace = traces[0];
        assert_eq!(trace.segments[0].start, Point::from_mm(0.0, 0.0));
    }

    #[test]
    fn test_apply_routes_preserves_locked() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        // Add a locked trace
        let locked = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(100.0, 100.0),
                Point::from_mm(200.0, 100.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: true, // Locked!
            source: TraceSource::Manual,
        };
        world.spawn_entity(locked);

        // Apply new routes (empty - just testing preservation)
        let result = RoutingResult::default();
        apply_routes(&mut world, &result);

        // Locked trace should still exist
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        let traces: Vec<_> = query.iter(ecs).collect();
        assert_eq!(traces.len(), 1);
        assert!(traces[0].locked);
    }

    #[test]
    fn test_preserve_locked_traces() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        // Add a locked trace
        let locked = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: true,
            source: TraceSource::Manual,
        };
        world.spawn_entity(locked);

        // Add an unlocked trace
        let unlocked = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(50.0, 50.0),
                Point::from_mm(60.0, 50.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: false,
            source: TraceSource::Autorouted,
        };
        world.spawn_entity(unlocked);

        let preserved = preserve_locked_traces(&mut world);
        assert_eq!(preserved.len(), 1);
    }

    #[test]
    fn test_apply_routes_groups_segments_by_net() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Create routes for two nets
        let routes = vec![
            RouteSegment::new(
                vcc,
                Layer::TopCopper,
                Nm::from_mm(0.2),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
            RouteSegment::new(
                vcc,
                Layer::TopCopper,
                Nm::from_mm(0.2),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(10.0, 10.0),
            ),
            RouteSegment::new(
                gnd,
                Layer::BottomCopper,
                Nm::from_mm(0.3),
                Point::from_mm(20.0, 20.0),
                Point::from_mm(30.0, 20.0),
            ),
        ];

        let result = RoutingResult::complete(routes, Vec::new());
        apply_routes(&mut world, &result);

        // Should have 2 traces (one for VCC on top, one for GND on bottom)
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        let traces: Vec<_> = query.iter(ecs).collect();
        assert_eq!(traces.len(), 2);

        // VCC trace should have 2 segments
        let vcc_trace = traces.iter().find(|t| t.net_id == vcc).unwrap();
        assert_eq!(vcc_trace.segments.len(), 2);

        // GND trace should have 1 segment
        let gnd_trace = traces.iter().find(|t| t.net_id == gnd).unwrap();
        assert_eq!(gnd_trace.segments.len(), 1);
    }
}
