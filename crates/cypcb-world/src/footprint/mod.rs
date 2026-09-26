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
//! ## Gull-wing IC Packages
//! - SOIC-8, SOIC-14, SOT-23, SOT-23-5, TQFP-32
//!
//! # Orientation
//!
//! One convention, for the board and for every footprint on it, stated here
//! and nowhere else:
//!
//! - **X grows to the right and Y grows up, seen from the top of the board.**
//!   A footprint's pad positions are in the same frame, relative to the
//!   part's origin, so a pad at +Y is nearer the top edge of the board when
//!   the part is not rotated.
//! - **A rotation is counter-clockwise, seen from the top.**
//!   [`place_pad`](crate::components::place_pad) is the one place it is
//!   applied.
//! - **Pin 1 sits at the top left**, the zero orientation of IPC-7351 as the
//!   KiCad library conventions state it in rule F4.2: a part whose pins are
//!   all in one line has pin 1 on the left, and the pins of an IC count
//!   counter-clockwise from pin 1. The built-in footprints are checked
//!   against this, pin by pin, by the test
//!   `every_builtin_footprint_counts_its_pins_counter_clockwise`.
//!
//! What each output does with Y, so that a file and the board agree:
//!
//! - Gerber, drill, DXF, PDF, the pick-and-place list: Y up, written as is.
//! - SVG and the viewer's canvas: Y down, flipped once for the whole drawing.
//! - KiCad: Y down. `cypcb-kicad` converts at its boundary, in one module,
//!   for boards and for `.kicad_mod` footprints, in both directions.
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

pub mod gullwing;
mod library;
pub mod mounting;
mod smd;
mod tht;

pub use library::{
    base_name, bottom_name, mirrored_to_bottom, Footprint, FootprintLibrary, PadDef, PadOutline,
    SilkShape, IPC_COURTYARD_EXCESS,
};
