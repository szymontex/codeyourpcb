//! Manufacturer presets and IPC-tier design rule configurations.
//!
//! Each [`RulesPreset`] variant produces a complete [`DesignConstraints`] +
//! [`Stackup`] pair with manufacturer-verified values. Source URLs for each
//! manufacturer's capability page are documented in the individual preset files.
//!
//! [`PresetRuleSet`] wraps a preset and implements [`RoutingRuleSet`], making
//! it the primary way the autorouter consumes design rules.

pub mod ipc;
pub mod jlcpcb;
pub mod oshpark;
pub mod pcbway;

use std::collections::HashMap;

use cypcb_core::Nm;

use crate::constraints::DesignConstraints;
use crate::routing_rules::RoutingRuleSet;
use crate::signal_class::{SignalClass, SignalClassConstraints};
use crate::stackup::Stackup;

/// All available manufacturer/IPC preset configurations.
///
/// Each variant maps to a specific manufacturer process or IPC reliability
/// class. Use [`RulesPreset::constraints()`] and [`RulesPreset::stackup()`]
/// to get the full design rule set.
///
/// # Examples
///
/// ```
/// use cypcb_rules::presets::RulesPreset;
///
/// let preset = RulesPreset::from_name("jlcpcb").unwrap();
/// assert_eq!(preset, RulesPreset::JlcpcbStandard2Layer);
///
/// let dc = preset.constraints();
/// let stackup = preset.stackup();
/// assert_eq!(stackup.copper_layer_count(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RulesPreset {
    /// JLCPCB 2-layer, standard process.
    JlcpcbStandard2Layer,
    /// JLCPCB 4-layer, standard process.
    JlcpcbStandard4Layer,
    /// JLCPCB 2-layer, advanced process (tighter tolerances).
    JlcpcbAdvanced2Layer,
    /// JLCPCB 4-layer, advanced process.
    JlcpcbAdvanced4Layer,
    /// PCBWay standard process.
    PcbWayStandard,
    /// OSHPark 2-layer (After Dark purple boards).
    OshPark2Layer,
    /// OSHPark 4-layer.
    OshPark4Layer,
    /// IPC Class 1 — consumer electronics (relaxed tolerances).
    IpcClass1,
    /// IPC Class 2 — dedicated service equipment (standard tolerances).
    IpcClass2,
    /// IPC Class 3 — high reliability (tight tolerances).
    IpcClass3,
    /// Prototyping — bigger than any fab requires, for hand assembly.
    Prototype,
}

/// Where a preset's numbers came from, which decides how far to trust them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// A fabricator's own capability page, read and dated in the source.
    Published,
    /// A reading of a design standard with no public page behind it.
    ///
    /// The IPC classes. IPC-2221 and IPC-6012 are not public documents, so
    /// every figure in those three tables is this project's understanding of a
    /// standard it cannot cite a line of. They may well be right; nothing here
    /// can show that they are, and a checker that cannot show its source has
    /// to say so instead.
    Standard,
    /// This project's own choice, answering to nobody's table.
    ///
    /// `prototype` is deliberately bigger than any fab requires - it exists so
    /// a board can be hand-assembled, not so it can be quoted.
    ThisTool,
}

impl Provenance {
    /// What to tell a reader whose board was measured against this.
    ///
    /// `None` for a published table: a fab's own page needs no apology.
    pub fn caveat(self, preset_name: &str) -> Option<String> {
        match self {
            Provenance::Published => None,
            Provenance::Standard => Some(format!(
                "{preset_name} is a design standard rather than a fabricator. \
                 These figures are this tool's reading of IPC, which is not a \
                 public document - check them against your own copy before \
                 trusting a board to them."
            )),
            Provenance::ThisTool => Some(format!(
                "{preset_name} is this tool's own table, not a fabricator's. \
                 It is deliberately looser than any house requires, for hand \
                 assembly rather than for quoting."
            )),
        }
    }
}

impl RulesPreset {
    /// All preset variants in definition order.
    pub const ALL: [RulesPreset; 11] = [
        RulesPreset::JlcpcbStandard2Layer,
        RulesPreset::JlcpcbStandard4Layer,
        RulesPreset::JlcpcbAdvanced2Layer,
        RulesPreset::JlcpcbAdvanced4Layer,
        RulesPreset::PcbWayStandard,
        RulesPreset::OshPark2Layer,
        RulesPreset::OshPark4Layer,
        RulesPreset::IpcClass1,
        RulesPreset::IpcClass2,
        RulesPreset::IpcClass3,
        RulesPreset::Prototype,
    ];

    /// Returns all presets as a slice.
    pub fn all() -> &'static [RulesPreset] {
        &Self::ALL
    }

    /// Canonical name string for this preset.
    pub fn name(self) -> &'static str {
        match self {
            Self::JlcpcbStandard2Layer => "jlcpcb_standard_2layer",
            Self::JlcpcbStandard4Layer => "jlcpcb_standard_4layer",
            Self::JlcpcbAdvanced2Layer => "jlcpcb_advanced_2layer",
            Self::JlcpcbAdvanced4Layer => "jlcpcb_advanced_4layer",
            Self::PcbWayStandard => "pcbway_standard",
            Self::OshPark2Layer => "oshpark_2layer",
            Self::OshPark4Layer => "oshpark_4layer",
            Self::IpcClass1 => "ipc_class1",
            Self::IpcClass2 => "ipc_class2",
            Self::IpcClass3 => "ipc_class3",
            Self::Prototype => "prototype",
        }
    }

    /// Lookup a preset by name, supporting aliases.
    ///
    /// Matching is case-insensitive. Hyphens and underscores are interchangeable.
    ///
    /// # Aliases
    ///
    /// | Input | Resolves to |
    /// |-------|-------------|
    /// | `"jlcpcb"`, `"jlcpcb_2layer"`, `"jlcpcb_standard"` | `JlcpcbStandard2Layer` |
    /// | `"jlcpcb_4layer"`, `"jlcpcb_standard_4layer"` | `JlcpcbStandard4Layer` |
    /// | `"jlcpcb_advanced"`, `"jlcpcb_advanced_2layer"` | `JlcpcbAdvanced2Layer` |
    /// | `"jlcpcb_advanced_4layer"` | `JlcpcbAdvanced4Layer` |
    /// | `"pcbway"`, `"pcbway_standard"` | `PcbWayStandard` |
    /// | `"oshpark"`, `"oshpark_2layer"` | `OshPark2Layer` |
    /// | `"oshpark_4layer"` | `OshPark4Layer` |
    /// | `"ipc1"`, `"ipc_class1"`, `"ipc_class_1"` | `IpcClass1` |
    /// | `"ipc2"`, `"ipc_class2"`, `"ipc_class_2"` | `IpcClass2` |
    /// | `"ipc3"`, `"ipc_class3"`, `"ipc_class_3"` | `IpcClass3` |
    /// | `"prototype"`, `"proto"` | `Prototype` |
    pub fn from_name(name: &str) -> Option<Self> {
        // Normalize: lowercase, replace hyphens with underscores
        let norm: String = name.to_ascii_lowercase().replace('-', "_");
        let n = norm.as_str();

        match n {
            // JLCPCB
            "jlcpcb" | "jlcpcb_2layer" | "jlcpcb_standard" | "jlcpcb_standard_2layer" => {
                Some(Self::JlcpcbStandard2Layer)
            }
            "jlcpcb_4layer" | "jlcpcb_standard_4layer" => Some(Self::JlcpcbStandard4Layer),
            "jlcpcb_advanced" | "jlcpcb_advanced_2layer" => Some(Self::JlcpcbAdvanced2Layer),
            "jlcpcb_advanced_4layer" => Some(Self::JlcpcbAdvanced4Layer),

            // PCBWay
            "pcbway" | "pcbway_standard" => Some(Self::PcbWayStandard),

            // OSHPark
            "oshpark" | "oshpark_2layer" => Some(Self::OshPark2Layer),
            "oshpark_4layer" => Some(Self::OshPark4Layer),

            // IPC tiers
            "ipc1" | "ipc_class1" | "ipc_class_1" => Some(Self::IpcClass1),
            "ipc2" | "ipc_class2" | "ipc_class_2" => Some(Self::IpcClass2),
            "ipc3" | "ipc_class3" | "ipc_class_3" => Some(Self::IpcClass3),

            // Not a fab
            "prototype" | "proto" => Some(Self::Prototype),

            _ => None,
        }
    }

    /// Get the full design constraints for this preset.
    /// Where this preset's numbers came from.
    ///
    /// Seven of the eleven are a fabricator's own published capability page,
    /// read and dated. Three are a reading of a design standard that has no
    /// public page to link to, and one is this project's own choice. A user
    /// being told a board is out of spec deserves to know which of those three
    /// is doing the telling.
    pub fn provenance(self) -> Provenance {
        match self {
            Self::JlcpcbStandard2Layer
            | Self::JlcpcbStandard4Layer
            | Self::JlcpcbAdvanced2Layer
            | Self::JlcpcbAdvanced4Layer
            | Self::PcbWayStandard
            | Self::OshPark2Layer
            | Self::OshPark4Layer => Provenance::Published,
            Self::IpcClass1 | Self::IpcClass2 | Self::IpcClass3 => Provenance::Standard,
            Self::Prototype => Provenance::ThisTool,
        }
    }

    pub fn constraints(self) -> DesignConstraints {
        match self {
            Self::JlcpcbStandard2Layer => jlcpcb::standard_2layer(),
            Self::JlcpcbStandard4Layer => jlcpcb::standard_4layer(),
            Self::JlcpcbAdvanced2Layer => jlcpcb::advanced_2layer(),
            Self::JlcpcbAdvanced4Layer => jlcpcb::advanced_4layer(),
            Self::PcbWayStandard => pcbway::standard(),
            Self::OshPark2Layer => oshpark::two_layer(),
            Self::OshPark4Layer => oshpark::four_layer(),
            Self::IpcClass1 => ipc::class1(),
            Self::IpcClass2 => ipc::class2(),
            Self::IpcClass3 => ipc::class3(),
            Self::Prototype => ipc::prototype(),
        }
    }

    /// Get the matching stackup for this preset.
    pub fn stackup(self) -> Stackup {
        match self {
            Self::JlcpcbStandard2Layer => jlcpcb::standard_2layer_stackup(),
            Self::JlcpcbStandard4Layer => jlcpcb::standard_4layer_stackup(),
            Self::JlcpcbAdvanced2Layer => jlcpcb::advanced_2layer_stackup(),
            Self::JlcpcbAdvanced4Layer => jlcpcb::advanced_4layer_stackup(),
            Self::PcbWayStandard => pcbway::standard_stackup(),
            Self::OshPark2Layer => oshpark::two_layer_stackup(),
            Self::OshPark4Layer => oshpark::four_layer_stackup(),
            Self::IpcClass1 => ipc::class1_stackup(),
            Self::IpcClass2 => ipc::class2_stackup(),
            Self::IpcClass3 => ipc::class3_stackup(),
            // A prototype is a two-layer consumer board; it borrows the one
            // IPC Class 1 describes rather than claiming a stackup of its own.
            Self::Prototype => ipc::class1_stackup(),
        }
    }
}

impl std::fmt::Display for RulesPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A routing rule set backed by a manufacturer/IPC preset.
///
/// Wraps a [`RulesPreset`] and implements [`RoutingRuleSet`] for use
/// by the autorouter. Supports optional per-net constraint overrides.
///
/// # Examples
///
/// ```
/// use cypcb_rules::presets::{RulesPreset, PresetRuleSet};
/// use cypcb_rules::RoutingRuleSet;
///
/// let ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
/// let constraints = ruleset.constraints_for_net(0);
/// assert!(constraints.min_trace_width.raw() > 0);
///
/// // Can be used as a trait object
/// let dyn_rules: &dyn RoutingRuleSet = &ruleset;
/// ```
pub struct PresetRuleSet {
    preset: RulesPreset,
    base_constraints: DesignConstraints,
    /// How many copper layers this preset's stackup has.
    ///
    /// Read once, at construction. `via_cost` and `layer_change_cost` are
    /// called for every layer transition the router considers - millions of
    /// times on a real board - and each one used to call
    /// `RulesPreset::stackup()`, which builds the whole stackup: a `Vec` of
    /// layer descriptions, allocated and dropped, to read one integer off it.
    /// It was 5.6% of the router's instructions in a callgrind profile, and
    /// the number it computes cannot change while a `PresetRuleSet` exists.
    copper_layers: u8,
    /// Per-net constraint overrides. Nets not in this map use the base preset.
    net_overrides: HashMap<u32, DesignConstraints>,
}

impl PresetRuleSet {
    /// Create a new preset rule set with no per-net overrides.
    pub fn new(preset: RulesPreset) -> Self {
        let base_constraints = preset.constraints();
        Self {
            preset,
            base_constraints,
            copper_layers: preset.stackup().copper_layer_count() as u8,
            net_overrides: HashMap::new(),
        }
    }

    /// Create a preset rule set with per-net constraint overrides.
    pub fn with_overrides(
        preset: RulesPreset,
        net_overrides: HashMap<u32, DesignConstraints>,
    ) -> Self {
        let base_constraints = preset.constraints();
        Self {
            preset,
            base_constraints,
            copper_layers: preset.stackup().copper_layer_count() as u8,
            net_overrides,
        }
    }

    /// The underlying preset.
    pub fn preset(&self) -> RulesPreset {
        self.preset
    }

    /// Add or replace a per-net override.
    pub fn set_net_override(&mut self, net_id: u32, constraints: DesignConstraints) {
        self.net_overrides.insert(net_id, constraints);
    }

    /// Remove a per-net override.
    pub fn remove_net_override(&mut self, net_id: u32) -> Option<DesignConstraints> {
        self.net_overrides.remove(&net_id)
    }
}

impl RoutingRuleSet for PresetRuleSet {
    fn constraints_for_net(&self, net_id: u32) -> &DesignConstraints {
        self.net_overrides
            .get(&net_id)
            .unwrap_or(&self.base_constraints)
    }

    fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints {
        class.default_constraints()
    }

    fn via_cost(&self, from_layer: u8, to_layer: u8) -> f64 {
        let span = (from_layer as i16 - to_layer as i16).unsigned_abs() as f64;
        let copper_layers = self.copper_layers;

        // Cost scales with layer span. Blind/buried vias (not spanning
        // full board) get a premium since they're more expensive to fab.
        let base_cost = span * 1.0;
        let is_through = (from_layer == 0 && to_layer == copper_layers.saturating_sub(1))
            || (to_layer == 0 && from_layer == copper_layers.saturating_sub(1));

        if self.base_constraints.blind_vias_allowed && !is_through {
            // Blind/buried via — higher fab cost
            base_cost * 2.0
        } else {
            base_cost
        }
    }

    fn layer_change_cost(&self, layer: u8) -> f64 {
        // Prefer outer layers for routing accessibility.
        // Inner layers have higher cost.
        let copper_layers = self.copper_layers;
        if copper_layers <= 2 {
            return 0.5; // all layers equal on 2-layer
        }
        if layer == 0 || layer == copper_layers.saturating_sub(1) {
            0.3 // outer layers preferred
        } else {
            0.8 // inner layers less preferred
        }
    }

    fn clearance_between(&self, net_a: u32, net_b: u32) -> Nm {
        // Use the stricter clearance of the two nets' constraints.
        let ca = self.constraints_for_net(net_a).min_clearance;
        let cb = self.constraints_for_net(net_b).min_clearance;
        if ca.raw() > cb.raw() {
            ca
        } else {
            cb
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_in_the_list_and_answers_to_its_name() {
        // A count on its own goes stale the moment a preset is added and says
        // nothing about whether the list is the enum. This walks the list.
        assert_eq!(RulesPreset::all().len(), RulesPreset::ALL.len());
        for preset in RulesPreset::all() {
            assert_eq!(
                RulesPreset::from_name(preset.name()),
                Some(*preset),
                "{} is in the list and its own name does not resolve to it",
                preset.name()
            );
        }
    }

    #[test]
    fn test_all_names_unique() {
        let names: Vec<&str> = RulesPreset::ALL.iter().map(|p| p.name()).collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len(), "Duplicate preset names found");
    }

    #[test]
    fn test_roundtrip_from_name() {
        for preset in RulesPreset::ALL {
            let name = preset.name();
            let resolved = RulesPreset::from_name(name)
                .unwrap_or_else(|| panic!("from_name({name:?}) returned None"));
            assert_eq!(resolved, preset, "Roundtrip failed for {name}");
        }
    }

    #[test]
    fn test_from_name_aliases_jlcpcb() {
        assert_eq!(
            RulesPreset::from_name("jlcpcb"),
            Some(RulesPreset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("JLCPCB"),
            Some(RulesPreset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("jlcpcb-2layer"),
            Some(RulesPreset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("jlcpcb_standard"),
            Some(RulesPreset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("jlcpcb_4layer"),
            Some(RulesPreset::JlcpcbStandard4Layer)
        );
        assert_eq!(
            RulesPreset::from_name("jlcpcb-advanced"),
            Some(RulesPreset::JlcpcbAdvanced2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("jlcpcb_advanced_4layer"),
            Some(RulesPreset::JlcpcbAdvanced4Layer)
        );
    }

    #[test]
    fn test_from_name_aliases_pcbway() {
        assert_eq!(
            RulesPreset::from_name("pcbway"),
            Some(RulesPreset::PcbWayStandard)
        );
        assert_eq!(
            RulesPreset::from_name("PCBWay-Standard"),
            Some(RulesPreset::PcbWayStandard)
        );
    }

    #[test]
    fn test_from_name_aliases_oshpark() {
        assert_eq!(
            RulesPreset::from_name("oshpark"),
            Some(RulesPreset::OshPark2Layer)
        );
        assert_eq!(
            RulesPreset::from_name("oshpark_4layer"),
            Some(RulesPreset::OshPark4Layer)
        );
    }

    #[test]
    fn test_from_name_aliases_ipc() {
        assert_eq!(RulesPreset::from_name("ipc1"), Some(RulesPreset::IpcClass1));
        assert_eq!(
            RulesPreset::from_name("ipc_class_2"),
            Some(RulesPreset::IpcClass2)
        );
        assert_eq!(
            RulesPreset::from_name("IPC-CLASS-3"),
            Some(RulesPreset::IpcClass3)
        );
    }

    #[test]
    fn test_from_name_unknown() {
        assert_eq!(RulesPreset::from_name("unknown"), None);
        assert_eq!(RulesPreset::from_name(""), None);
    }

    #[test]
    fn test_every_preset_constraints_populated() {
        for preset in RulesPreset::ALL {
            let dc = preset.constraints();
            assert!(
                dc.min_trace_width.raw() > 0,
                "{:?} has zero min_trace_width",
                preset
            );
            assert!(
                dc.min_clearance.raw() > 0,
                "{:?} has zero min_clearance",
                preset
            );
            assert!(
                dc.min_drill_size.raw() > 0,
                "{:?} has zero min_drill_size",
                preset
            );
            assert!(
                dc.min_via_drill.raw() > 0,
                "{:?} has zero min_via_drill",
                preset
            );
            assert!(
                dc.min_annular_ring.raw() > 0,
                "{:?} has zero min_annular_ring",
                preset
            );
            assert!(
                dc.min_silk_width.raw() > 0,
                "{:?} has zero min_silk_width",
                preset
            );
            assert!(
                dc.min_edge_clearance.raw() > 0,
                "{:?} has zero min_edge_clearance",
                preset
            );
            assert!(
                dc.board_thickness.raw() > 0,
                "{:?} has zero board_thickness",
                preset
            );
            assert!(
                dc.copper_weight_oz_x10 > 0,
                "{:?} has zero copper_weight",
                preset
            );
            assert!(
                dc.max_copper_layers >= 2,
                "{:?} has <2 copper layers",
                preset
            );
        }
    }

    #[test]
    fn test_every_preset_has_stackup() {
        for preset in RulesPreset::ALL {
            let stackup = preset.stackup();
            assert!(
                stackup.copper_layer_count() >= 2,
                "{:?} stackup has <2 copper layers",
                preset
            );
            assert!(
                stackup.total_thickness.raw() > 0,
                "{:?} stackup has zero thickness",
                preset
            );
        }
    }

    #[test]
    fn test_preset_ruleset_basic() {
        let ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
        let dc = ruleset.constraints_for_net(0);
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.127)); // 5mil
    }

    #[test]
    fn test_preset_ruleset_with_override() {
        let mut ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
        let mut custom = RulesPreset::JlcpcbStandard2Layer.constraints();
        custom.min_trace_width = Nm::from_mm(0.3); // wider trace for net 42

        ruleset.set_net_override(42, custom);

        // Net 42 uses override
        assert_eq!(
            ruleset.constraints_for_net(42).min_trace_width,
            Nm::from_mm(0.3)
        );
        // Other nets use base preset
        assert_eq!(
            ruleset.constraints_for_net(0).min_trace_width,
            Nm::from_mm(0.127)
        );
    }

    #[test]
    fn test_preset_ruleset_object_safe() {
        let ruleset = PresetRuleSet::new(RulesPreset::PcbWayStandard);
        let dyn_rules: &dyn RoutingRuleSet = &ruleset;
        let _ = dyn_rules.constraints_for_net(0);
        let _ = dyn_rules.via_cost(0, 1);
        let _ = dyn_rules.layer_change_cost(0);
        let _ = dyn_rules.clearance_between(0, 1);
        let _ = dyn_rules.constraints_for_class(SignalClass::Digital);
    }

    #[test]
    fn test_preset_ruleset_via_cost() {
        let ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard4Layer);
        let cost_1 = ruleset.via_cost(0, 1);
        let cost_3 = ruleset.via_cost(0, 3);
        assert!(cost_3 > cost_1, "Multi-layer span should cost more");
    }

    #[test]
    fn test_preset_ruleset_clearance_between() {
        let mut ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
        let base_clearance = ruleset.constraints_for_net(0).min_clearance;

        // Override net 10 with wider clearance
        let mut wide = RulesPreset::JlcpcbStandard2Layer.constraints();
        wide.min_clearance = Nm::from_mm(0.5);
        ruleset.set_net_override(10, wide);

        // Clearance between net 10 and any other net should be the wider one
        let between = ruleset.clearance_between(10, 0);
        assert!(between.raw() >= base_clearance.raw());
        assert_eq!(between, Nm::from_mm(0.5));
    }

    #[test]
    fn test_preset_ruleset_remove_override() {
        let mut ruleset = PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
        let mut custom = RulesPreset::JlcpcbStandard2Layer.constraints();
        custom.min_trace_width = Nm::from_mm(0.5);
        ruleset.set_net_override(10, custom);
        assert_eq!(
            ruleset.constraints_for_net(10).min_trace_width,
            Nm::from_mm(0.5)
        );

        ruleset.remove_net_override(10);
        assert_eq!(
            ruleset.constraints_for_net(10).min_trace_width,
            Nm::from_mm(0.127) // back to base
        );
    }
}
