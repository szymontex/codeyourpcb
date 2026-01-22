//! Autorouting Integration
//!
//! Integrates with FreeRouting autorouter via DSN/SES file exchange.
//!
//! # Workflow
//!
//! 1. Export board to DSN format using [`export_dsn`]
//! 2. Run FreeRouting CLI (external process)
//! 3. Import routes from SES format using `import_ses` (Plan 05-06)
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_router::{export_dsn, import_ses};
//! use std::fs::File;
//!
//! // Export board to DSN
//! let mut dsn_file = File::create("board.dsn")?;
//! export_dsn(&world, &mut dsn_file)?;
//!
//! // Run FreeRouting (external)
//! // java -jar freerouting.jar -de board.dsn -do board.ses
//!
//! // Import routing results
//! let routes = import_ses("board.ses")?;
//! ```
//!
//! # DSN Format
//!
//! The Specctra DSN (Design) format is the standard input format for
//! FreeRouting and other autorouters. It contains:
//!
//! - Board boundary and layer definitions
//! - Component placements
//! - Footprint library (padstacks)
//! - Network (nets and pin connections)
//! - Design rules (clearances, widths)
//! - Existing wiring (locked traces)
//!
//! # SES Format
//!
//! The Specctra SES (Session) format contains routing results:
//!
//! - Routed wire paths
//! - Via placements
//! - Routing statistics

pub mod dsn;
pub mod freerouting;
pub mod ses;
pub mod types;

pub use dsn::{export_dsn, DsnExportError};
pub use freerouting::{FreeRoutingRunner, RoutingConfig, RoutingError, RoutingProgress};
pub use ses::{import_ses, import_ses_from_str, SesImportError};
pub use types::{RouteSegment, RoutingResult, RoutingStatus, ViaPlacement};
