//! Footprint definitions for PCB components.
//!
//! This module provides the footprint library system, including:
//! - [`Footprint`]: Complete footprint definition with pads and bounds
//! - [`PadDef`]: Individual pad definition within a footprint
//! - [`FootprintLibrary`]: Registry of available footprints
//!
//! # Built-in Footprints
//!
//! The library comes pre-loaded with common footprints:
//!
//! ## SMD Packages
//! - 0402, 0603, 0805, 1206, 2512 chip resistors/capacitors
//!
//! ## Through-hole Packages
//! - Axial 300mil (resistors, diodes)
//! - DIP-8 IC package
//! - Pin header 1x2
//!
//! # Example
//!
//! ```
//! use cypcb_world::footprint::FootprintLibrary;
//!
//! let lib = FootprintLibrary::new();
//!
//! // Look up a chip resistor footprint
//! let fp = lib.get("0402").expect("0402 should exist");
//! assert_eq!(fp.pads.len(), 2);
//!
//! // All footprints are available
//! assert!(lib.get("0603").is_some());
//! assert!(lib.get("DIP-8").is_some());
//! ```

mod library;
mod smd;
mod tht;

pub use library::{Footprint, FootprintLibrary, PadDef};
