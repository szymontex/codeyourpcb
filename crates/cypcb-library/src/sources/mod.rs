use crate::{Component, LibraryError, LibraryInfo};

/// Trait for library source implementations (KiCad, JLCPCB, custom, etc.)
///
/// Note: Uses blocking I/O since this runs in spawn_blocking contexts.
/// All sources should be designed to avoid holding locks during I/O.
pub trait LibrarySource {
    /// Returns the source identifier (e.g., "kicad", "jlcpcb")
    fn source_name(&self) -> &str;

    /// Lists all available libraries from this source
    fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError>;

    /// Imports all components from a specific library by name
    fn import_library(&self, name: &str) -> Result<Vec<Component>, LibraryError>;
}

pub mod custom;
#[cfg(feature = "jlcpcb")]
pub mod jlcpcb;
pub mod kicad;
