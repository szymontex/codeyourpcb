pub mod error;
pub mod models;
pub mod schema;

// Re-export key types for convenience
pub use error::LibraryError;
pub use models::{Component, ComponentId, ComponentMetadata, LibraryInfo, SearchFilters, SearchResult};
