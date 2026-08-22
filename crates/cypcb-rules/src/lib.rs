//! PCB Design Rules Crate
//!
//! Provides the foundational types for PCB design rules, layer stackup
//! definitions, routing rule interfaces, manufacturer presets, and IPC-2221
//! clearance tables.
//!
//! This crate is a leaf dependency — it depends only on `cypcb-core` and has
//! no dependency on `cypcb-world`, `cypcb-drc`, or any other workspace crate.
//!
//! # Modules
//!
//! - [`constraints`]: PCB fabrication constraints ([`DesignConstraints`])
//! - [`stackup`]: Layer stackup definitions ([`Stackup`], [`LayerStackEntry`])
//! - [`routing_rules`]: Routing rule trait for the autorouter ([`RoutingRuleSet`])
//! - [`presets`]: Manufacturer and IPC-tier presets ([`RulesPreset`], [`PresetRuleSet`])
//! - [`clearance_table`]: IPC-2221 voltage-based clearance lookup
//!
//! # Usage
//!
//! ```
//! use cypcb_rules::presets::{RulesPreset, PresetRuleSet};
//! use cypcb_rules::RoutingRuleSet;
//!
//! // Get JLCPCB 2-layer constraints
//! let preset = RulesPreset::from_name("jlcpcb").unwrap();
//! let constraints = preset.constraints();
//! let stackup = preset.stackup();
//!
//! // Use as a routing rule set
//! let ruleset = PresetRuleSet::new(preset);
//! let net_constraints = ruleset.constraints_for_net(0);
//! assert_eq!(net_constraints.min_trace_width, constraints.min_trace_width);
//! ```

pub mod clearance_table;
pub mod constraints;
pub mod presets;
pub mod routing_rules;
pub mod stackup;

// Re-export primary types at crate root.
pub use constraints::DesignConstraints;
pub use routing_rules::RoutingRuleSet;
pub use stackup::{LayerStackEntry, LayerType, Stackup};
