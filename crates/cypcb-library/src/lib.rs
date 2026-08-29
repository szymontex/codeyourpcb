//! A local component library: SQLite, a schema, a search, and importers.
//!
//! **Nothing in this workspace calls this crate, and that is a decision rather
//! than an oversight.** `no_crate_is_written_and_never_called` has listed it
//! since the census was written, and the question it left open - is this work
//! that has not landed yet, or work nothing will ever reach again - is
//! answered here: it is the first, and it is kept.
//!
//! What is here, measured on 2026-08-29: **3751 lines and 41 passing tests**
//! (`cargo test -p cypcb-library`), a schema and a manager over `rusqlite`, a
//! search by field, metadata and preview, and importers under `sources` - the
//! KiCad one reads `.pretty` folders and `.kicad_mod` files, which is the
//! format every footprint this project already parses comes in.
//!
//! Why it is kept rather than deleted: the parts a component library needs are
//! written and tested, and the tool has the other half - `cypcb-kicad` reads
//! footprints, the viewer has a search panel over JLCPCB's catalogue. What is
//! missing is one path between them, not a body of work.
//!
//! What would make it live, in one sentence: a `cypcb library` subcommand that
//! imports a `.pretty` folder and searches what it imported, which is the
//! smallest thing that would give the crate a caller and a user at once.
//!
//! Until then it builds and its tests run in the gate, which is what keeps it
//! from rotting while it waits.

pub mod error;
pub mod manager;
pub mod metadata;
pub mod models;
pub mod preview;
pub mod schema;
pub mod search;
pub mod sources;

// Re-export key types for convenience
pub use error::LibraryError;
pub use manager::LibraryManager;
pub use models::{
    Component, ComponentId, ComponentMetadata, LibraryInfo, SearchFilters, SearchResult,
};
pub use sources::LibrarySource;
