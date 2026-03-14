//! KiCad File Import
//!
//! Import KiCad footprint files (.kicad_mod) and PCB files (.kicad_pcb)
//! for use in CodeYourPCB.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cypcb_kicad::{import_footprint, scan_library, parse_kicad_pcb};
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
//!
//! // Parse a KiCad PCB file
//! let result = parse_kicad_pcb(Path::new("board.kicad_pcb"))?;
//! println!("Components: {}", result.metadata.component_count);
//! ```

pub mod footprint;
pub mod library;
pub mod pcb_parser;

pub use footprint::{import_footprint, import_footprint_from_str, KicadImportError};
pub use library::{find_by_library, find_by_name, scan_libraries, scan_library, LibraryEntry};
pub use pcb_parser::{
    parse_kicad_pcb, parse_kicad_pcb_str, BenchmarkComplexity, KicadBenchmark, KicadPcbError,
    KicadPcbMetadata, KicadPcbParseResult, BENCHMARKS,
};
