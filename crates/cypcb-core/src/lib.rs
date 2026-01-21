//! CodeYourPCB Core
//!
//! Shared types, coordinates, and geometry primitives for the CodeYourPCB system.
//! All coordinates use integer nanometers (i64) for deterministic precision.
//!
//! # Why Integer Nanometers?
//!
//! PCB tools commonly face issues with floating-point accumulation errors.
//! Small precision errors compound across complex routing operations, leading to:
//! - Traces that should connect but don't (off by sub-micron amounts)
//! - Non-deterministic DRC results
//! - Different output from the same input file
//!
//! By using i64 nanometers internally:
//! - All coordinate arithmetic is exact
//! - Same input always produces identical output
//! - Comparisons and hashing are reliable
//! - Range is sufficient (i64 max = ~9.2 billion meters)
//!
//! # Coordinate System
//!
//! - Origin: bottom-left of the board
//! - X-axis: positive right
//! - Y-axis: positive up (mathematical convention)
//!
//! This matches standard mathematical conventions and most Gerber viewers.
//!
//! # Unit Conversions
//!
//! | Unit | Nanometers |
//! |------|------------|
//! | 1 mm | 1,000,000 |
//! | 1 mil | 25,400 |
//! | 1 inch | 25,400,000 |
//!
//! # Modules
//!
//! - [`coords`]: Core coordinate types ([`Nm`], [`Point`])
//! - [`units`]: Unit parsing and conversion ([`Unit`])
//! - [`geometry`]: Geometric primitives ([`Rect`])

pub mod coords;
pub mod geometry;
pub mod units;

// Re-export primary types at crate root for convenience
pub use coords::{Nm, Point, NM_PER_INCH, NM_PER_MIL, NM_PER_MM};
pub use geometry::Rect;
pub use units::{Dimension, ParseUnitError, Unit};
