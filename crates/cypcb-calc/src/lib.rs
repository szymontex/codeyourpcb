//! CodeYourPCB Electrical Calculators
//!
//! Provides IPC-standard calculations for PCB design:
//! - Trace width from current (IPC-2221)
//! - Future: Impedance calculation (IPC-2141)
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

pub mod trace_width;

pub use trace_width::{TraceWidthCalculator, TraceWidthParams, TraceWidthResult, TraceWidthWarning};
