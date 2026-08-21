//! CodeYourPCB Electrical Calculators
//!
//! Provides IPC-standard calculations for PCB design:
//! - Trace width from current (IPC-2221)
//! - Impedance for a microstrip and a symmetric stripline (IPC-2141)
//!
//! # Example
//!
//! ```
//! use cypcb_calc::{TraceWidthCalculator, TraceWidthParams};
//!
//! // Calculate minimum trace width for 1A current
//! let params = TraceWidthParams::new(1.0);
//! let result = TraceWidthCalculator::calculate(&params);
//!
//! println!("Minimum width: {:.2}mm", result.width.to_mm());
//! println!("Cross-section: {:.4}mm²", result.cross_section_mm2);
//! ```

pub mod impedance;
pub mod trace_width;

pub use impedance::{microstrip_ohms_x100, stripline_ohms_x100};
pub use trace_width::{
    TraceWidthCalculator, TraceWidthParams, TraceWidthResult, TraceWidthWarning,
};
