//! PCB layer stackup definitions.
//!
//! A [`Stackup`] describes the physical layer construction of a PCB:
//! copper layers, dielectric materials, solder mask, and silkscreen.
//! Factory methods provide common stackup configurations.

use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// Type of layer in a PCB stackup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayerType {
    /// Copper signal/routing layer.
    Signal,
    /// Copper plane layer (ground or power).
    Plane,
    /// Dielectric (insulating) material between copper layers.
    Dielectric,
    /// Solder mask layer.
    SolderMask,
    /// Silkscreen layer.
    Silkscreen,
}

impl LayerType {
    /// Whether this layer type contains copper.
    pub fn is_copper(self) -> bool {
        matches!(self, LayerType::Signal | LayerType::Plane)
    }
}

/// A single layer entry in a PCB stackup.
///
/// Describes one physical layer with its type, thickness, material
/// properties, and copper weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerStackEntry {
    /// Human-readable name (e.g. "Top Copper", "Core", "Prepreg 1").
    pub name: String,
    /// Layer type.
    pub layer_type: LayerType,
    /// Layer thickness.
    pub thickness: Nm,
    /// Material name (e.g. "FR-4", "Copper", "LPI").
    pub material: String,
    /// Copper weight in oz/ft² × 10 (e.g. 10 = 1.0 oz). Zero for non-copper layers.
    pub copper_weight_oz_x10: u32,
    /// Dielectric constant (εr × 1000 for integer precision, e.g. 4500 = 4.500).
    /// Zero for copper layers.
    pub dielectric_constant_x1000: u32,
}

impl LayerStackEntry {
    /// Create a copper signal layer.
    pub fn signal(name: impl Into<String>, thickness: Nm, copper_oz_x10: u32) -> Self {
        Self {
            name: name.into(),
            layer_type: LayerType::Signal,
            thickness,
            material: "Copper".into(),
            copper_weight_oz_x10: copper_oz_x10,
            dielectric_constant_x1000: 0,
        }
    }

    /// Create a copper plane layer.
    pub fn plane(name: impl Into<String>, thickness: Nm, copper_oz_x10: u32) -> Self {
        Self {
            name: name.into(),
            layer_type: LayerType::Plane,
            thickness,
            material: "Copper".into(),
            copper_weight_oz_x10: copper_oz_x10,
            dielectric_constant_x1000: 0,
        }
    }

    /// Create a dielectric layer.
    pub fn dielectric(
        name: impl Into<String>,
        thickness: Nm,
        material: impl Into<String>,
        er_x1000: u32,
    ) -> Self {
        Self {
            name: name.into(),
            layer_type: LayerType::Dielectric,
            thickness,
            material: material.into(),
            copper_weight_oz_x10: 0,
            dielectric_constant_x1000: er_x1000,
        }
    }

    /// Create a solder mask layer.
    pub fn solder_mask(name: impl Into<String>, thickness: Nm) -> Self {
        Self {
            name: name.into(),
            layer_type: LayerType::SolderMask,
            thickness,
            material: "LPI".into(),
            copper_weight_oz_x10: 0,
            dielectric_constant_x1000: 3500, // typical LPI εr ≈ 3.5
        }
    }

    /// Create a silkscreen layer.
    pub fn silkscreen(name: impl Into<String>, thickness: Nm) -> Self {
        Self {
            name: name.into(),
            layer_type: LayerType::Silkscreen,
            thickness,
            material: "Epoxy Ink".into(),
            copper_weight_oz_x10: 0,
            dielectric_constant_x1000: 0,
        }
    }
}

/// Complete PCB layer stackup.
///
/// Describes the full physical construction from top to bottom. Layers are
/// ordered top-to-bottom: `layers[0]` is the topmost physical layer (typically
/// top silkscreen), `layers[last]` is the bottommost.
///
/// # Examples
///
/// ```
/// use cypcb_rules::Stackup;
///
/// let stackup = Stackup::two_layer_1oz();
/// assert_eq!(stackup.name, "2-Layer 1oz");
/// assert_eq!(stackup.copper_layer_count(), 2);
/// assert!(stackup.total_thickness.raw() > 0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stackup {
    /// Descriptive name for this stackup configuration.
    pub name: String,
    /// Ordered layers from top to bottom.
    pub layers: Vec<LayerStackEntry>,
    /// Total board thickness (sum of all layer thicknesses).
    pub total_thickness: Nm,
}

impl Stackup {
    /// Count the number of copper layers (Signal + Plane).
    pub fn copper_layer_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|l| l.layer_type.is_copper())
            .count()
    }

    /// Get all copper layer entries.
    pub fn copper_layers(&self) -> Vec<&LayerStackEntry> {
        self.layers
            .iter()
            .filter(|l| l.layer_type.is_copper())
            .collect()
    }

    /// Get the total copper thickness.
    pub fn total_copper_thickness(&self) -> Nm {
        Nm(self
            .layers
            .iter()
            .filter(|l| l.layer_type.is_copper())
            .map(|l| l.thickness.raw())
            .sum())
    }

    /// Standard 2-layer, 1oz copper stackup.
    ///
    /// Total thickness: ~1.6mm (standard FR-4).
    /// Layers: Silkscreen / Mask / Top Cu / Core / Bottom Cu / Mask / Silkscreen
    pub fn two_layer_1oz() -> Self {
        let cu_thickness = Nm::from_mm(0.035); // 1oz copper ≈ 35µm
        let core_thickness = Nm::from_mm(1.5); // FR-4 core
        let mask_thickness = Nm::from_mm(0.01);
        let silk_thickness = Nm::from_mm(0.005);

        let layers = vec![
            LayerStackEntry::silkscreen("Top Silkscreen", silk_thickness),
            LayerStackEntry::solder_mask("Top Solder Mask", mask_thickness),
            LayerStackEntry::signal("Top Copper", cu_thickness, 10),
            LayerStackEntry::dielectric("Core", core_thickness, "FR-4", 4500),
            LayerStackEntry::signal("Bottom Copper", cu_thickness, 10),
            LayerStackEntry::solder_mask("Bottom Solder Mask", mask_thickness),
            LayerStackEntry::silkscreen("Bottom Silkscreen", silk_thickness),
        ];

        let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

        Self {
            name: "2-Layer 1oz".into(),
            layers,
            total_thickness: total,
        }
    }

    /// Standard 4-layer stackup.
    ///
    /// Signal / GND Plane / Power Plane / Signal.
    /// Total thickness: ~1.6mm.
    pub fn four_layer_standard() -> Self {
        let outer_cu = Nm::from_mm(0.035); // 1oz
        let inner_cu = Nm::from_mm(0.035); // 1oz inner
        let prepreg = Nm::from_mm(0.2); // prepreg between outer and plane
        let core = Nm::from_mm(1.0); // core between planes
        let mask_thickness = Nm::from_mm(0.01);
        let silk_thickness = Nm::from_mm(0.005);

        let layers = vec![
            LayerStackEntry::silkscreen("Top Silkscreen", silk_thickness),
            LayerStackEntry::solder_mask("Top Solder Mask", mask_thickness),
            LayerStackEntry::signal("Top Copper (L1)", outer_cu, 10),
            LayerStackEntry::dielectric("Prepreg 1", prepreg, "FR-4 Prepreg", 4200),
            LayerStackEntry::plane("GND Plane (L2)", inner_cu, 10),
            LayerStackEntry::dielectric("Core", core, "FR-4", 4500),
            LayerStackEntry::plane("Power Plane (L3)", inner_cu, 10),
            LayerStackEntry::dielectric("Prepreg 2", prepreg, "FR-4 Prepreg", 4200),
            LayerStackEntry::signal("Bottom Copper (L4)", outer_cu, 10),
            LayerStackEntry::solder_mask("Bottom Solder Mask", mask_thickness),
            LayerStackEntry::silkscreen("Bottom Silkscreen", silk_thickness),
        ];

        let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

        Self {
            name: "4-Layer Standard".into(),
            layers,
            total_thickness: total,
        }
    }

    /// Standard 6-layer stackup.
    ///
    /// Signal / GND / Signal / Signal / Power / Signal.
    /// Total thickness: ~1.6mm.
    pub fn six_layer_standard() -> Self {
        let outer_cu = Nm::from_mm(0.035);
        let inner_cu = Nm::from_mm(0.018); // 0.5oz inner layers
        let prepreg_outer = Nm::from_mm(0.13);
        let core_1 = Nm::from_mm(0.36);
        let core_2 = Nm::from_mm(0.36);
        let prepreg_mid = Nm::from_mm(0.13);
        let mask_thickness = Nm::from_mm(0.01);
        let silk_thickness = Nm::from_mm(0.005);

        let layers = vec![
            LayerStackEntry::silkscreen("Top Silkscreen", silk_thickness),
            LayerStackEntry::solder_mask("Top Solder Mask", mask_thickness),
            LayerStackEntry::signal("Top Copper (L1)", outer_cu, 10),
            LayerStackEntry::dielectric("Prepreg 1", prepreg_outer, "FR-4 Prepreg", 4200),
            LayerStackEntry::plane("GND Plane (L2)", inner_cu, 5),
            LayerStackEntry::dielectric("Core 1", core_1, "FR-4", 4500),
            LayerStackEntry::signal("Inner Signal (L3)", inner_cu, 5),
            LayerStackEntry::dielectric("Prepreg 2", prepreg_mid, "FR-4 Prepreg", 4200),
            LayerStackEntry::signal("Inner Signal (L4)", inner_cu, 5),
            LayerStackEntry::dielectric("Core 2", core_2, "FR-4", 4500),
            LayerStackEntry::plane("Power Plane (L5)", inner_cu, 5),
            LayerStackEntry::dielectric("Prepreg 3", prepreg_outer, "FR-4 Prepreg", 4200),
            LayerStackEntry::signal("Bottom Copper (L6)", outer_cu, 10),
            LayerStackEntry::solder_mask("Bottom Solder Mask", mask_thickness),
            LayerStackEntry::silkscreen("Bottom Silkscreen", silk_thickness),
        ];

        let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

        Self {
            name: "6-Layer Standard".into(),
            layers,
            total_thickness: total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_layer_copper_count() {
        let s = Stackup::two_layer_1oz();
        assert_eq!(s.copper_layer_count(), 2);
    }

    #[test]
    fn test_four_layer_copper_count() {
        let s = Stackup::four_layer_standard();
        assert_eq!(s.copper_layer_count(), 4);
    }

    #[test]
    fn test_six_layer_copper_count() {
        let s = Stackup::six_layer_standard();
        assert_eq!(s.copper_layer_count(), 6);
    }

    #[test]
    fn test_two_layer_total_thickness_reasonable() {
        let s = Stackup::two_layer_1oz();
        let mm = s.total_thickness.to_mm();
        // Should be approximately 1.6mm
        assert!(
            mm > 1.4 && mm < 1.8,
            "2-layer thickness {mm}mm not in 1.4-1.8 range"
        );
    }

    #[test]
    fn test_four_layer_total_thickness_reasonable() {
        let s = Stackup::four_layer_standard();
        let mm = s.total_thickness.to_mm();
        assert!(
            mm > 1.3 && mm < 1.9,
            "4-layer thickness {mm}mm not in 1.3-1.9 range"
        );
    }

    #[test]
    fn test_six_layer_total_thickness_reasonable() {
        let s = Stackup::six_layer_standard();
        let mm = s.total_thickness.to_mm();
        assert!(
            mm > 1.0 && mm < 2.0,
            "6-layer thickness {mm}mm not in 1.0-2.0 range"
        );
    }

    #[test]
    fn test_layer_type_is_copper() {
        assert!(LayerType::Signal.is_copper());
        assert!(LayerType::Plane.is_copper());
        assert!(!LayerType::Dielectric.is_copper());
        assert!(!LayerType::SolderMask.is_copper());
        assert!(!LayerType::Silkscreen.is_copper());
    }

    #[test]
    fn test_copper_layers_returns_only_copper() {
        let s = Stackup::four_layer_standard();
        let copper = s.copper_layers();
        assert_eq!(copper.len(), 4);
        for layer in &copper {
            assert!(layer.layer_type.is_copper());
        }
    }

    #[test]
    fn test_stackup_serde_roundtrip() {
        let s = Stackup::two_layer_1oz();
        let json = serde_json::to_string(&s).unwrap();
        let s2: Stackup = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn test_layer_entry_constructors() {
        let sig = LayerStackEntry::signal("Top", Nm::from_mm(0.035), 10);
        assert_eq!(sig.layer_type, LayerType::Signal);
        assert_eq!(sig.copper_weight_oz_x10, 10);

        let plane = LayerStackEntry::plane("GND", Nm::from_mm(0.035), 10);
        assert_eq!(plane.layer_type, LayerType::Plane);

        let di = LayerStackEntry::dielectric("Core", Nm::from_mm(1.0), "FR-4", 4500);
        assert_eq!(di.layer_type, LayerType::Dielectric);
        assert_eq!(di.dielectric_constant_x1000, 4500);
        assert_eq!(di.copper_weight_oz_x10, 0);

        let mask = LayerStackEntry::solder_mask("Top Mask", Nm::from_mm(0.01));
        assert_eq!(mask.layer_type, LayerType::SolderMask);

        let silk = LayerStackEntry::silkscreen("Top Silk", Nm::from_mm(0.005));
        assert_eq!(silk.layer_type, LayerType::Silkscreen);
    }
}
