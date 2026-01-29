pub mod error;
pub mod manager;
pub mod metadata;
pub mod models;
pub mod schema;
pub mod search;
pub mod sources;

// Re-export key types for convenience
pub use error::LibraryError;
pub use manager::LibraryManager;
pub use models::{Component, ComponentId, ComponentMetadata, LibraryInfo, SearchFilters, SearchResult};
pub use sources::LibrarySource;
