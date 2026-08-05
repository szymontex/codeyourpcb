//! CodeYourPCB Design Rule Check (DRC) Engine
//!
//! Validates PCB designs against manufacturer constraints before fabrication.
//! Uses the spatial index for efficient clearance checking with O(log n) queries.
//!
//! # Architecture
//!
//! DRC rules are implemented as structs that implement the [`DrcRule`] trait.
//! The engine runs all enabled rules against the board and collects violations.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cypcb_drc::{run_drc, DesignRules, DrcResult, Preset};
//! use cypcb_world::BoardWorld;
//!
//! let world = BoardWorld::new();
//! // ... load board ...
//!
//! // Use a manufacturer preset
//! let rules = DesignRules::jlcpcb_2layer();
//! let result = run_drc(&world, &rules);
//!
//! // Or lookup by name (from DSL parsing)
//! let preset = Preset::from_name("pcbway").unwrap();
//! let rules = preset.rules();
//!
//! if result.passed() {
//!     println!("Board passes DRC!");
//! } else {
//!     println!("{} violations found", result.violation_count());
//!     for violation in &result.violations {
//!         println!("  {}: {}", violation.kind, violation.message);
//!     }
//! }
//! ```

pub mod presets;
pub mod rules;
pub mod violation;

pub use presets::{DesignRules, Preset};
pub use rules::DrcRule;
pub use violation::{DrcViolation, ViolationKind};

use cypcb_world::BoardWorld;

use hashbrown::HashMap;

/// Result of running DRC on a board.
#[derive(Debug, Clone, Default)]
pub struct DrcResult {
    /// List of violations found.
    pub violations: Vec<DrcViolation>,
    /// Time taken to run DRC in milliseconds (for performance tracking).
    pub duration_ms: u64,
}

impl DrcResult {
    /// Check if the board passed all checks.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DrcResult;
    ///
    /// let result = DrcResult { violations: vec![], duration_ms: 10 };
    /// assert!(result.passed());
    /// ```
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    /// Number of violations found.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DrcResult;
    ///
    /// let result = DrcResult { violations: vec![], duration_ms: 10 };
    /// assert_eq!(result.violation_count(), 0);
    /// ```
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

/// Run DRC on a board world.
///
/// Executes all enabled rules against the board and returns accumulated violations.
///
/// # Arguments
///
/// * `world` - The board world to check (mutable for ECS queries)
/// * `rules` - Design rules to check against
///
/// # Returns
///
/// DrcResult with all violations found.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::{run_drc, DesignRules};
/// use cypcb_world::BoardWorld;
///
/// let mut world = BoardWorld::new();
/// let rules = DesignRules::default();
/// let result = run_drc(&mut world, &rules);
/// println!("DRC completed in {}ms", result.duration_ms);
/// ```
pub fn run_drc(world: &mut BoardWorld, rules: &DesignRules) -> DrcResult {
    // Timing - skip in WASM (Instant may not work reliably)
    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();

    let mut violations = Vec::new();

    // Create all rule checkers
    let checkers: Vec<Box<dyn DrcRule>> = vec![
        Box::new(rules::ClearanceRule),
        Box::new(rules::MinDrillSizeRule),
        Box::new(rules::MinTraceWidthRule),
        Box::new(rules::UnconnectedPinRule),
        Box::new(rules::KeepoutRule),
        Box::new(rules::EdgeClearanceRule),
        Box::new(rules::AnnularRingRule),
        Box::new(rules::HoleToHoleRule),
        Box::new(rules::ViaDiameterRule),
        Box::new(rules::CourtyardClearanceRule),
        Box::new(rules::SolderMaskBridgeRule),
        Box::new(rules::SilkClearanceRule),
    ];

    // Run each checker
    for checker in &checkers {
        violations.extend(checker.check(world, rules));
    }

    // Enrich violation messages with entity names (refdes, net, pad info).
    // Build lookups from ECS once, then annotate all violations.
    enrich_violation_messages(&mut violations, world);

    #[cfg(not(target_arch = "wasm32"))]
    let duration_ms = start.elapsed().as_millis() as u64;
    #[cfg(target_arch = "wasm32")]
    let duration_ms = 0; // Skip timing in WASM

    DrcResult {
        violations,
        duration_ms,
    }
}

/// Enrich violation messages with human-readable entity identifiers.
///
/// Builds lookups from ECS (entity → refdes, entity → net name, entity → pad parent)
/// and prepends entity identifiers to each violation message so users can see
/// which components/traces are involved.
fn enrich_violation_messages(violations: &mut [DrcViolation], world: &mut BoardWorld) {
    use cypcb_world::components::trace::{Trace, Via};
    use cypcb_world::components::{NetId, PadInstance, RefDes};

    // Entity index → refdes string
    let refdes_map: HashMap<u32, String> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &RefDes)>();
        query
            .iter(ecs)
            .map(|(e, r)| (e.index(), r.as_str().to_string()))
            .collect()
    };

    // Entity index → net name (for traces/vias with NetId)
    let net_name_map: HashMap<u32, String> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetId)>();
        let pairs: Vec<_> = query.iter(ecs).map(|(e, n)| (e.index(), *n)).collect();

        pairs
            .into_iter()
            .filter_map(|(idx, net_id)| world.net_name(net_id).map(|name| (idx, name.to_string())))
            .collect()
    };

    // Entity index → parent refdes (for PadInstance entities)
    let pad_parent_map: HashMap<u32, String> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &PadInstance)>();
        let pads: Vec<_> = query
            .iter(ecs)
            .map(|(e, pi)| (e.index(), pi.parent.index()))
            .collect();
        pads.into_iter()
            .filter_map(|(idx, parent_idx)| refdes_map.get(&parent_idx).map(|r| (idx, r.clone())))
            .collect()
    };

    // Entity index → "trace on <net>" or "via on <net>"
    let trace_label_map: HashMap<u32, String> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
        query
            .iter(ecs)
            .map(|(e, _)| {
                let net = net_name_map
                    .get(&e.index())
                    .map(|n| n.as_str())
                    .unwrap_or("?");
                (e.index(), format!("trace '{}'", net))
            })
            .collect()
    };

    let via_label_map: HashMap<u32, String> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
        query
            .iter(ecs)
            .map(|(e, _)| {
                let net = net_name_map
                    .get(&e.index())
                    .map(|n| n.as_str())
                    .unwrap_or("?");
                (e.index(), format!("via '{}'", net))
            })
            .collect()
    };

    // Helper: get human label for an entity
    let label = |idx: u32| -> String {
        if let Some(r) = refdes_map.get(&idx) {
            return r.clone();
        }
        if let Some(r) = pad_parent_map.get(&idx) {
            return format!("pad on {}", r);
        }
        if let Some(l) = trace_label_map.get(&idx) {
            return l.clone();
        }
        if let Some(l) = via_label_map.get(&idx) {
            return l.clone();
        }
        format!("entity #{}", idx)
    };

    for v in violations.iter_mut() {
        let primary = label(v.entity.index());
        let secondary = v.other_entity.map(|e| label(e.index()));

        // Prepend entity info to the message
        let between = match secondary {
            Some(ref other) => format!("{} ↔ {}: ", primary, other),
            None => format!("{}: ", primary),
        };

        // Only prepend if message doesn't already contain the refdes
        if !v.message.contains(&primary) {
            v.message = format!("{}{}", between, v.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drc_result_passed_empty() {
        let result = DrcResult::default();
        assert!(result.passed());
        assert_eq!(result.violation_count(), 0);
    }

    #[test]
    fn test_drc_result_with_violations() {
        use bevy_ecs::entity::Entity;
        use cypcb_core::Point;

        let result = DrcResult {
            violations: vec![DrcViolation::unconnected_pin(
                Entity::from_raw(1),
                "1",
                "R1",
                Point::ORIGIN,
            )],
            duration_ms: 5,
        };

        assert!(!result.passed());
        assert_eq!(result.violation_count(), 1);
    }

    #[test]
    fn test_run_drc_empty_world() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        let result = run_drc(&mut world, &rules);

        // Empty world should have no violations
        assert!(result.passed());
    }
}
