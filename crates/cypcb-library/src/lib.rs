//! A local component library: SQLite, a schema, a search, and importers.
//!
//! **`cypcb library` is what calls this**, and the day it did is the day this
//! crate stopped being 3751 lines nothing reached. `import` walks a directory
//! for `<name>.pretty` folders and indexes the `.kicad_mod` files in them;
//! `search` finds one again by name, description, package or manufacturer;
//! `list` says what is indexed. The index is a file the command writes where
//! it was run.
//!
//! Two things the first caller found, both now measured rather than assumed:
//!
//! - The search handed what a person typed straight to FTS5, so `SOT-23-5` was
//!   read as a query language rather than a phrase and
//!   `nothing-is-called-this` failed with `no such column: is`. A plain query
//!   is a quoted phrase now; `field:value` and a trailing `*` still mean what
//!   they mean.
//! - The importer read footprints with a generic S-expression reader that
//!   refused every file carrying `(tedit 5E1BAA69)` - a hexadecimal timestamp
//!   it took for a malformed float. It reads them through `cypcb-kicad` now,
//!   the same reader the rest of this project uses.
//!
//! What it still cannot do, held by a test that names the message: a KiCad 6
//! footprint - `(footprint ...)` with `(version ...)` beside it - is refused
//! with `unknown element in module: version`. The head is renamed on the way
//! in; the fields are a gap in `cypcb-kicad` rather than here.

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
