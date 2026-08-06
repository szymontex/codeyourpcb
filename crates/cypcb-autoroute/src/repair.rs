//! DRC-driven post-route repair.
//!
//! The router plans on a grid; the checker measures real copper. Where the two
//! disagree the board is illegal, and grid geometry has not closed that gap:
//! six measured attempts at expressing clearance as a grid-wide rule each
//! improved one benchmark board and worsened another by a similar amount.
//!
//! This pass works the other way round. Route, run the real DRC, take the
//! places the checker actually complained about, forbid exactly those cells,
//! route again. Nothing is assumed about why a violation happened - only where
//! it was, which is the one thing the checker is authoritative about.
//!
//! Two properties keep it honest:
//!
//! - A candidate is kept only when the violation count drops *and* the board is
//!   still complete, so repair can never buy a prettier number with an
//!   abandoned connection. The worst it can cost is time.
//! - How far around a violation to forbid is not a tuned constant. Each radius
//!   in [`AutorouteConfig::repair_block_radii`] is tried as its own attempt and
//!   the measured winner is kept, because radius 0 helps one benchmark board
//!   and radius 2 the other.

use std::collections::HashSet;

use bevy_ecs::entity::Entity;

use cypcb_core::Point;
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_router::apply_routes;
use cypcb_router::types::{RoutingResult, RoutingStatus};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::components::trace::Trace;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::grid::layer_to_index;
use crate::pathfinder_v2::PathFinderStrategy;
use crate::AutorouteConfig;

/// A place the router may not use, in board coordinates.
///
/// Produced from a DRC violation's location, so a blocker marks copper that was
/// measured illegal rather than copper a rule predicts might be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocker {
    /// Board position of the violation - the middle of the gap the checker
    /// measured, not a centroid of the entities involved.
    pub at: Point,
    /// Bit `i` set means copper layer index `i` is forbidden here.
    /// `u32::MAX` means every layer, used when the offending entity spans them.
    pub layers: u32,
    /// Radius in cells around `at`. Zero forbids the single cell.
    pub radius_cells: u32,
}

impl Blocker {
    /// Whether this blocker applies to the given copper layer index.
    #[inline]
    pub fn covers_layer(&self, layer: usize) -> bool {
        layer < 32 && self.layers & (1u32 << layer) != 0
    }
}

/// Repair a routed board against its own DRC report.
///
/// `initial` is the result of a first routing pass. Returns either that result
/// unchanged or a strictly better one: fewer violations, same completeness.
///
/// The world is left holding the returned result with its spatial index built,
/// so a caller that wants to score the board does not have to re-apply.
pub fn repair_routes(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
    initial: RoutingResult,
) -> RoutingResult {
    if config.repair_passes == 0 || config.repair_block_radii.is_empty() || !is_complete(&initial) {
        return initial;
    }

    let design_rules = DesignRules::from_constraints(rules.constraints_for_net(0));
    let resolution = PathFinderStrategy::resolution_for(world, rules, config);

    let mut best = initial;
    let (mut best_count, baseline_violations) = measure(world, library, &best, &design_rules);
    let before = best_count;
    if best_count == 0 {
        return best;
    }

    let mut accepted = 0u32;
    let mut winning_radius = None;

    for &radius in &config.repair_block_radii {
        // Every radius starts from the same board, so the attempts are
        // independent and comparable.
        let mut blockers = dedup(blockers_from(&baseline_violations, radius), resolution);

        for pass in 1..=config.repair_passes {
            let candidate =
                PathFinderStrategy.route_with_blockers(world, library, rules, config, &blockers);

            if !is_complete(&candidate) {
                tracing::info!(
                    radius,
                    pass,
                    blocked_cells = blockers.len(),
                    "Repair attempt rejected: the board no longer routes complete"
                );
                break;
            }

            let (count, violations) = measure(world, library, &candidate, &design_rules);
            tracing::info!(
                radius,
                pass,
                violations = count,
                best = best_count,
                blocked_cells = blockers.len(),
                "Repair attempt measured"
            );

            if count < best_count {
                best_count = count;
                best = candidate;
                accepted += 1;
                winning_radius = Some(radius);
            }
            if count == 0 {
                break;
            }

            // A rejected attempt still contributes its cells, which makes the
            // loop a fixed point on real DRC output rather than a search.
            blockers.extend(blockers_from(&violations, radius));
            blockers = dedup(blockers, resolution);
        }

        if best_count == 0 {
            break;
        }
    }

    tracing::info!(
        violations_before = before,
        violations_after = best_count,
        accepted_attempts = accepted,
        winning_radius,
        "Repair complete"
    );

    // The last candidate measured may have been rejected; leave the world
    // holding what we return.
    apply_routes(world, &best);
    world.rebuild_spatial_index_from_library(library);
    best
}

/// Apply a result, index it and run the real DRC over it.
///
/// Returns the count of violations this pass could act on, and for each of
/// them where it was and which copper layers it touched. The layers are
/// resolved here because a later attempt despawns these entities.
///
/// The count deliberately excludes everything rerouting cannot move - a part
/// placed off the board edge, silkscreen over a pad, a pin nobody connected.
/// Judging attempts on a total dominated by violations the pass is powerless
/// over is how a pass ends up looking like it failed when it did not, and on
/// a board where the fixed part of the total moves for its own reasons, how
/// it accepts an attempt that made its own work worse.
fn measure(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    result: &RoutingResult,
    design_rules: &DesignRules,
) -> (usize, Vec<Contact>) {
    apply_routes(world, result);
    world.rebuild_spatial_index_from_library(library);

    let report = run_drc(world, design_rules);
    let contacts: Vec<Contact> = report
        .violations
        .iter()
        .filter(|v| is_actionable(v.kind))
        .map(|v| {
            let mut layers = layer_mask(world, v.entity);
            if let Some(other) = v.other_entity {
                layers |= layer_mask(world, other);
            }
            Contact {
                at: v.location,
                layers,
            }
        })
        .collect();

    (contacts.len(), contacts)
}

/// Can rerouting copper change this kind of violation?
///
/// Only positional ones: moving a trace cannot widen it, drill a bigger hole,
/// move a component or connect a pin nobody wired.
fn is_actionable(kind: ViolationKind) -> bool {
    matches!(
        kind,
        ViolationKind::Clearance | ViolationKind::EdgeClearance | ViolationKind::HoleToHole
    )
}

/// A place the checker measured a violation, with the layers it touched.
#[derive(Debug, Clone, Copy)]
struct Contact {
    at: Point,
    layers: u32,
}

/// Turn measured contacts into cells the router may not use.
fn blockers_from(contacts: &[Contact], radius_cells: u32) -> Vec<Blocker> {
    contacts
        .iter()
        .map(|contact| Blocker {
            at: contact.at,
            layers: contact.layers,
            radius_cells,
        })
        .collect()
}

/// Which copper layers an entity occupies.
///
/// A trace lives on one layer. Everything else the checker can name - a via, a
/// through-hole pad, a component - reaches all of them.
fn layer_mask(world: &BoardWorld, entity: Entity) -> u32 {
    match world.ecs().get::<Trace>(entity) {
        Some(trace) => layer_to_index(trace.layer)
            .map(|index| 1u32 << index)
            .unwrap_or(u32::MAX),
        None => u32::MAX,
    }
}

/// Collapse blockers that land on the same cell, layers and radius.
///
/// Snapping to the routing grid before comparing is what makes the accumulated
/// list converge instead of growing every pass.
fn dedup(blockers: Vec<Blocker>, resolution: i64) -> Vec<Blocker> {
    let mut seen: HashSet<(i64, i64, u32, u32)> = HashSet::with_capacity(blockers.len());
    let mut out = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        let key = (
            blocker.at.x.raw().div_euclid(resolution),
            blocker.at.y.raw().div_euclid(resolution),
            blocker.layers,
            blocker.radius_cells,
        );
        if seen.insert(key) {
            out.push(blocker);
        }
    }
    out
}

/// Whether every connection was routed.
fn is_complete(result: &RoutingResult) -> bool {
    matches!(result.status, RoutingStatus::Complete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    fn blocker(layers: u32) -> Blocker {
        Blocker {
            at: Point::from_mm(1.0, 2.0),
            layers,
            radius_cells: 0,
        }
    }

    #[test]
    fn covers_layer_reads_the_mask() {
        let top = blocker(1 << 0);
        assert!(top.covers_layer(0));
        assert!(!top.covers_layer(1));

        let every = blocker(u32::MAX);
        assert!(every.covers_layer(0));
        assert!(every.covers_layer(31));
        assert!(!every.covers_layer(32), "layer index must stay in range");
    }

    #[test]
    fn dedup_collapses_within_one_cell() {
        let resolution = Nm::from_mm(0.254).raw();
        let make = |x: f64, radius: u32| Blocker {
            at: Point::from_mm(x, 1.0),
            layers: u32::MAX,
            radius_cells: radius,
        };

        let blockers = vec![
            make(1.000, 0),
            // 10µm away: the same cell on a 254µm grid.
            make(1.010, 0),
            // Same place, wider block: a distinct restriction.
            make(1.000, 2),
            // A cell away: distinct.
            make(1.400, 0),
        ];

        assert_eq!(dedup(blockers, resolution).len(), 3);
    }

    #[test]
    fn a_blocker_keeps_the_contact_it_came_from() {
        let contacts = [
            Contact {
                at: Point::from_mm(1.0, 1.0),
                layers: 1,
            },
            Contact {
                at: Point::from_mm(2.0, 2.0),
                layers: u32::MAX,
            },
        ];

        let blockers = blockers_from(&contacts, 2);
        assert_eq!(blockers.len(), 2);
        assert_eq!(blockers[0].at, Point::from_mm(1.0, 1.0));
        assert_eq!(blockers[0].layers, 1);
        assert!(blockers.iter().all(|b| b.radius_cells == 2));
    }
}
