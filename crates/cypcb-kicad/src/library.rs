//! KiCad library directory scanning.
//!
//! Scans .pretty directories for .kicad_mod footprint files.
//!
//! # KiCad Library Structure
//!
//! KiCad organizes footprints in `.pretty` folders:
//!
//! ```text
//! Package_SO.pretty/
//!   SOIC-8_3.9x4.9mm_P1.27mm.kicad_mod
//!   SOIC-14_3.9x8.7mm_P1.27mm.kicad_mod
//!   ...
//! Resistor_SMD.pretty/
//!   R_0402_1005Metric.kicad_mod
//!   R_0603_1608Metric.kicad_mod
//!   ...
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_kicad::scan_library;
//! use std::path::Path;
//!
//! let entries = scan_library(Path::new("Resistor_SMD.pretty"))?;
//! for entry in entries {
//!     println!("{}: {} (from {})", entry.name, entry.path.display(), entry.library);
//! }
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// An entry in a KiCad footprint library.
///
/// Represents a single .kicad_mod file discovered during library scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    /// Footprint name (filename without extension).
    pub name: String,
    /// Full path to the .kicad_mod file.
    pub path: PathBuf,
    /// Parent library name (the .pretty folder name, without extension).
    pub library: String,
}

impl LibraryEntry {
    /// Create a new library entry.
    pub fn new(name: String, path: PathBuf, library: String) -> Self {
        LibraryEntry {
            name,
            path,
            library,
        }
    }
}

/// Scan a KiCad library directory for footprint files.
///
/// Recursively walks the directory to find all .kicad_mod files.
/// For each file, extracts:
/// - `name`: Filename without extension
/// - `path`: Full absolute path to the file
/// - `library`: Parent .pretty folder name (without extension)
///
/// # Arguments
///
/// * `path` - Path to a directory (typically a .pretty folder)
///
/// # Returns
///
/// List of footprint entries found in the directory.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
///
/// # Example
///
/// ```rust,ignore
/// use cypcb_kicad::scan_library;
/// use std::path::Path;
///
/// let entries = scan_library(Path::new("/usr/share/kicad/footprints/Package_SO.pretty"))?;
/// assert!(!entries.is_empty());
/// ```
pub fn scan_library(path: &Path) -> Result<Vec<LibraryEntry>, std::io::Error> {
    let mut entries = Vec::new();

    // Get the library name from the path
    let default_library = extract_library_name(path);

    for entry in WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path();

        // Check if it's a .kicad_mod file
        if file_path.extension() == Some(OsStr::new("kicad_mod")) {
            // Extract footprint name from filename
            let name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            // Find the .pretty parent directory for library name
            let library = find_pretty_parent(file_path)
                .unwrap_or_else(|| default_library.clone());

            entries.push(LibraryEntry::new(
                name,
                file_path.to_path_buf(),
                library,
            ));
        }
    }

    Ok(entries)
}

/// Scan multiple KiCad library directories for footprint files.
///
/// Combines results from multiple directories. If the same footprint name
/// exists in multiple libraries, all entries are kept (distinguished by
/// their `library` field).
///
/// # Arguments
///
/// * `paths` - Paths to directories to scan
///
/// # Returns
///
/// Combined list of footprint entries from all directories.
///
/// # Errors
///
/// Returns an error if any directory cannot be read.
///
/// # Example
///
/// ```rust,ignore
/// use cypcb_kicad::scan_libraries;
/// use std::path::Path;
///
/// let entries = scan_libraries(&[
///     Path::new("Package_SO.pretty"),
///     Path::new("Resistor_SMD.pretty"),
/// ])?;
/// ```
pub fn scan_libraries(paths: &[&Path]) -> Result<Vec<LibraryEntry>, std::io::Error> {
    let mut entries = Vec::new();
    for path in paths {
        entries.extend(scan_library(path)?);
    }
    Ok(entries)
}

/// Extract the library name from a path.
///
/// If the path ends with `.pretty`, removes that extension.
/// Otherwise uses the last component of the path.
fn extract_library_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Remove .pretty extension if present
    if let Some(stripped) = name.strip_suffix(".pretty") {
        stripped.to_string()
    } else {
        name.to_string()
    }
}

/// Find the .pretty parent directory and extract its name.
fn find_pretty_parent(path: &Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if let Some(name) = ancestor.file_name() {
            if let Some(name_str) = name.to_str() {
                if name_str.ends_with(".pretty") {
                    return Some(name_str.strip_suffix(".pretty").unwrap_or(name_str).to_string());
                }
            }
        }
    }
    None
}

/// Find all footprint entries matching a name pattern.
///
/// Simple case-insensitive substring matching.
pub fn find_by_name<'a>(entries: &'a [LibraryEntry], pattern: &str) -> Vec<&'a LibraryEntry> {
    let pattern_lower = pattern.to_lowercase();
    entries
        .iter()
        .filter(|e| e.name.to_lowercase().contains(&pattern_lower))
        .collect()
}

/// Find all footprint entries from a specific library.
pub fn find_by_library<'a>(entries: &'a [LibraryEntry], library: &str) -> Vec<&'a LibraryEntry> {
    entries
        .iter()
        .filter(|e| e.library == library)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    /// Create a temporary directory structure for testing
    fn create_test_library(base: &Path, name: &str, footprints: &[&str]) -> PathBuf {
        let lib_path = base.join(format!("{}.pretty", name));
        fs::create_dir_all(&lib_path).unwrap();

        for fp_name in footprints {
            let fp_path = lib_path.join(format!("{}.kicad_mod", fp_name));
            let mut file = File::create(&fp_path).unwrap();
            // Write minimal valid footprint content
            writeln!(file, "(module {} (layer F.Cu))", fp_name).unwrap();
        }

        lib_path
    }

    #[test]
    fn test_scan_empty_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lib_path = temp_dir.path().join("Empty.pretty");
        fs::create_dir_all(&lib_path).unwrap();

        let entries = scan_library(&lib_path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scan_library_finds_footprints() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lib_path = create_test_library(
            temp_dir.path(),
            "Resistor_SMD",
            &["R_0402", "R_0603", "R_0805"],
        );

        let entries = scan_library(&lib_path).unwrap();
        assert_eq!(entries.len(), 3);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"R_0402"));
        assert!(names.contains(&"R_0603"));
        assert!(names.contains(&"R_0805"));
    }

    #[test]
    fn test_library_name_extraction() {
        let temp_dir = tempfile::tempdir().unwrap();
        let lib_path = create_test_library(
            temp_dir.path(),
            "Package_SO",
            &["SOIC-8"],
        );

        let entries = scan_library(&lib_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].library, "Package_SO");
    }

    #[test]
    fn test_scan_multiple_libraries() {
        let temp_dir = tempfile::tempdir().unwrap();

        let lib1 = create_test_library(temp_dir.path(), "Lib1", &["FP1", "FP2"]);
        let lib2 = create_test_library(temp_dir.path(), "Lib2", &["FP3", "FP4"]);

        let entries = scan_libraries(&[lib1.as_path(), lib2.as_path()]).unwrap();
        assert_eq!(entries.len(), 4);

        // Check library attribution
        let lib1_entries: Vec<_> = entries.iter().filter(|e| e.library == "Lib1").collect();
        let lib2_entries: Vec<_> = entries.iter().filter(|e| e.library == "Lib2").collect();

        assert_eq!(lib1_entries.len(), 2);
        assert_eq!(lib2_entries.len(), 2);
    }

    #[test]
    fn test_duplicate_names_in_different_libraries() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Same footprint name in two libraries
        let lib1 = create_test_library(temp_dir.path(), "Lib1", &["SOIC-8"]);
        let lib2 = create_test_library(temp_dir.path(), "Lib2", &["SOIC-8"]);

        let entries = scan_libraries(&[lib1.as_path(), lib2.as_path()]).unwrap();

        // Both should be present
        assert_eq!(entries.len(), 2);
        assert_eq!(entries.iter().filter(|e| e.name == "SOIC-8").count(), 2);

        // Can distinguish by library
        let from_lib1 = entries.iter().find(|e| e.library == "Lib1").unwrap();
        let from_lib2 = entries.iter().find(|e| e.library == "Lib2").unwrap();

        assert_eq!(from_lib1.name, "SOIC-8");
        assert_eq!(from_lib2.name, "SOIC-8");
        assert_ne!(from_lib1.path, from_lib2.path);
    }

    #[test]
    fn test_find_by_name() {
        let entries = vec![
            LibraryEntry::new("R_0402".into(), PathBuf::from("/a/R_0402.kicad_mod"), "Resistor_SMD".into()),
            LibraryEntry::new("R_0603".into(), PathBuf::from("/a/R_0603.kicad_mod"), "Resistor_SMD".into()),
            LibraryEntry::new("SOIC-8".into(), PathBuf::from("/b/SOIC-8.kicad_mod"), "Package_SO".into()),
        ];

        let resistors = find_by_name(&entries, "r_0");
        assert_eq!(resistors.len(), 2);

        let soic = find_by_name(&entries, "SOIC");
        assert_eq!(soic.len(), 1);
        assert_eq!(soic[0].name, "SOIC-8");
    }

    #[test]
    fn test_find_by_library() {
        let entries = vec![
            LibraryEntry::new("R_0402".into(), PathBuf::from("/a/R_0402.kicad_mod"), "Resistor_SMD".into()),
            LibraryEntry::new("R_0603".into(), PathBuf::from("/a/R_0603.kicad_mod"), "Resistor_SMD".into()),
            LibraryEntry::new("SOIC-8".into(), PathBuf::from("/b/SOIC-8.kicad_mod"), "Package_SO".into()),
        ];

        let resistors = find_by_library(&entries, "Resistor_SMD");
        assert_eq!(resistors.len(), 2);

        let packages = find_by_library(&entries, "Package_SO");
        assert_eq!(packages.len(), 1);
    }

    #[test]
    fn test_extract_library_name() {
        assert_eq!(
            extract_library_name(Path::new("Package_SO.pretty")),
            "Package_SO"
        );
        assert_eq!(
            extract_library_name(Path::new("/path/to/Resistor_SMD.pretty")),
            "Resistor_SMD"
        );
        assert_eq!(
            extract_library_name(Path::new("/path/to/my_library")),
            "my_library"
        );
    }

    #[test]
    fn test_nested_pretty_directories() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create nested structure: base/Package_SO.pretty/subfolder/FP.kicad_mod
        let lib_path = temp_dir.path().join("Package_SO.pretty");
        let sub_path = lib_path.join("subfolder");
        fs::create_dir_all(&sub_path).unwrap();

        let fp_path = sub_path.join("SOIC-8.kicad_mod");
        let mut file = File::create(&fp_path).unwrap();
        writeln!(file, "(module SOIC-8 (layer F.Cu))").unwrap();

        let entries = scan_library(&lib_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "SOIC-8");
        assert_eq!(entries[0].library, "Package_SO");
    }
}
