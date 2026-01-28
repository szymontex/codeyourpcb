//! PCB export functionality for manufacturing file generation.
//!
//! This crate provides export capabilities for PCB designs, including:
//! - Coordinate conversion from internal nanometers to Gerber/Excellon decimal format
//! - Aperture management for Gerber D-code generation
//! - Support for all standard pad shapes (circle, rectangle, oblong, rounded rectangle)
//!
//! # Examples
//!
//! ```
//! use cypcb_export::coords::{CoordinateFormat, nm_to_gerber};
//! use cypcb_core::Nm;
//!
//! let format = CoordinateFormat::FORMAT_MM_2_6;
//! let gerber_str = nm_to_gerber(Nm::from_mm(1.0).0, &format);
//! assert_eq!(gerber_str, "1.000000");
//! ```

pub mod coords;
pub mod apertures;

// Re-export commonly used types
pub use coords::{CoordinateFormat, Unit, nm_to_gerber, gerber_format_string};
pub use apertures::{ApertureManager, ApertureShape, aperture_for_pad};
