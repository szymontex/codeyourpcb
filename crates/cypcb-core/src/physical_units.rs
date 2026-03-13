//! Physical unit types for electrical/electronic quantities.
//!
//! This module provides [`PhysicalUnit`] and [`PhysicalQuantity`] for representing
//! typed physical values in the DSL (e.g., `10kohm`, `3.3V`, `100nF`).
//!
//! This is intentionally separate from [`crate::units::Unit`] which handles
//! length/dimension units with nanometer conversion. Electrical quantities have
//! fundamentally different base units and conversion factors.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Categories of physical quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhysicalQuantity {
    /// Electrical resistance (ohms).
    Resistance,
    /// Electrical capacitance (farads).
    Capacitance,
    /// Electrical inductance (henries).
    Inductance,
    /// Electrical voltage (volts).
    Voltage,
    /// Electrical current (amperes).
    Current,
    /// Frequency (hertz).
    Frequency,
    /// Electrical power (watts).
    Power,
}

impl fmt::Display for PhysicalQuantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhysicalQuantity::Resistance => write!(f, "resistance"),
            PhysicalQuantity::Capacitance => write!(f, "capacitance"),
            PhysicalQuantity::Inductance => write!(f, "inductance"),
            PhysicalQuantity::Voltage => write!(f, "voltage"),
            PhysicalQuantity::Current => write!(f, "current"),
            PhysicalQuantity::Frequency => write!(f, "frequency"),
            PhysicalQuantity::Power => write!(f, "power"),
        }
    }
}

/// Physical units for electrical/electronic quantities.
///
/// Each variant maps to a DSL suffix string and has a known conversion
/// factor to its base SI unit.
///
/// # Base units per quantity
///
/// | Quantity    | Base unit |
/// |------------|-----------|
/// | Resistance | Ohm (Ω)  |
/// | Capacitance| Farad (F) |
/// | Inductance | Henry (H) |
/// | Voltage    | Volt (V)  |
/// | Current    | Ampere (A)|
/// | Frequency  | Hertz (Hz)|
/// | Power      | Watt (W)  |
///
/// # Examples
///
/// ```
/// use cypcb_core::physical_units::PhysicalUnit;
///
/// let unit: PhysicalUnit = "kohm".parse().unwrap();
/// assert_eq!(unit.to_base_f64(10.0), 10_000.0);
/// assert_eq!(unit.suffix(), "kohm");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalUnit {
    // Resistance
    /// Ohms (Ω) — base unit for resistance.
    Ohm,
    /// Kilohms (kΩ) = 1,000 Ω.
    KiloOhm,
    /// Megohms (MΩ) = 1,000,000 Ω.
    MegaOhm,

    // Capacitance
    /// Picofarads (pF) = 1e-12 F.
    PicoFarad,
    /// Nanofarads (nF) = 1e-9 F.
    NanoFarad,
    /// Microfarads (µF) = 1e-6 F.
    MicroFarad,
    /// Millifarads (mF) = 1e-3 F.
    MilliFarad,

    // Inductance
    /// Nanohenries (nH) = 1e-9 H.
    NanoHenry,
    /// Microhenries (µH) = 1e-6 H.
    MicroHenry,
    /// Millihenries (mH) = 1e-3 H.
    MilliHenry,
    /// Henries (H) — base unit for inductance.
    Henry,

    // Voltage
    /// Millivolts (mV) = 1e-3 V.
    MilliVolt,
    /// Volts (V) — base unit for voltage.
    Volt,
    /// Kilovolts (kV) = 1,000 V.
    KiloVolt,

    // Current
    /// Microamperes (µA) = 1e-6 A.
    MicroAmp,
    /// Milliamperes (mA) = 1e-3 A.
    MilliAmp,
    /// Amperes (A) — base unit for current.
    Amp,

    // Frequency
    /// Hertz (Hz) — base unit for frequency.
    Hertz,
    /// Kilohertz (kHz) = 1,000 Hz.
    KiloHertz,
    /// Megahertz (MHz) = 1,000,000 Hz.
    MegaHertz,
    /// Gigahertz (GHz) = 1,000,000,000 Hz.
    GigaHertz,

    // Power
    /// Milliwatts (mW) = 1e-3 W.
    MilliWatt,
    /// Watts (W) — base unit for power.
    Watt,
}

impl PhysicalUnit {
    /// Get the physical quantity category for this unit.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::physical_units::{PhysicalUnit, PhysicalQuantity};
    ///
    /// assert_eq!(PhysicalUnit::KiloOhm.quantity(), PhysicalQuantity::Resistance);
    /// assert_eq!(PhysicalUnit::NanoFarad.quantity(), PhysicalQuantity::Capacitance);
    /// assert_eq!(PhysicalUnit::Volt.quantity(), PhysicalQuantity::Voltage);
    /// ```
    pub const fn quantity(&self) -> PhysicalQuantity {
        match self {
            PhysicalUnit::Ohm | PhysicalUnit::KiloOhm | PhysicalUnit::MegaOhm => {
                PhysicalQuantity::Resistance
            }
            PhysicalUnit::PicoFarad
            | PhysicalUnit::NanoFarad
            | PhysicalUnit::MicroFarad
            | PhysicalUnit::MilliFarad => PhysicalQuantity::Capacitance,
            PhysicalUnit::NanoHenry
            | PhysicalUnit::MicroHenry
            | PhysicalUnit::MilliHenry
            | PhysicalUnit::Henry => PhysicalQuantity::Inductance,
            PhysicalUnit::MilliVolt | PhysicalUnit::Volt | PhysicalUnit::KiloVolt => {
                PhysicalQuantity::Voltage
            }
            PhysicalUnit::MicroAmp | PhysicalUnit::MilliAmp | PhysicalUnit::Amp => {
                PhysicalQuantity::Current
            }
            PhysicalUnit::Hertz
            | PhysicalUnit::KiloHertz
            | PhysicalUnit::MegaHertz
            | PhysicalUnit::GigaHertz => PhysicalQuantity::Frequency,
            PhysicalUnit::MilliWatt | PhysicalUnit::Watt => PhysicalQuantity::Power,
        }
    }

    /// Convert a value in this unit to the base unit for the quantity.
    ///
    /// Base units: ohms, farads, henries, volts, amperes, hertz, watts.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::physical_units::PhysicalUnit;
    ///
    /// assert_eq!(PhysicalUnit::KiloOhm.to_base_f64(10.0), 10_000.0);
    /// assert!((PhysicalUnit::NanoFarad.to_base_f64(100.0) - 1e-7).abs() < 1e-19);
    /// assert_eq!(PhysicalUnit::MegaHertz.to_base_f64(2.4), 2_400_000.0);
    /// ```
    pub fn to_base_f64(&self, value: f64) -> f64 {
        value * self.multiplier()
    }

    /// Convert a value from the base unit to this unit.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::physical_units::PhysicalUnit;
    ///
    /// assert_eq!(PhysicalUnit::KiloOhm.from_base_f64(10_000.0), 10.0);
    /// assert_eq!(PhysicalUnit::MegaHertz.from_base_f64(2_400_000.0), 2.4);
    /// ```
    pub fn from_base_f64(&self, value: f64) -> f64 {
        value / self.multiplier()
    }

    /// Get the DSL suffix string for this unit.
    ///
    /// These match the grammar's `physical_unit` rule exactly.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::physical_units::PhysicalUnit;
    ///
    /// assert_eq!(PhysicalUnit::KiloOhm.suffix(), "kohm");
    /// assert_eq!(PhysicalUnit::NanoFarad.suffix(), "nF");
    /// assert_eq!(PhysicalUnit::Volt.suffix(), "V");
    /// ```
    pub const fn suffix(&self) -> &'static str {
        match self {
            // Resistance
            PhysicalUnit::Ohm => "ohm",
            PhysicalUnit::KiloOhm => "kohm",
            PhysicalUnit::MegaOhm => "Mohm",
            // Capacitance
            PhysicalUnit::PicoFarad => "pF",
            PhysicalUnit::NanoFarad => "nF",
            PhysicalUnit::MicroFarad => "uF",
            PhysicalUnit::MilliFarad => "mF",
            // Inductance
            PhysicalUnit::NanoHenry => "nH",
            PhysicalUnit::MicroHenry => "uH",
            PhysicalUnit::MilliHenry => "mH",
            PhysicalUnit::Henry => "H",
            // Voltage
            PhysicalUnit::MilliVolt => "mV",
            PhysicalUnit::Volt => "V",
            PhysicalUnit::KiloVolt => "kV",
            // Current
            PhysicalUnit::MicroAmp => "uA",
            PhysicalUnit::MilliAmp => "mA",
            PhysicalUnit::Amp => "A",
            // Frequency
            PhysicalUnit::Hertz => "Hz",
            PhysicalUnit::KiloHertz => "kHz",
            PhysicalUnit::MegaHertz => "MHz",
            PhysicalUnit::GigaHertz => "GHz",
            // Power
            PhysicalUnit::MilliWatt => "mW",
            PhysicalUnit::Watt => "W",
        }
    }

    /// The multiplier to convert from this unit to its base unit.
    const fn multiplier(&self) -> f64 {
        match self {
            // Resistance (base: ohm)
            PhysicalUnit::Ohm => 1.0,
            PhysicalUnit::KiloOhm => 1e3,
            PhysicalUnit::MegaOhm => 1e6,
            // Capacitance (base: farad)
            PhysicalUnit::PicoFarad => 1e-12,
            PhysicalUnit::NanoFarad => 1e-9,
            PhysicalUnit::MicroFarad => 1e-6,
            PhysicalUnit::MilliFarad => 1e-3,
            // Inductance (base: henry)
            PhysicalUnit::NanoHenry => 1e-9,
            PhysicalUnit::MicroHenry => 1e-6,
            PhysicalUnit::MilliHenry => 1e-3,
            PhysicalUnit::Henry => 1.0,
            // Voltage (base: volt)
            PhysicalUnit::MilliVolt => 1e-3,
            PhysicalUnit::Volt => 1.0,
            PhysicalUnit::KiloVolt => 1e3,
            // Current (base: ampere)
            PhysicalUnit::MicroAmp => 1e-6,
            PhysicalUnit::MilliAmp => 1e-3,
            PhysicalUnit::Amp => 1.0,
            // Frequency (base: hertz)
            PhysicalUnit::Hertz => 1.0,
            PhysicalUnit::KiloHertz => 1e3,
            PhysicalUnit::MegaHertz => 1e6,
            PhysicalUnit::GigaHertz => 1e9,
            // Power (base: watt)
            PhysicalUnit::MilliWatt => 1e-3,
            PhysicalUnit::Watt => 1.0,
        }
    }
}

impl fmt::Display for PhysicalUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// Error type for physical unit parsing.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("unknown physical unit: '{0}' (expected: ohm, kohm, Mohm, pF, nF, uF, mF, nH, uH, mH, H, mV, V, kV, uA, mA, A, Hz, kHz, MHz, GHz, mW, W)")]
pub struct ParsePhysicalUnitError(pub String);

impl FromStr for PhysicalUnit {
    type Err = ParsePhysicalUnitError;

    /// Parse a physical unit from its DSL suffix string.
    ///
    /// This is case-sensitive to match the grammar exactly.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::physical_units::PhysicalUnit;
    ///
    /// assert_eq!("kohm".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::KiloOhm);
    /// assert_eq!("nF".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::NanoFarad);
    /// assert_eq!("V".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Volt);
    /// assert_eq!("MHz".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MegaHertz);
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // Resistance
            "ohm" => Ok(PhysicalUnit::Ohm),
            "kohm" => Ok(PhysicalUnit::KiloOhm),
            "Mohm" => Ok(PhysicalUnit::MegaOhm),
            // Capacitance
            "pF" => Ok(PhysicalUnit::PicoFarad),
            "nF" => Ok(PhysicalUnit::NanoFarad),
            "uF" => Ok(PhysicalUnit::MicroFarad),
            "mF" => Ok(PhysicalUnit::MilliFarad),
            // Inductance
            "nH" => Ok(PhysicalUnit::NanoHenry),
            "uH" => Ok(PhysicalUnit::MicroHenry),
            "mH" => Ok(PhysicalUnit::MilliHenry),
            "H" => Ok(PhysicalUnit::Henry),
            // Voltage
            "mV" => Ok(PhysicalUnit::MilliVolt),
            "V" => Ok(PhysicalUnit::Volt),
            "kV" => Ok(PhysicalUnit::KiloVolt),
            // Current
            "uA" => Ok(PhysicalUnit::MicroAmp),
            "mA" => Ok(PhysicalUnit::MilliAmp),
            "A" => Ok(PhysicalUnit::Amp),
            // Frequency
            "Hz" => Ok(PhysicalUnit::Hertz),
            "kHz" => Ok(PhysicalUnit::KiloHertz),
            "MHz" => Ok(PhysicalUnit::MegaHertz),
            "GHz" => Ok(PhysicalUnit::GigaHertz),
            // Power
            "mW" => Ok(PhysicalUnit::MilliWatt),
            "W" => Ok(PhysicalUnit::Watt),
            _ => Err(ParsePhysicalUnitError(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== FromStr — every suffix =====

    #[test]
    fn test_from_str_resistance() {
        assert_eq!("ohm".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Ohm);
        assert_eq!("kohm".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::KiloOhm);
        assert_eq!("Mohm".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MegaOhm);
    }

    #[test]
    fn test_from_str_capacitance() {
        assert_eq!("pF".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::PicoFarad);
        assert_eq!("nF".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::NanoFarad);
        assert_eq!("uF".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MicroFarad);
        assert_eq!("mF".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MilliFarad);
    }

    #[test]
    fn test_from_str_inductance() {
        assert_eq!("nH".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::NanoHenry);
        assert_eq!("uH".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MicroHenry);
        assert_eq!("mH".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MilliHenry);
        assert_eq!("H".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Henry);
    }

    #[test]
    fn test_from_str_voltage() {
        assert_eq!("mV".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MilliVolt);
        assert_eq!("V".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Volt);
        assert_eq!("kV".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::KiloVolt);
    }

    #[test]
    fn test_from_str_current() {
        assert_eq!("uA".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MicroAmp);
        assert_eq!("mA".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MilliAmp);
        assert_eq!("A".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Amp);
    }

    #[test]
    fn test_from_str_frequency() {
        assert_eq!("Hz".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Hertz);
        assert_eq!("kHz".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::KiloHertz);
        assert_eq!("MHz".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MegaHertz);
        assert_eq!("GHz".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::GigaHertz);
    }

    #[test]
    fn test_from_str_power() {
        assert_eq!("mW".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::MilliWatt);
        assert_eq!("W".parse::<PhysicalUnit>().unwrap(), PhysicalUnit::Watt);
    }

    #[test]
    fn test_from_str_invalid() {
        assert!("unknown".parse::<PhysicalUnit>().is_err());
        assert!("kOhm".parse::<PhysicalUnit>().is_err()); // case-sensitive
        assert!("".parse::<PhysicalUnit>().is_err());
        assert!("mm".parse::<PhysicalUnit>().is_err()); // length unit, not physical
    }

    // ===== quantity() =====

    #[test]
    fn test_quantity() {
        assert_eq!(PhysicalUnit::Ohm.quantity(), PhysicalQuantity::Resistance);
        assert_eq!(PhysicalUnit::KiloOhm.quantity(), PhysicalQuantity::Resistance);
        assert_eq!(PhysicalUnit::MegaOhm.quantity(), PhysicalQuantity::Resistance);

        assert_eq!(PhysicalUnit::PicoFarad.quantity(), PhysicalQuantity::Capacitance);
        assert_eq!(PhysicalUnit::NanoFarad.quantity(), PhysicalQuantity::Capacitance);
        assert_eq!(PhysicalUnit::MicroFarad.quantity(), PhysicalQuantity::Capacitance);
        assert_eq!(PhysicalUnit::MilliFarad.quantity(), PhysicalQuantity::Capacitance);

        assert_eq!(PhysicalUnit::NanoHenry.quantity(), PhysicalQuantity::Inductance);
        assert_eq!(PhysicalUnit::MicroHenry.quantity(), PhysicalQuantity::Inductance);
        assert_eq!(PhysicalUnit::MilliHenry.quantity(), PhysicalQuantity::Inductance);
        assert_eq!(PhysicalUnit::Henry.quantity(), PhysicalQuantity::Inductance);

        assert_eq!(PhysicalUnit::MilliVolt.quantity(), PhysicalQuantity::Voltage);
        assert_eq!(PhysicalUnit::Volt.quantity(), PhysicalQuantity::Voltage);
        assert_eq!(PhysicalUnit::KiloVolt.quantity(), PhysicalQuantity::Voltage);

        assert_eq!(PhysicalUnit::MicroAmp.quantity(), PhysicalQuantity::Current);
        assert_eq!(PhysicalUnit::MilliAmp.quantity(), PhysicalQuantity::Current);
        assert_eq!(PhysicalUnit::Amp.quantity(), PhysicalQuantity::Current);

        assert_eq!(PhysicalUnit::Hertz.quantity(), PhysicalQuantity::Frequency);
        assert_eq!(PhysicalUnit::KiloHertz.quantity(), PhysicalQuantity::Frequency);
        assert_eq!(PhysicalUnit::MegaHertz.quantity(), PhysicalQuantity::Frequency);
        assert_eq!(PhysicalUnit::GigaHertz.quantity(), PhysicalQuantity::Frequency);

        assert_eq!(PhysicalUnit::MilliWatt.quantity(), PhysicalQuantity::Power);
        assert_eq!(PhysicalUnit::Watt.quantity(), PhysicalQuantity::Power);
    }

    // ===== to_base_f64 =====

    #[test]
    fn test_to_base_resistance() {
        assert_eq!(PhysicalUnit::Ohm.to_base_f64(100.0), 100.0);
        assert_eq!(PhysicalUnit::KiloOhm.to_base_f64(10.0), 10_000.0);
        assert_eq!(PhysicalUnit::MegaOhm.to_base_f64(1.0), 1_000_000.0);
    }

    #[test]
    fn test_to_base_capacitance() {
        assert!((PhysicalUnit::PicoFarad.to_base_f64(100.0) - 1e-10).abs() < 1e-22);
        assert!((PhysicalUnit::NanoFarad.to_base_f64(100.0) - 1e-7).abs() < 1e-19);
        assert!((PhysicalUnit::MicroFarad.to_base_f64(10.0) - 1e-5).abs() < 1e-17);
        assert!((PhysicalUnit::MilliFarad.to_base_f64(1.0) - 1e-3).abs() < 1e-15);
    }

    #[test]
    fn test_to_base_inductance() {
        assert!((PhysicalUnit::NanoHenry.to_base_f64(100.0) - 1e-7).abs() < 1e-19);
        assert!((PhysicalUnit::MicroHenry.to_base_f64(10.0) - 1e-5).abs() < 1e-17);
        assert!((PhysicalUnit::MilliHenry.to_base_f64(1.0) - 1e-3).abs() < 1e-15);
        assert_eq!(PhysicalUnit::Henry.to_base_f64(2.0), 2.0);
    }

    #[test]
    fn test_to_base_voltage() {
        assert!((PhysicalUnit::MilliVolt.to_base_f64(3300.0) - 3.3).abs() < 1e-12);
        assert_eq!(PhysicalUnit::Volt.to_base_f64(3.3), 3.3);
        assert_eq!(PhysicalUnit::KiloVolt.to_base_f64(1.0), 1000.0);
    }

    #[test]
    fn test_to_base_current() {
        assert!((PhysicalUnit::MicroAmp.to_base_f64(100.0) - 1e-4).abs() < 1e-16);
        assert!((PhysicalUnit::MilliAmp.to_base_f64(500.0) - 0.5).abs() < 1e-12);
        assert_eq!(PhysicalUnit::Amp.to_base_f64(2.0), 2.0);
    }

    #[test]
    fn test_to_base_frequency() {
        assert_eq!(PhysicalUnit::Hertz.to_base_f64(1000.0), 1000.0);
        assert_eq!(PhysicalUnit::KiloHertz.to_base_f64(1.0), 1000.0);
        assert_eq!(PhysicalUnit::MegaHertz.to_base_f64(2.4), 2_400_000.0);
        assert_eq!(PhysicalUnit::GigaHertz.to_base_f64(1.0), 1e9);
    }

    #[test]
    fn test_to_base_power() {
        assert!((PhysicalUnit::MilliWatt.to_base_f64(250.0) - 0.25).abs() < 1e-12);
        assert_eq!(PhysicalUnit::Watt.to_base_f64(5.0), 5.0);
    }

    // ===== from_base_f64 =====

    #[test]
    fn test_from_base_round_trip() {
        // For every unit, converting to base and back should give the original value
        let units = [
            PhysicalUnit::Ohm,
            PhysicalUnit::KiloOhm,
            PhysicalUnit::MegaOhm,
            PhysicalUnit::PicoFarad,
            PhysicalUnit::NanoFarad,
            PhysicalUnit::MicroFarad,
            PhysicalUnit::MilliFarad,
            PhysicalUnit::NanoHenry,
            PhysicalUnit::MicroHenry,
            PhysicalUnit::MilliHenry,
            PhysicalUnit::Henry,
            PhysicalUnit::MilliVolt,
            PhysicalUnit::Volt,
            PhysicalUnit::KiloVolt,
            PhysicalUnit::MicroAmp,
            PhysicalUnit::MilliAmp,
            PhysicalUnit::Amp,
            PhysicalUnit::Hertz,
            PhysicalUnit::KiloHertz,
            PhysicalUnit::MegaHertz,
            PhysicalUnit::GigaHertz,
            PhysicalUnit::MilliWatt,
            PhysicalUnit::Watt,
        ];

        for unit in units {
            let original = 42.5;
            let base = unit.to_base_f64(original);
            let back = unit.from_base_f64(base);
            assert!(
                (back - original).abs() < 1e-10,
                "Round-trip failed for {:?}: {} -> {} -> {}",
                unit,
                original,
                base,
                back,
            );
        }
    }

    // ===== Display round-trip =====

    #[test]
    fn test_display_round_trip() {
        let units = [
            PhysicalUnit::Ohm,
            PhysicalUnit::KiloOhm,
            PhysicalUnit::MegaOhm,
            PhysicalUnit::PicoFarad,
            PhysicalUnit::NanoFarad,
            PhysicalUnit::MicroFarad,
            PhysicalUnit::MilliFarad,
            PhysicalUnit::NanoHenry,
            PhysicalUnit::MicroHenry,
            PhysicalUnit::MilliHenry,
            PhysicalUnit::Henry,
            PhysicalUnit::MilliVolt,
            PhysicalUnit::Volt,
            PhysicalUnit::KiloVolt,
            PhysicalUnit::MicroAmp,
            PhysicalUnit::MilliAmp,
            PhysicalUnit::Amp,
            PhysicalUnit::Hertz,
            PhysicalUnit::KiloHertz,
            PhysicalUnit::MegaHertz,
            PhysicalUnit::GigaHertz,
            PhysicalUnit::MilliWatt,
            PhysicalUnit::Watt,
        ];

        for unit in units {
            let displayed = unit.to_string();
            let parsed: PhysicalUnit = displayed.parse().unwrap();
            assert_eq!(
                parsed, unit,
                "Display round-trip failed for {:?}: displayed as '{}', parsed back as {:?}",
                unit, displayed, parsed,
            );
        }
    }

    // ===== suffix() =====

    #[test]
    fn test_suffix_matches_grammar() {
        // These must match the grammar's physical_unit rule exactly
        assert_eq!(PhysicalUnit::Ohm.suffix(), "ohm");
        assert_eq!(PhysicalUnit::KiloOhm.suffix(), "kohm");
        assert_eq!(PhysicalUnit::MegaOhm.suffix(), "Mohm");
        assert_eq!(PhysicalUnit::PicoFarad.suffix(), "pF");
        assert_eq!(PhysicalUnit::NanoFarad.suffix(), "nF");
        assert_eq!(PhysicalUnit::MicroFarad.suffix(), "uF");
        assert_eq!(PhysicalUnit::MilliFarad.suffix(), "mF");
        assert_eq!(PhysicalUnit::NanoHenry.suffix(), "nH");
        assert_eq!(PhysicalUnit::MicroHenry.suffix(), "uH");
        assert_eq!(PhysicalUnit::MilliHenry.suffix(), "mH");
        assert_eq!(PhysicalUnit::Henry.suffix(), "H");
        assert_eq!(PhysicalUnit::MilliVolt.suffix(), "mV");
        assert_eq!(PhysicalUnit::Volt.suffix(), "V");
        assert_eq!(PhysicalUnit::KiloVolt.suffix(), "kV");
        assert_eq!(PhysicalUnit::MicroAmp.suffix(), "uA");
        assert_eq!(PhysicalUnit::MilliAmp.suffix(), "mA");
        assert_eq!(PhysicalUnit::Amp.suffix(), "A");
        assert_eq!(PhysicalUnit::Hertz.suffix(), "Hz");
        assert_eq!(PhysicalUnit::KiloHertz.suffix(), "kHz");
        assert_eq!(PhysicalUnit::MegaHertz.suffix(), "MHz");
        assert_eq!(PhysicalUnit::GigaHertz.suffix(), "GHz");
        assert_eq!(PhysicalUnit::MilliWatt.suffix(), "mW");
        assert_eq!(PhysicalUnit::Watt.suffix(), "W");
    }

    // ===== error message =====

    #[test]
    fn test_parse_error_message() {
        let err = "bogus".parse::<PhysicalUnit>().unwrap_err();
        assert_eq!(err.0, "bogus");
        let msg = err.to_string();
        assert!(msg.contains("bogus"));
        assert!(msg.contains("expected:"));
    }
}
