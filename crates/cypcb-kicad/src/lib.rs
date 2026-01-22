//! KiCad File Import
//!
//! Import KiCad footprint files (.kicad_mod) for use in CodeYourPCB.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cypcb_kicad::{import_footprint, scan_library};
//! use std::path::Path;
//!
//! // Import single footprint
//! let fp = import_footprint(Path::new("Resistors_SMD.pretty/R_0402.kicad_mod"))?;
//!
//! // Scan entire library directory
//! let entries = scan_library(Path::new("Resistors_SMD.pretty"))?;
//! for entry in entries {
//!     println!("{}: {}", entry.name, entry.path.display());
//! }
//! ```

pub mod footprint;
pub mod library;

pub use footprint::{import_footprint, import_footprint_from_str, KicadImportError};
pub use library::{scan_library, scan_libraries, LibraryEntry};
