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

/// Result of running DRC on a board.
#[derive(Debug, Clone)]
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

impl Default for DrcResult {
    fn default() -> Self {
        DrcResult {
            violations: Vec::new(),
            duration_ms: 0,
        }
    }
}

/// Run DRC on a board world.
///
/// Executes all enabled rules against the board and returns accumulated violations.
///
/// # Arguments
///
/// * `world` - The board world to check
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
/// let world = BoardWorld::new();
/// let rules = DesignRules::default();
/// let result = run_drc(&world, &rules);
/// println!("DRC completed in {}ms", result.duration_ms);
/// ```
pub fn run_drc(world: &BoardWorld, rules: &DesignRules) -> DrcResult {
    use std::time::Instant;

    let start = Instant::now();
    let mut violations = Vec::new();

    // Create all rule checkers
    let checkers: Vec<Box<dyn DrcRule>> = vec![
        Box::new(rules::ClearanceRule),
        Box::new(rules::MinDrillSizeRule),
        Box::new(rules::UnconnectedPinRule),
    ];

    // Run each checker
    for checker in &checkers {
        violations.extend(checker.check(world, rules));
    }

    DrcResult {
        violations,
        duration_ms: start.elapsed().as_millis() as u64,
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
        use cypcb_core::Point;
        use bevy_ecs::entity::Entity;

        let result = DrcResult {
            violations: vec![
                DrcViolation::unconnected_pin(
                    Entity::from_raw(1),
                    "1",
                    "R1",
                    Point::ORIGIN,
                ),
            ],
            duration_ms: 5,
        };

        assert!(!result.passed());
        assert_eq!(result.violation_count(), 1);
    }

    #[test]
    fn test_run_drc_empty_world() {
        let world = BoardWorld::new();
        let rules = DesignRules::default();
        let result = run_drc(&world, &rules);

        // Empty world should have no violations
        assert!(result.passed());
    }
}
