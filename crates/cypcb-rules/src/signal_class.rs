//! Signal classification and per-class constraint overrides.
//!
//! Signals on a PCB are categorized by their electrical characteristics.
//! Each [`SignalClass`] maps to a [`SignalClassConstraints`] providing
//! per-class design rule overrides that take precedence over the base
//! [`DesignConstraints`].

use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// Signal classification for PCB nets.
///
/// Each net in a design is assigned a signal class that determines
/// the routing rules and constraints applied to it.
///
/// # Examples
///
/// ```
/// use cypcb_rules::SignalClass;
///
/// let class = SignalClass::HighSpeed;
/// let constraints = class.default_constraints();
/// assert!(constraints.min_trace_width.raw() > 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalClass {
    /// Standard digital signals (GPIO, chip select, reset, etc.).
    Digital,
    /// High-speed digital signals (USB, HDMI, PCIe, DDR clocks, etc.).
    /// Requires controlled impedance, length matching, and stub limits.
    HighSpeed,
    /// Analog signals (ADC/DAC, sensor inputs, audio, etc.).
    /// Requires wider clearances and guard traces.
    Analog,
    /// Power distribution (VCC, GND, voltage rails).
    /// Requires wider traces for current capacity.
    Power,
    /// Differential pair signals (USB data, LVDS, Ethernet, etc.).
    /// Requires matched impedance, tight gap control, and length matching.
    Differential,
}

impl SignalClass {
    /// Returns all signal class variants.
    pub const ALL: [SignalClass; 5] = [
        SignalClass::Digital,
        SignalClass::HighSpeed,
        SignalClass::Analog,
        SignalClass::Power,
        SignalClass::Differential,
    ];

    /// Return default constraints for this signal class.
    ///
    /// These are sensible defaults based on IPC standards and common
    /// PCB design practice. Override per-project as needed.
    pub fn default_constraints(self) -> SignalClassConstraints {
        match self {
            SignalClass::Digital => SignalClassConstraints {
                min_trace_width: Nm::from_mm(0.15), // 6 mil — comfortable digital
                min_clearance: Nm::from_mm(0.15),   // 6 mil
                preferred_layers: Vec::new(),       // any copper layer
                require_impedance_control: false,
                require_length_matching: false,
                require_diff_pair: false,
                max_stub_length: None,
                guard_trace_clearance: None,
            },
            SignalClass::HighSpeed => SignalClassConstraints {
                min_trace_width: Nm::from_mm(0.127), // 5 mil — controlled impedance
                min_clearance: Nm::from_mm(0.2),     // 8 mil — extra clearance
                preferred_layers: vec![0, 2],        // outer + inner-1
                require_impedance_control: true,
                require_length_matching: true,
                require_diff_pair: false,
                max_stub_length: Some(Nm::from_mm(0.5)), // short stubs only
                guard_trace_clearance: None,
            },
            SignalClass::Analog => SignalClassConstraints {
                min_trace_width: Nm::from_mm(0.2), // 8 mil — low noise
                min_clearance: Nm::from_mm(0.3),   // 12 mil — isolation
                preferred_layers: Vec::new(),
                require_impedance_control: false,
                require_length_matching: false,
                require_diff_pair: false,
                max_stub_length: None,
                guard_trace_clearance: Some(Nm::from_mm(0.5)), // guard traces
            },
            SignalClass::Power => SignalClassConstraints {
                min_trace_width: Nm::from_mm(0.5), // 20 mil — current capacity
                min_clearance: Nm::from_mm(0.2),   // 8 mil
                preferred_layers: Vec::new(),
                require_impedance_control: false,
                require_length_matching: false,
                require_diff_pair: false,
                max_stub_length: None,
                guard_trace_clearance: None,
            },
            SignalClass::Differential => SignalClassConstraints {
                min_trace_width: Nm::from_mm(0.127), // 5 mil — impedance controlled
                min_clearance: Nm::from_mm(0.2),     // 8 mil
                preferred_layers: vec![0],           // outer layer preferred
                require_impedance_control: true,
                require_length_matching: true,
                require_diff_pair: true,
                max_stub_length: Some(Nm::from_mm(0.3)),
                guard_trace_clearance: None,
            },
        }
    }
}

/// Per-signal-class constraint overrides.
///
/// When a net belongs to a signal class, these constraints override or
/// supplement the base [`DesignConstraints`]. Fields here represent
/// overrides — a routing engine should use the *stricter* of the base
/// constraint and the class constraint.
///
/// # Layer indices
///
/// `preferred_layers` uses `u8` layer indices (0 = top copper, 1 = inner-1, etc.)
/// to avoid depending on `cypcb-world`'s `Layer` enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalClassConstraints {
    /// Minimum trace width for this class.
    pub min_trace_width: Nm,
    /// Minimum clearance from other signals.
    pub min_clearance: Nm,
    /// Preferred routing layers (indices). Empty = no preference.
    pub preferred_layers: Vec<u8>,
    /// Whether impedance control is required.
    pub require_impedance_control: bool,
    /// Whether length matching is required within the net group.
    pub require_length_matching: bool,
    /// Whether this class requires differential pair routing.
    pub require_diff_pair: bool,
    /// Maximum stub length. `None` = no restriction.
    pub max_stub_length: Option<Nm>,
    /// Clearance for guard traces. `None` = no guard traces needed.
    pub guard_trace_clearance: Option<Nm>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_covered() {
        // Ensure default_constraints() works for every variant
        for class in SignalClass::ALL {
            let c = class.default_constraints();
            assert!(
                c.min_trace_width.raw() > 0,
                "{class:?} has zero trace width"
            );
            assert!(c.min_clearance.raw() > 0, "{class:?} has zero clearance");
        }
    }

    #[test]
    fn test_power_has_wider_traces() {
        let digital = SignalClass::Digital.default_constraints();
        let power = SignalClass::Power.default_constraints();
        assert!(
            power.min_trace_width.raw() > digital.min_trace_width.raw(),
            "Power traces should be wider than digital"
        );
    }

    #[test]
    fn test_high_speed_requires_impedance() {
        let hs = SignalClass::HighSpeed.default_constraints();
        assert!(hs.require_impedance_control);
        assert!(hs.require_length_matching);
    }

    #[test]
    fn test_differential_requires_pair_routing() {
        let diff = SignalClass::Differential.default_constraints();
        assert!(diff.require_diff_pair);
        assert!(diff.require_impedance_control);
        assert!(diff.require_length_matching);
    }

    #[test]
    fn test_analog_has_guard_trace_clearance() {
        let analog = SignalClass::Analog.default_constraints();
        assert!(analog.guard_trace_clearance.is_some());
        assert!(analog.guard_trace_clearance.unwrap().raw() > 0);
    }

    #[test]
    fn test_high_speed_has_stub_limit() {
        let hs = SignalClass::HighSpeed.default_constraints();
        assert!(hs.max_stub_length.is_some());
        assert!(hs.max_stub_length.unwrap().raw() > 0);
    }

    #[test]
    fn test_signal_class_serde_roundtrip() {
        let class = SignalClass::Differential;
        let json = serde_json::to_string(&class).unwrap();
        let class2: SignalClass = serde_json::from_str(&json).unwrap();
        assert_eq!(class, class2);
    }

    #[test]
    fn test_signal_class_constraints_serde_roundtrip() {
        let sc = SignalClass::HighSpeed.default_constraints();
        let json = serde_json::to_string(&sc).unwrap();
        let sc2: SignalClassConstraints = serde_json::from_str(&json).unwrap();
        assert_eq!(sc, sc2);
    }

    #[test]
    fn test_analog_wider_clearance_than_digital() {
        let digital = SignalClass::Digital.default_constraints();
        let analog = SignalClass::Analog.default_constraints();
        assert!(
            analog.min_clearance.raw() > digital.min_clearance.raw(),
            "Analog should have wider clearance for noise isolation"
        );
    }
}
