//! CodeYourPCB Language Server
//!
//! LSP implementation for the CodeYourPCB DSL providing:
//! - Hover information for components, nets, footprints, and pins
//! - Diagnostics from DRC rule violations and parse errors
//! - Document synchronization with incremental updates
//!
//! # Architecture
//!
//! The server uses tower-lsp for protocol handling and integrates with:
//! - cypcb-parser for syntax analysis and AST generation
//! - cypcb-world for board model construction
//! - cypcb-drc for design rule checking
//!
//! # Usage
//!
//! Start the server via the binary:
//! ```bash
//! cypcb-lsp --stdio
//! ```
//!
//! Or use the library directly:
//! ```rust,ignore
//! use cypcb_lsp::Backend;
//! use tower_lsp::{LspService, Server};
//!
//! let (service, socket) = LspService::new(|client| Backend::new(client));
//! ```

// Module declarations
#[cfg(feature = "server")]
pub mod backend;

pub mod completion;
pub mod diagnostics;
pub mod document;
pub mod hover;

// Re-exports
#[cfg(feature = "server")]
pub use backend::Backend;

pub use completion::{completion_at_position, CompletionItem, CompletionItemKind};
pub use diagnostics::run_diagnostics;
pub use document::DocumentState;
pub use hover::hover_at_position;
