//! Gerber file export for PCB manufacturing.
//!
//! This module handles export of Gerber files (RS-274X format with X2 extensions)
//! for PCB fabrication. Gerber is the industry-standard format for communicating
//! PCB layer data to manufacturers.
//!
//! # Module Structure
//!
//! - [`header`] - X2 header generation with file attributes
//! - [`copper`] - Copper layer export (pads, traces, vias)
//! - [`mask`] - Soldermask and solderpaste layer export
//! - [`outline`] - Board outline/profile export
//! - [`silk`] - Silkscreen layer export

pub mod header;
pub mod copper;
pub mod mask;
pub mod outline;
pub mod silk;

// Re-export key types for convenience
pub use header::{write_header, GerberFileFunction, CopperSide, Side};
pub use copper::export_copper_layer;
pub use mask::{export_soldermask, export_solderpaste, MaskPasteConfig};
pub use outline::export_outline;
pub use silk::{export_silkscreen, SilkConfig};
