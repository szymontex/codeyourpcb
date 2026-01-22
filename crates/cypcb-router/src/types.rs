//! Routing result types.
//!
//! Types for representing autorouting results from FreeRouting.

use cypcb_core::{Nm, Point};
use cypcb_world::{Layer, NetId};

/// Status of the routing operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingStatus {
    /// All nets successfully routed.
    Complete,
    /// Some nets could not be routed.
    Partial {
        /// Number of connections that could not be routed.
        unrouted_count: usize,
    },
    /// Routing failed completely.
    Failed {
        /// Error message or reason for failure.
        reason: String,
    },
}

impl RoutingStatus {
    /// Check if routing completed successfully.
    pub fn is_complete(&self) -> bool {
        matches!(self, RoutingStatus::Complete)
    }

    /// Check if routing failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, RoutingStatus::Failed { .. })
    }
}

/// A single routed trace segment.
///
/// Represents a wire segment from the autorouter output.
/// Multiple segments form a complete trace path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSegment {
    /// The net this segment belongs to.
    pub net_id: NetId,
    /// Which copper layer this segment is on.
    pub layer: Layer,
    /// Trace width in nanometers.
    pub width: Nm,
    /// Start point of the segment.
    pub start: Point,
    /// End point of the segment.
    pub end: Point,
}

impl RouteSegment {
    /// Create a new route segment.
    pub fn new(net_id: NetId, layer: Layer, width: Nm, start: Point, end: Point) -> Self {
        RouteSegment {
            net_id,
            layer,
            width,
            start,
            end,
        }
    }

    /// Calculate the length of this segment in nanometers.
    pub fn length(&self) -> Nm {
        let dx = self.end.x.0 - self.start.x.0;
        let dy = self.end.y.0 - self.start.y.0;

        // Use i128 to prevent overflow during squared calculation
        let dx_squared = (dx as i128) * (dx as i128);
        let dy_squared = (dy as i128) * (dy as i128);

        let sum = dx_squared + dy_squared;
        let length = (sum as f64).sqrt() as i64;

        Nm(length)
    }
}

/// A via placement from the autorouter.
///
/// Vias connect traces between different copper layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaPlacement {
    /// The net this via belongs to.
    pub net_id: NetId,
    /// Center position of the via.
    pub position: Point,
    /// Drill hole diameter in nanometers.
    pub drill: Nm,
    /// Upper layer connection (typically TopCopper).
    pub start_layer: Layer,
    /// Lower layer connection (typically BottomCopper).
    pub end_layer: Layer,
}

impl ViaPlacement {
    /// Create a new via placement.
    pub fn new(
        net_id: NetId,
        position: Point,
        drill: Nm,
        start_layer: Layer,
        end_layer: Layer,
    ) -> Self {
        ViaPlacement {
            net_id,
            position,
            drill,
            start_layer,
            end_layer,
        }
    }

    /// Create a through-hole via (top to bottom copper).
    pub fn through_hole(net_id: NetId, position: Point, drill: Nm) -> Self {
        ViaPlacement {
            net_id,
            position,
            drill,
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
        }
    }
}

/// Result of an autorouting operation.
///
/// Contains the routing status and all generated routes and vias.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Status of the routing operation.
    pub status: RoutingStatus,
    /// Generated trace segments.
    pub routes: Vec<RouteSegment>,
    /// Generated via placements.
    pub vias: Vec<ViaPlacement>,
}

impl RoutingResult {
    /// Create a successful routing result.
    pub fn complete(routes: Vec<RouteSegment>, vias: Vec<ViaPlacement>) -> Self {
        RoutingResult {
            status: RoutingStatus::Complete,
            routes,
            vias,
        }
    }

    /// Create a partial routing result.
    pub fn partial(
        routes: Vec<RouteSegment>,
        vias: Vec<ViaPlacement>,
        unrouted_count: usize,
    ) -> Self {
        RoutingResult {
            status: RoutingStatus::Partial { unrouted_count },
            routes,
            vias,
        }
    }

    /// Create a failed routing result.
    pub fn failed(reason: impl Into<String>) -> Self {
        RoutingResult {
            status: RoutingStatus::Failed {
                reason: reason.into(),
            },
            routes: Vec::new(),
            vias: Vec::new(),
        }
    }

    /// Check if routing completed successfully.
    pub fn is_complete(&self) -> bool {
        self.status.is_complete()
    }

    /// Get the total number of route segments.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Get the total number of vias.
    pub fn via_count(&self) -> usize {
        self.vias.len()
    }

    /// Calculate the total routed wire length.
    pub fn total_length(&self) -> Nm {
        let total: i64 = self.routes.iter().map(|r| r.length().0).sum();
        Nm(total)
    }
}

impl Default for RoutingResult {
    fn default() -> Self {
        RoutingResult {
            status: RoutingStatus::Complete,
            routes: Vec::new(),
            vias: Vec::new(),
        }
    }
}

/// Quality metrics for routing results.
///
/// Used to evaluate the "satisfaction score" of a routing solution.
/// Lower values generally indicate better routing quality.
///
/// # Example
///
/// ```
/// use cypcb_router::types::RoutingMetrics;
/// use cypcb_core::Nm;
///
/// let metrics = RoutingMetrics {
///     total_length: Nm::from_mm(150.0),
///     via_count: 5,
///     layer_changes: 5,
///     unrouted_nets: 0,
/// };
///
/// assert!(metrics.is_complete());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutingMetrics {
    /// Total routed wire length in nanometers.
    pub total_length: Nm,

    /// Number of vias used.
    pub via_count: u32,

    /// Number of layer changes (equals via_count for simple designs).
    pub layer_changes: u32,

    /// Number of nets that could not be routed.
    pub unrouted_nets: u32,
}

impl RoutingMetrics {
    /// Check if all nets were successfully routed.
    pub fn is_complete(&self) -> bool {
        self.unrouted_nets == 0
    }

    /// Calculate a simple quality score (lower is better).
    ///
    /// Scoring formula:
    /// - Base: total_length in mm
    /// - Penalty: +5mm per via
    /// - Penalty: +1000mm per unrouted net
    ///
    /// This provides a rough measure for comparing routing solutions.
    pub fn quality_score(&self) -> f64 {
        let length_mm = self.total_length.0 as f64 / 1_000_000.0;
        let via_penalty = self.via_count as f64 * 5.0;
        let unrouted_penalty = self.unrouted_nets as f64 * 1000.0;

        length_mm + via_penalty + unrouted_penalty
    }
}

/// Calculate routing metrics from a RoutingResult.
///
/// # Example
///
/// ```
/// use cypcb_router::types::{RoutingResult, RouteSegment, ViaPlacement, calculate_metrics};
/// use cypcb_world::{Layer, NetId};
/// use cypcb_core::{Nm, Point};
///
/// let routes = vec![
///     RouteSegment::new(NetId::new(0), Layer::TopCopper, Nm::from_mm(0.2),
///         Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
/// ];
/// let vias = vec![
///     ViaPlacement::through_hole(NetId::new(0), Point::from_mm(10.0, 0.0), Nm::from_mm(0.3)),
/// ];
///
/// let result = RoutingResult::complete(routes, vias);
/// let metrics = calculate_metrics(&result);
///
/// assert_eq!(metrics.via_count, 1);
/// assert_eq!(metrics.total_length, Nm::from_mm(10.0));
/// ```
pub fn calculate_metrics(result: &RoutingResult) -> RoutingMetrics {
    let total_length = result.total_length();
    let via_count = result.vias.len() as u32;

    // Count layer changes (each via represents a layer change)
    let layer_changes = via_count;

    // Count unrouted nets from status
    let unrouted_nets = match &result.status {
        RoutingStatus::Complete => 0,
        RoutingStatus::Partial { unrouted_count } => *unrouted_count as u32,
        RoutingStatus::Failed { .. } => u32::MAX, // Unknown, assume worst
    };

    RoutingMetrics {
        total_length,
        via_count,
        layer_changes,
        unrouted_nets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_status() {
        assert!(RoutingStatus::Complete.is_complete());
        assert!(!RoutingStatus::Complete.is_failed());

        let partial = RoutingStatus::Partial { unrouted_count: 5 };
        assert!(!partial.is_complete());
        assert!(!partial.is_failed());

        let failed = RoutingStatus::Failed {
            reason: "timeout".into(),
        };
        assert!(!failed.is_complete());
        assert!(failed.is_failed());
    }

    #[test]
    fn test_route_segment() {
        let seg = RouteSegment::new(
            NetId::new(0),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 0.0),
        );

        assert_eq!(seg.net_id, NetId::new(0));
        assert_eq!(seg.layer, Layer::TopCopper);
        assert_eq!(seg.width, Nm::from_mm(0.2));
        assert_eq!(seg.length(), Nm::from_mm(10.0));
    }

    #[test]
    fn test_route_segment_diagonal() {
        // 3-4-5 triangle
        let seg = RouteSegment::new(
            NetId::new(0),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(3.0, 4.0),
        );

        assert_eq!(seg.length(), Nm::from_mm(5.0));
    }

    #[test]
    fn test_via_placement() {
        let via = ViaPlacement::through_hole(
            NetId::new(1),
            Point::from_mm(5.0, 5.0),
            Nm::from_mm(0.3),
        );

        assert_eq!(via.net_id, NetId::new(1));
        assert_eq!(via.position, Point::from_mm(5.0, 5.0));
        assert_eq!(via.drill, Nm::from_mm(0.3));
        assert_eq!(via.start_layer, Layer::TopCopper);
        assert_eq!(via.end_layer, Layer::BottomCopper);
    }

    #[test]
    fn test_routing_result_complete() {
        let routes = vec![RouteSegment::new(
            NetId::new(0),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 0.0),
        )];
        let vias = vec![ViaPlacement::through_hole(
            NetId::new(0),
            Point::from_mm(10.0, 0.0),
            Nm::from_mm(0.3),
        )];

        let result = RoutingResult::complete(routes, vias);

        assert!(result.is_complete());
        assert_eq!(result.route_count(), 1);
        assert_eq!(result.via_count(), 1);
        assert_eq!(result.total_length(), Nm::from_mm(10.0));
    }

    #[test]
    fn test_routing_result_partial() {
        let result = RoutingResult::partial(Vec::new(), Vec::new(), 3);

        assert!(!result.is_complete());
        assert_eq!(result.route_count(), 0);

        if let RoutingStatus::Partial { unrouted_count } = result.status {
            assert_eq!(unrouted_count, 3);
        } else {
            panic!("Expected Partial status");
        }
    }

    #[test]
    fn test_routing_result_failed() {
        let result = RoutingResult::failed("Process crashed");

        assert!(!result.is_complete());
        assert!(result.routes.is_empty());
        assert!(result.vias.is_empty());

        if let RoutingStatus::Failed { reason } = result.status {
            assert_eq!(reason, "Process crashed");
        } else {
            panic!("Expected Failed status");
        }
    }

    #[test]
    fn test_routing_result_default() {
        let result = RoutingResult::default();

        assert!(result.is_complete());
        assert!(result.routes.is_empty());
        assert!(result.vias.is_empty());
    }

    #[test]
    fn test_routing_metrics_complete() {
        let metrics = RoutingMetrics {
            total_length: Nm::from_mm(100.0),
            via_count: 3,
            layer_changes: 3,
            unrouted_nets: 0,
        };

        assert!(metrics.is_complete());
    }

    #[test]
    fn test_routing_metrics_incomplete() {
        let metrics = RoutingMetrics {
            total_length: Nm::from_mm(50.0),
            via_count: 1,
            layer_changes: 1,
            unrouted_nets: 2,
        };

        assert!(!metrics.is_complete());
    }

    #[test]
    fn test_routing_metrics_quality_score() {
        // Simple case: 100mm length, 0 vias, 0 unrouted
        let metrics = RoutingMetrics {
            total_length: Nm::from_mm(100.0),
            via_count: 0,
            layer_changes: 0,
            unrouted_nets: 0,
        };

        // Score should be just the length
        assert!((metrics.quality_score() - 100.0).abs() < 0.01);

        // With vias
        let metrics_with_vias = RoutingMetrics {
            total_length: Nm::from_mm(100.0),
            via_count: 2,
            layer_changes: 2,
            unrouted_nets: 0,
        };

        // Score = 100 + (2 * 5) = 110
        assert!((metrics_with_vias.quality_score() - 110.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_metrics_complete() {
        let routes = vec![
            RouteSegment::new(
                NetId::new(0),
                Layer::TopCopper,
                Nm::from_mm(0.2),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
            RouteSegment::new(
                NetId::new(0),
                Layer::BottomCopper,
                Nm::from_mm(0.2),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(20.0, 0.0),
            ),
        ];
        let vias = vec![ViaPlacement::through_hole(
            NetId::new(0),
            Point::from_mm(10.0, 0.0),
            Nm::from_mm(0.3),
        )];

        let result = RoutingResult::complete(routes, vias);
        let metrics = calculate_metrics(&result);

        assert_eq!(metrics.total_length, Nm::from_mm(20.0));
        assert_eq!(metrics.via_count, 1);
        assert_eq!(metrics.layer_changes, 1);
        assert_eq!(metrics.unrouted_nets, 0);
        assert!(metrics.is_complete());
    }

    #[test]
    fn test_calculate_metrics_partial() {
        let routes = vec![RouteSegment::new(
            NetId::new(0),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(5.0, 0.0),
        )];

        let result = RoutingResult::partial(routes, Vec::new(), 3);
        let metrics = calculate_metrics(&result);

        assert_eq!(metrics.total_length, Nm::from_mm(5.0));
        assert_eq!(metrics.via_count, 0);
        assert_eq!(metrics.unrouted_nets, 3);
        assert!(!metrics.is_complete());
    }

    #[test]
    fn test_routing_metrics_default() {
        let metrics = RoutingMetrics::default();

        assert_eq!(metrics.total_length, Nm(0));
        assert_eq!(metrics.via_count, 0);
        assert_eq!(metrics.layer_changes, 0);
        assert_eq!(metrics.unrouted_nets, 0);
        assert!(metrics.is_complete());
    }
}
