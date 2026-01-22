//! KiCad library directory scanning.
//!
//! Scans .pretty directories for .kicad_mod footprint files.

use std::path::{Path, PathBuf};

/// An entry in a KiCad footprint library.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    /// Footprint name (filename without extension).
    pub name: String,
    /// Full path to the .kicad_mod file.
    pub path: PathBuf,
    /// Parent library name (the .pretty folder name).
    pub library: String,
}

/// Scan a KiCad library directory for footprint files.
///
/// # Arguments
///
/// * `path` - Path to a .pretty directory
///
/// # Returns
///
/// List of footprint entries found in the directory.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn scan_library(path: &Path) -> Result<Vec<LibraryEntry>, std::io::Error> {
    // Placeholder - will be implemented in Task 3
    let _ = path;
    Ok(vec![])
}

/// Scan multiple KiCad library directories for footprint files.
///
/// # Arguments
///
/// * `paths` - Paths to .pretty directories
///
/// # Returns
///
/// Combined list of footprint entries from all directories.
///
/// # Errors
///
/// Returns an error if any directory cannot be read.
pub fn scan_libraries(paths: &[&Path]) -> Result<Vec<LibraryEntry>, std::io::Error> {
    let mut entries = Vec::new();
    for path in paths {
        entries.extend(scan_library(path)?);
    }
    Ok(entries)
}
