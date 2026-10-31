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
//! - A candidate is kept only when the variant ranking puts it ahead of the
//!   best so far *and* it is no worse in either of two ways the ranking adds
//!   together: it leaves no more connections open, and no net touches another
//!   net's copper in more places than it did (see [`Footing::admits`]). Repair
//!   can run on a board with connections open, and the ranking alone would
//!   buy back an open connection with a short. The worst it can cost is time.
//! - How far around a violation to forbid is not a tuned constant. Each radius
//!   in [`AutorouteConfig::repair_block_radii`] is tried as its own attempt and
//!   the measured winner is kept, because radius 0 helps one benchmark board
//!   and radius 2 the other.

use std::collections::{HashMap, HashSet};

use bevy_ecs::entity::Entity;

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_router::apply_routes;
use cypcb_router::types::{RoutingResult, RoutingStatus};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::components::trace::Trace;
use cypcb_world::components::{NetConnections, NetId, RefDes};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::grid::layer_to_index;
use crate::pathfinder_v2::PathFinderStrategy;
use crate::scoring::{score_board, ScoreWeights};
use crate::variant::RankKey;
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

/// Where a measured board stands: its rank, and what the rank adds together.
///
/// The ranking counts an open connection and a short alike (`incomplete`), so
/// it ranks a board that traded the first for the second ahead of the board it
/// started from. Repair keeps the two apart.
#[derive(Debug, Clone)]
struct Footing {
    key: RankKey,
    /// Connections still open: the router's own count, pins no copper reaches
    /// and nets cut in two.
    open: usize,
    /// Nets with a pin no copper reaches.
    open_nets: HashSet<u32>,
    /// For each net, the places where its copper touches another net's. A
    /// place between two nets counts for both; copper of no net counts for none.
    touching: HashMap<u32, u32>,
}

impl Footing {
    fn of(key: RankKey, open_nets: HashSet<u32>, touching: HashMap<u32, u32>) -> Self {
        Footing {
            key,
            open: key.incomplete - key.shorts as usize,
            open_nets,
            touching,
        }
    }

    /// Whether a board with this footing may replace the one standing on `self`.
    ///
    /// It has to rank ahead, leave no more connections open, touch no more
    /// places in all, and leave no net that has a pin open touching another in
    /// more places than before. The last clause is what refuses a candidate
    /// that closes a connection of one net by laying it across another, even
    /// when it clears as many places elsewhere.
    fn admits(&self, candidate: &Footing) -> bool {
        candidate.key.cmp_rank(&self.key) == std::cmp::Ordering::Less
            && candidate.open <= self.open
            && candidate.key.shorts <= self.key.shorts
            && self.open_nets.iter().all(|net| {
                candidate.touching.get(net).copied().unwrap_or(0)
                    <= self.touching.get(net).copied().unwrap_or(0)
            })
    }
}

/// Repair a routed board against its own DRC report.
///
/// `initial` is the result of a first routing pass. Returns either that result
/// unchanged or a better one: ahead in the ranking, with no more connections
/// open and no net touching another in more places. A board with connections
/// open is repaired like any other, and comes back with the status of the copper
/// it holds, `Partial` while a connection is open.
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
    if config.repair_passes == 0
        || config.repair_block_radii.is_empty()
        || unrouted_of(&initial).is_none()
    {
        return initial;
    }

    let design_rules = DesignRules::from_constraints(rules.constraints_for_net(0));
    let resolution = PathFinderStrategy::resolution_for(world, rules, config);

    let mut best = initial;
    let (mut best_footing, baseline_violations) = measure(world, library, &best, &design_rules);
    let before = best_footing.key;
    let mut best_count = baseline_violations.len();
    if best_count == 0 {
        return best;
    }

    // Routing again with the strip of nets that share copper on is what keeps
    // a repaired board from sharing it again. Where connections are open the
    // strip is what opened them, and an attempt without it may close some
    // without a short; `Footing::admits` refuses the ones that do not.
    let mut resolve_modes = vec![config.resolve_contested];
    if config.resolve_contested && best_footing.open > 0 {
        resolve_modes.push(false);
    }

    let mut accepted = 0u32;
    let mut winning_radius = None;

    for (resolve_contested, &radius) in resolve_modes.iter().flat_map(|&mode| {
        config
            .repair_block_radii
            .iter()
            .map(move |radius| (mode, radius))
    }) {
        // Every radius starts from the same board, so the attempts are
        // independent and comparable.
        let mut blockers = dedup(blockers_from(&baseline_violations, radius), resolution);

        for pass in 1..=config.repair_passes {
            // The board still carries the copper `measure` laid, and the router
            // leaves alone a net that copper already joins - so without this an
            // attempt routes nothing, and an empty board has no contacts.
            apply_routes(world, &RoutingResult::complete(Vec::new(), Vec::new()));
            world.rebuild_spatial_index_from_library(library);
            let attempt = AutorouteConfig {
                resolve_contested,
                ..config.clone()
            };
            let candidate =
                PathFinderStrategy.route_with_blockers(world, library, rules, &attempt, &blockers);

            if unrouted_of(&candidate).is_none_or(|n| n > unrouted_of(&best).unwrap_or(0)) {
                tracing::info!(
                    radius,
                    pass,
                    blocked_cells = blockers.len(),
                    "Repair attempt rejected: the board routes fewer connections"
                );
                break;
            }

            let (footing, violations) = measure(world, library, &candidate, &design_rules);
            let key = footing.key;
            let count = violations.len();
            tracing::info!(
                radius,
                pass,
                violations = count,
                key = ?key,
                best = ?best_footing.key,
                resolve_contested,
                blocked_cells = blockers.len(),
                // How much copper the attempt actually carries. A pass that
                // wins on violations while carrying a fraction of the routes
                // has not repaired the board, it has emptied it.
                routes = candidate.route_count(),
                vias = candidate.via_count(),
                "Repair attempt measured"
            );

            // Kept only when the ranking would put it ahead of the best so
            // far and it opens no connection and touches no net more than
            // the best does. Judged by the pass's own count of contacts,
            // attempts that gave up connections or traded tight gaps for
            // shorts were kept.
            if best_footing.admits(&footing) {
                best_footing = footing;
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
        before = ?before,
        after = ?best_footing.key,
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
/// Returns where the board stands in the variant ranking, and each violation
/// this pass could act on: where it was and which copper layers it touched.
/// The layers are resolved here because a later attempt despawns these
/// entities.
///
/// The contacts deliberately exclude everything rerouting cannot move - a part
/// placed off the board edge, silkscreen over a pad, a pin nobody connected -
/// because they are what the next attempt blocks. Whether an attempt is kept
/// is the ranking's question, not theirs.
fn measure(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    result: &RoutingResult,
    design_rules: &DesignRules,
) -> (Footing, Vec<Contact>) {
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

    let mut touching: HashMap<u32, u32> = HashMap::new();
    for violation in &report.violations {
        if violation.kind != ViolationKind::Clearance || violation.actual != Some(Nm::ZERO) {
            continue;
        }
        let nets: HashSet<u32> = [Some(violation.entity), violation.other_entity]
            .into_iter()
            .flatten()
            .filter_map(|entity| world.ecs().get::<NetId>(entity))
            .map(|net| net.id())
            .collect();
        for net in nets {
            *touching.entry(net).or_insert(0) += 1;
        }
    }
    let open_nets: HashSet<u32> = report
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::UnroutedPin)
        .filter_map(|violation| net_of_pin(world, violation))
        .collect();
    let score = score_board(world, design_rules, &ScoreWeights::default());
    let key = RankKey::of(unrouted_of(result).unwrap_or(0), &score);
    (Footing::of(key, open_nets, touching), contacts)
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

/// The net of the pin an `unrouted-pin` violation names.
///
/// The violation carries the part and says `REF.PIN is on a net ...`; the pin
/// is read from that text because it is the only place the pin is stated.
fn net_of_pin(world: &BoardWorld, violation: &cypcb_drc::DrcViolation) -> Option<u32> {
    let refdes = world.ecs().get::<RefDes>(violation.entity)?;
    let pin = violation
        .message
        .strip_prefix(refdes.as_str())?
        .strip_prefix('.')?
        .split_whitespace()
        .next()?;
    let nets = world.ecs().get::<NetConnections>(violation.entity)?;
    nets.pin_net(pin).map(|net| net.id())
}

/// Connections the router left open; `None` for a run that failed outright.
fn unrouted_of(result: &RoutingResult) -> Option<usize> {
    match result.status {
        RoutingStatus::Complete => Some(0),
        RoutingStatus::Partial { unrouted_count } => Some(unrouted_count),
        RoutingStatus::Failed { .. } => None,
    }
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

    /// A board with `open` connections open, on the nets `open_nets`, and the
    /// given places where one net touches another, `(net, net)` for each.
    fn footing_of_nets(open: usize, open_nets: &[u32], places: &[(u32, u32)]) -> Footing {
        let mut touching: HashMap<u32, u32> = HashMap::new();
        for &(a, b) in places {
            *touching.entry(a).or_insert(0) += 1;
            *touching.entry(b).or_insert(0) += 1;
        }
        let shorts = places.len() as u32;
        let key = RankKey {
            incomplete: open + places.len(),
            shorts,
            composite: 1.0,
        };
        Footing::of(key, open_nets.iter().copied().collect(), touching)
    }

    fn footing(open: usize, places: &[(u32, u32)]) -> Footing {
        footing_of_nets(open, &[3], places)
    }

    #[test]
    fn a_board_that_clears_a_short_and_opens_nothing_is_kept() {
        let best = footing(1, &[(1, 2)]);
        let candidate = footing(1, &[]);

        assert!(best.admits(&candidate));
    }

    /// Net 3 is open and nets 1 and 2 touch. The attempt clears that and closes
    /// net 3 by laying it across net 4: one open connection and one short
    /// traded for one short, which the totals read as a draw on shorts and a
    /// gain on connections. The place net 3 now touches is the trade.
    #[test]
    fn an_open_connection_closed_across_another_net_is_refused_when_a_short_elsewhere_is_cleared() {
        let best = footing(1, &[(1, 2)]);
        let candidate = footing(0, &[(3, 4)]);

        assert_eq!(candidate.key.shorts, best.key.shorts);
        assert_eq!(
            candidate.key.cmp_rank(&best.key),
            std::cmp::Ordering::Less,
            "the ranking alone would keep it"
        );
        assert!(!best.admits(&candidate));
    }

    /// One connection open and two shorts against two open and none: the ranking
    /// counts three against two and keeps the second, which gave up a connection
    /// to clear two shorts.
    #[test]
    fn an_attempt_that_leaves_more_connections_open_is_refused() {
        let best = footing(1, &[(1, 2), (3, 4)]);
        let candidate = footing(2, &[]);

        assert_eq!(
            candidate.key.cmp_rank(&best.key),
            std::cmp::Ordering::Less,
            "the ranking alone would keep it"
        );
        assert!(!best.admits(&candidate));
    }

    /// Two connections open and one short against none open and two shorts: the
    /// ranking counts three against two. The attempt closes both connections
    /// cleanly, and nets with a pin open touch nothing more, but the board has
    /// more places where copper touches copper than it had.
    #[test]
    fn an_attempt_that_touches_more_places_in_all_is_refused() {
        let best = footing_of_nets(2, &[3], &[(1, 2)]);
        let candidate = footing_of_nets(0, &[], &[(5, 6), (7, 8)]);

        assert_eq!(
            candidate.key.cmp_rank(&best.key),
            std::cmp::Ordering::Less,
            "the ranking alone would keep it"
        );
        assert!(candidate.key.shorts > best.key.shorts);
        assert!(!best.admits(&candidate));
    }

    #[test]
    fn an_attempt_that_does_not_rank_ahead_is_refused() {
        let best = footing(0, &[(1, 2)]);

        assert!(!best.admits(&footing(0, &[(1, 2)])));
    }

    #[test]
    fn copper_of_no_net_is_not_counted_against_a_net() {
        // A place between a pad and a pour: neither entity carries a net.
        let best = footing(0, &[(1, 2)]);
        let mut candidate = footing(0, &[]);
        candidate.key.incomplete = 0;

        assert!(best.admits(&candidate));
    }
}
