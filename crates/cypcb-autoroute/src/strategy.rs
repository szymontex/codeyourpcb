//! Multi-strategy routing abstraction.
//!
//! Defines the [`RoutingStrategy`] trait that all routing algorithms implement,
//! and [`StrategyKind`] for selecting among them at configuration time.
//!
//! # Adding a New Strategy
//!
//! 1. Implement [`RoutingStrategy`] for your struct
//! 2. Add a variant to [`StrategyKind`]
//! 3. Add a dispatch arm in `route_board()` in `lib.rs`

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::AutorouteConfig;

/// A routing algorithm that can route all unrouted nets on a board.
///
/// Strategies receive the full board context and return a [`RoutingResult`]
/// with generated segments and vias. Each strategy owns its routing loop —
/// the orchestration (net ordering, rip-up policy, cost function) lives
/// inside the strategy, not in the caller.
pub trait RoutingStrategy {
    /// A stable, human-readable name for this strategy (e.g. `"improved-astar"`).
    ///
    /// Used in tracing output and test assertions.
    fn name(&self) -> &str;

    /// Route all unrouted nets on the board.
    ///
    /// # Arguments
    ///
    /// * `world` - Board world with components, pads, zones, and existing traces
    /// * `library` - Footprint library for pad geometry lookup
    /// * `rules` - Design rules providing clearances, widths, and via costs
    /// * `config` - Autorouter configuration (grid resolution, iteration limits, etc.)
    fn route(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
    ) -> RoutingResult;
}

/// Which routing strategy to use.
///
/// Selects the algorithm used by [`route_board()`](crate::route_board).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrategyKind {
    /// PathFinder negotiated congestion router (VPR-style iterative rip-up).
    #[default]
    PathFinder,

    /// Improved A* with congestion-aware cost, better net ordering,
    /// and multi-victim rip-up (3 victims per failed connection).
    ImprovedAStar,
}

impl std::fmt::Display for StrategyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrategyKind::PathFinder => write!(f, "pathfinder"),
            StrategyKind::ImprovedAStar => write!(f, "improved-astar"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_kind_default_is_pathfinder() {
        assert_eq!(StrategyKind::default(), StrategyKind::PathFinder);
    }

    #[test]
    fn strategy_kind_display() {
        assert_eq!(format!("{}", StrategyKind::PathFinder), "pathfinder");
        assert_eq!(format!("{}", StrategyKind::ImprovedAStar), "improved-astar");
    }

    #[test]
    fn strategy_kind_equality() {
        assert_ne!(StrategyKind::PathFinder, StrategyKind::ImprovedAStar);
    }
}
