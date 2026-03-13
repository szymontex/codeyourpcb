//! IPC-2221 voltage-based clearance lookup tables.
//!
//! Implements Table 6-1 from IPC-2221B for minimum electrical clearance
//! (creepage) distances based on peak voltage and coating conditions.
//!
//! Source: IPC-2221B "Generic Standard on Printed Board Design", Table 6-1
//! Reference: <https://www.ipc.org/ipc-2221>
//!
//! Note: IPC-2221 is behind a paywall. These values are from widely-published
//! summaries (Cadence, Altium, ProtoExpress references) and represent the
//! standard breakpoints for internal/external conductors.

use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// Surface coating/environmental condition for clearance lookup.
///
/// Different coating conditions affect the minimum clearance required
/// for a given voltage. Conformal coating and sea-level conditions have
/// different dielectric breakdown characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CoatingType {
    /// Bare board, no conformal coating. External conductors, sea level.
    /// Most conservative clearance requirements.
    Bare,
    /// Conformal-coated board (acrylic, silicone, urethane, epoxy coating).
    /// Reduced clearance requirements due to improved dielectric strength.
    ConformCoat,
    /// Bare board at sea level altitude (≤3050m / 10,000ft).
    /// Slightly more conservative than conformal-coated.
    SeaLevel,
}

/// IPC-2221 voltage clearance breakpoints.
///
/// Table 6-1 defines minimum clearances at specific voltage thresholds.
/// The lookup returns the clearance for the *lowest* breakpoint that is
/// ≥ the input voltage.
///
/// Breakpoints (DC or AC peak voltage):
/// - 0–15V
/// - 16–30V
/// - 31–50V
/// - 51–100V
/// - 101–150V
/// - 151–170V
/// - 171–250V
/// - 251–300V
/// - 301–500V
///
/// Above 500V: extrapolate at ~0.25mm per 100V additional (bare),
/// reduced proportionally for coated.
///
/// Minimum electrical clearance for a given voltage and coating condition.
///
/// Based on IPC-2221B Table 6-1 for external conductors (B1/B2 columns).
///
/// # Arguments
///
/// * `voltage_v` - Peak voltage (DC or AC peak) between conductors.
///   Negative values are treated as their absolute value.
/// * `coating` - Surface coating condition.
///
/// # Returns
///
/// Minimum clearance distance as [`Nm`].
///
/// # Examples
///
/// ```
/// use cypcb_rules::clearance_table::{voltage_clearance, CoatingType};
/// use cypcb_core::Nm;
///
/// // Low voltage (0-15V): 0.1mm bare
/// let clearance = voltage_clearance(5.0, CoatingType::Bare);
/// assert_eq!(clearance, Nm::from_mm(0.1));
///
/// // Higher voltage needs more clearance
/// let clearance_100v = voltage_clearance(100.0, CoatingType::Bare);
/// assert!(clearance_100v.raw() > clearance.raw());
/// ```
pub fn voltage_clearance(voltage_v: f64, coating: CoatingType) -> Nm {
    let v = voltage_v.abs();

    // IPC-2221B Table 6-1 breakpoints
    // Values in mm for each coating type at each voltage threshold.
    //
    // Columns: (Bare, ConformCoat, SeaLevel)
    // These values represent minimum clearance for EXTERNAL conductors.
    //
    // Source: IPC-2221B Table 6-1, B1/B2/B4 columns
    let (bare_mm, coated_mm, sea_level_mm) = if v <= 15.0 {
        // 0-15V
        (0.1, 0.05, 0.1)
    } else if v <= 30.0 {
        // 16-30V
        (0.1, 0.05, 0.1)
    } else if v <= 50.0 {
        // 31-50V
        (0.6, 0.13, 0.6)
    } else if v <= 100.0 {
        // 51-100V
        (0.6, 0.13, 1.5)
    } else if v <= 150.0 {
        // 101-150V
        (0.6, 0.4, 3.2)
    } else if v <= 170.0 {
        // 151-170V
        (1.25, 0.4, 3.2)
    } else if v <= 250.0 {
        // 171-250V
        (1.25, 0.4, 6.4)
    } else if v <= 300.0 {
        // 251-300V
        (1.25, 0.4, 6.4)
    } else if v <= 500.0 {
        // 301-500V
        (2.5, 0.8, 12.5)
    } else {
        // >500V: extrapolate linearly from 500V breakpoint
        // Add ~0.25mm per 100V for bare, ~0.08mm for coated, ~1.25mm for sea level
        let extra_v = v - 500.0;
        let bare = 2.5 + (extra_v / 100.0) * 0.25;
        let coated = 0.8 + (extra_v / 100.0) * 0.08;
        let sea = 12.5 + (extra_v / 100.0) * 1.25;
        return match coating {
            CoatingType::Bare => Nm::from_mm(bare),
            CoatingType::ConformCoat => Nm::from_mm(coated),
            CoatingType::SeaLevel => Nm::from_mm(sea),
        };
    };

    let mm = match coating {
        CoatingType::Bare => bare_mm,
        CoatingType::ConformCoat => coated_mm,
        CoatingType::SeaLevel => sea_level_mm,
    };

    Nm::from_mm(mm)
}

/// Returns the full IPC-2221 clearance table as a vector of
/// `(max_voltage, bare_mm, coated_mm, sea_level_mm)` tuples.
///
/// Useful for displaying the table to users or for custom lookups.
pub fn clearance_table() -> Vec<(f64, f64, f64, f64)> {
    vec![
        (15.0, 0.1, 0.05, 0.1),
        (30.0, 0.1, 0.05, 0.1),
        (50.0, 0.6, 0.13, 0.6),
        (100.0, 0.6, 0.13, 1.5),
        (150.0, 0.6, 0.4, 3.2),
        (170.0, 1.25, 0.4, 3.2),
        (250.0, 1.25, 0.4, 6.4),
        (300.0, 1.25, 0.4, 6.4),
        (500.0, 2.5, 0.8, 12.5),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_voltage_bare() {
        // 0-15V: 0.1mm
        assert_eq!(voltage_clearance(0.0, CoatingType::Bare), Nm::from_mm(0.1));
        assert_eq!(voltage_clearance(5.0, CoatingType::Bare), Nm::from_mm(0.1));
        assert_eq!(voltage_clearance(15.0, CoatingType::Bare), Nm::from_mm(0.1));
    }

    #[test]
    fn test_low_voltage_coated() {
        // 0-15V coated: 0.05mm
        assert_eq!(
            voltage_clearance(5.0, CoatingType::ConformCoat),
            Nm::from_mm(0.05)
        );
    }

    #[test]
    fn test_30v_bare() {
        // 16-30V: still 0.1mm bare
        assert_eq!(voltage_clearance(25.0, CoatingType::Bare), Nm::from_mm(0.1));
    }

    #[test]
    fn test_50v_bare() {
        // 31-50V: 0.6mm bare
        assert_eq!(voltage_clearance(50.0, CoatingType::Bare), Nm::from_mm(0.6));
    }

    #[test]
    fn test_100v_bare() {
        // 51-100V: 0.6mm bare
        assert_eq!(
            voltage_clearance(100.0, CoatingType::Bare),
            Nm::from_mm(0.6)
        );
    }

    #[test]
    fn test_100v_sea_level() {
        // 51-100V sea level: 1.5mm
        assert_eq!(
            voltage_clearance(100.0, CoatingType::SeaLevel),
            Nm::from_mm(1.5)
        );
    }

    #[test]
    fn test_250v_bare() {
        // 171-250V: 1.25mm bare
        assert_eq!(
            voltage_clearance(250.0, CoatingType::Bare),
            Nm::from_mm(1.25)
        );
    }

    #[test]
    fn test_500v_bare() {
        // 301-500V: 2.5mm bare
        assert_eq!(
            voltage_clearance(500.0, CoatingType::Bare),
            Nm::from_mm(2.5)
        );
    }

    #[test]
    fn test_500v_coated() {
        // 301-500V coated: 0.8mm
        assert_eq!(
            voltage_clearance(500.0, CoatingType::ConformCoat),
            Nm::from_mm(0.8)
        );
    }

    #[test]
    fn test_clearance_increases_with_voltage() {
        let voltages = [5.0, 30.0, 50.0, 100.0, 150.0, 250.0, 500.0];
        let mut prev = Nm(0);
        for v in voltages {
            let c = voltage_clearance(v, CoatingType::Bare);
            assert!(
                c.raw() >= prev.raw(),
                "Clearance should not decrease: {v}V gives {c:?} < {prev:?}"
            );
            prev = c;
        }
    }

    #[test]
    fn test_coated_less_than_bare() {
        for v in [5.0, 50.0, 150.0, 500.0] {
            let bare = voltage_clearance(v, CoatingType::Bare);
            let coated = voltage_clearance(v, CoatingType::ConformCoat);
            assert!(
                coated.raw() <= bare.raw(),
                "Coated should be <= bare at {v}V: coated={coated:?}, bare={bare:?}"
            );
        }
    }

    #[test]
    fn test_extrapolation_above_500v() {
        let c500 = voltage_clearance(500.0, CoatingType::Bare);
        let c600 = voltage_clearance(600.0, CoatingType::Bare);
        let c1000 = voltage_clearance(1000.0, CoatingType::Bare);

        assert!(c600.raw() > c500.raw(), "600V should exceed 500V clearance");
        assert!(
            c1000.raw() > c600.raw(),
            "1000V should exceed 600V clearance"
        );
    }

    #[test]
    fn test_negative_voltage_treated_as_absolute() {
        let pos = voltage_clearance(100.0, CoatingType::Bare);
        let neg = voltage_clearance(-100.0, CoatingType::Bare);
        assert_eq!(pos, neg);
    }

    #[test]
    fn test_clearance_table_has_9_entries() {
        assert_eq!(clearance_table().len(), 9);
    }

    #[test]
    fn test_clearance_table_monotonic() {
        let table = clearance_table();
        for i in 1..table.len() {
            assert!(
                table[i].0 >= table[i - 1].0,
                "Voltage breakpoints should increase"
            );
            assert!(
                table[i].1 >= table[i - 1].1,
                "Bare clearance should not decrease"
            );
        }
    }

    #[test]
    fn test_170v_bare() {
        // 151-170V: 1.25mm bare
        assert_eq!(
            voltage_clearance(170.0, CoatingType::Bare),
            Nm::from_mm(1.25)
        );
    }

    #[test]
    fn test_sea_level_grows_fast() {
        // Sea level clearance grows quickly with voltage
        let c50 = voltage_clearance(50.0, CoatingType::SeaLevel);
        let c250 = voltage_clearance(250.0, CoatingType::SeaLevel);
        assert!(
            c250.raw() > c50.raw() * 5,
            "Sea level clearance should grow significantly: 50V={c50:?}, 250V={c250:?}"
        );
    }
}
