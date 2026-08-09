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
//! // No decimal point: `%FSLAX26Y26*%` already said where it goes.
//! let gerber_str = nm_to_gerber(Nm::from_mm(1.0).0, &format);
//! assert_eq!(gerber_str, "1000000");
//! ```

pub mod apertures;
pub mod bom;
pub mod coords;
pub mod cpl;
pub mod excellon;
pub mod gerber;
pub mod job;
pub mod jobfile;
/// The geometry a copper pour takes, re-exported from cypcb-core where both
/// the exporter and the renderer can reach it.
pub use cypcb_core::pour;

pub mod presets;

// Re-export commonly used types
pub use apertures::{aperture_for_pad, ApertureManager, ApertureShape};
pub use bom::{group_components, BomEntry};
pub use coords::{gerber_format_string, nm_to_gerber, CoordinateFormat, Unit};
pub use cpl::{CplConfig, CplEntry};
pub use excellon::{export_excellon, DrillType, ToolTable};
pub use gerber::{export_copper_layer, write_header, GerberFileFunction};
pub use job::{inner_layer_suffix, run_export, ExportError, ExportJob, ExportResult, ExportedFile};
pub use jobfile::build_job_file;
pub use presets::{ExportLayers, ExportPreset, FileNaming};
