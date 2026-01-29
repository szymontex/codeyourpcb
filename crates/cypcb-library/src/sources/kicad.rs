use crate::{Component, ComponentId, ComponentMetadata, LibraryError, LibraryInfo};
use std::fs;
use std::path::{Path, PathBuf};

use super::LibrarySource;

/// KiCad library source for importing .pretty folders and .kicad_mod files
pub struct KiCadSource {
    search_paths: Vec<PathBuf>,
}

impl KiCadSource {
    /// Creates a new KiCad source with the given search paths
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self { search_paths }
    }

    /// Auto-organizes a dropped folder, detecting .pretty libraries
    ///
    /// If path is a .pretty folder, treats it as a single library.
    /// If path contains .pretty folders, treats each as a library.
    pub fn auto_organize_folder(path: &Path) -> Result<Vec<LibraryInfo>, LibraryError> {
        let mut libraries = Vec::new();

        if path.is_dir() {
            // Check if this is itself a .pretty folder
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.ends_with(".pretty"))
                .unwrap_or(false)
            {
                // Single .pretty folder
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .trim_end_matches(".pretty")
                    .to_string();

                let component_count = count_kicad_mods(path)?;

                libraries.push(LibraryInfo {
                    source: "kicad".to_string(),
                    name,
                    path: Some(path.to_string_lossy().to_string()),
                    version: None,
                    enabled: true,
                    component_count,
                });
            } else {
                // Check for .pretty folders inside this directory
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();

                    if entry_path.is_dir() {
                        if let Some(name_str) = entry_path.file_name().and_then(|n| n.to_str()) {
                            if name_str.ends_with(".pretty") {
                                let name = name_str.trim_end_matches(".pretty").to_string();
                                let component_count = count_kicad_mods(&entry_path)?;

                                libraries.push(LibraryInfo {
                                    source: "kicad".to_string(),
                                    name,
                                    path: Some(entry_path.to_string_lossy().to_string()),
                                    version: None,
                                    enabled: true,
                                    component_count,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(libraries)
    }
}

impl LibrarySource for KiCadSource {
    fn source_name(&self) -> &str {
        "kicad"
    }

    fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        let mut libraries = Vec::new();

        for search_path in &self.search_paths {
            if !search_path.exists() || !search_path.is_dir() {
                continue;
            }

            // Scan for .pretty folders
            for entry in fs::read_dir(search_path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    if let Some(name_str) = path.file_name().and_then(|n| n.to_str()) {
                        if name_str.ends_with(".pretty") {
                            let name = name_str.trim_end_matches(".pretty").to_string();
                            let component_count = count_kicad_mods(&path)?;

                            libraries.push(LibraryInfo {
                                source: "kicad".to_string(),
                                name,
                                path: Some(path.to_string_lossy().to_string()),
                                version: None,
                                enabled: true,
                                component_count,
                            });
                        }
                    }
                }
            }
        }

        Ok(libraries)
    }

    fn import_library(&self, name: &str) -> Result<Vec<Component>, LibraryError> {
        // Find the .pretty folder matching the name
        let mut library_path: Option<PathBuf> = None;

        for search_path in &self.search_paths {
            let candidate = search_path.join(format!("{}.pretty", name));
            if candidate.exists() && candidate.is_dir() {
                library_path = Some(candidate);
                break;
            }
        }

        let library_path = library_path.ok_or_else(|| {
            LibraryError::NotFound(format!("Library '{}' not found in search paths", name))
        })?;

        // Read all .kicad_mod files in the directory
        let mut components = Vec::new();

        for entry in fs::read_dir(&library_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "kicad_mod" {
                        match parse_kicad_mod(&path, name) {
                            Ok(component) => components.push(component),
                            Err(e) => {
                                // Log error but continue with other files
                                eprintln!("Warning: Failed to parse {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(components)
    }
}

/// Counts the number of .kicad_mod files in a directory
fn count_kicad_mods(path: &Path) -> Result<usize, LibraryError> {
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("kicad_mod") {
            count += 1;
        }
    }
    Ok(count)
}

/// Parses a .kicad_mod file into a Component
fn parse_kicad_mod(path: &Path, library: &str) -> Result<Component, LibraryError> {
    let content = fs::read_to_string(path)?;

    // Parse S-expression
    let value = lexpr::from_str(&content)
        .map_err(|e| LibraryError::Parse(format!("Failed to parse S-expression: {}", e)))?;

    // Extract component name from (footprint "NAME" ...)
    let name = extract_footprint_name(&value)?;

    // Extract description from (descr "...")
    let description = extract_field(&value, "descr");

    // Extract layer from (layer "...")
    let layer = extract_field(&value, "layer");

    // Derive category from layer (simple heuristic)
    let category = layer.as_ref().map(|l| {
        if l.contains("F.Cu") || l.contains("B.Cu") {
            "SMD".to_string()
        } else if l.contains("*.Cu") {
            "Through-Hole".to_string()
        } else {
            "Other".to_string()
        }
    });

    Ok(Component {
        id: ComponentId::new("kicad", &name),
        library: library.to_string(),
        category,
        footprint_data: Some(content), // Store raw S-expression for preview
        metadata: ComponentMetadata {
            description,
            package: layer, // Store layer info in package field for now
            ..Default::default()
        },
    })
}

/// Extracts the footprint name from the S-expression
fn extract_footprint_name(value: &lexpr::Value) -> Result<String, LibraryError> {
    // Navigate: (footprint "NAME" ...)
    // The value should be a list starting with symbol "footprint"

    if let lexpr::Value::Cons(ref cons) = value {
        // First element should be the symbol "footprint"
        if let lexpr::Value::Symbol(ref sym) = cons.car() {
            if sym.as_ref() == "footprint" || sym.as_ref() == "module" {
                // Second element should be the name string
                if let lexpr::Value::Cons(ref rest) = cons.cdr() {
                    if let lexpr::Value::String(ref name_str) = rest.car() {
                        return Ok(name_str.as_ref().to_string());
                    }
                }
            }
        }
    }

    Err(LibraryError::Parse(
        "Could not extract footprint name".to_string(),
    ))
}

/// Extracts a field value from the S-expression
fn extract_field(value: &lexpr::Value, field_name: &str) -> Option<String> {
    // Search for (field_name "value") pattern in the tree
    find_field_recursive(value, field_name)
}

/// Recursively searches for a field in the S-expression tree
fn find_field_recursive(value: &lexpr::Value, field_name: &str) -> Option<String> {
    match value {
        lexpr::Value::Cons(cons) => {
            // Check if this list starts with the field name
            if let lexpr::Value::Symbol(ref sym) = cons.car() {
                if sym.as_ref() == field_name {
                    // Get the next element (the value)
                    if let lexpr::Value::Cons(ref rest) = cons.cdr() {
                        if let lexpr::Value::String(ref val) = rest.car() {
                            return Some(val.as_ref().to_string());
                        }
                    }
                }
            }

            // Recursively search in car and cdr
            if let Some(result) = find_field_recursive(cons.car(), field_name) {
                return Some(result);
            }
            if let Some(result) = find_field_recursive(cons.cdr(), field_name) {
                return Some(result);
            }

            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_kicad_mod() {
        let sexpr = r#"(footprint "R_0805_2012Metric"
  (version 20211014)
  (generator pcbnew)
  (layer "F.Cu")
  (descr "Resistor SMD 0805")
  (pad "1" smd rect (at -1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
  (pad "2" smd rect (at 1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
)"#;

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_r0805.kicad_mod");
        fs::write(&test_file, sexpr).unwrap();

        // Parse
        let component = parse_kicad_mod(&test_file, "Resistor_SMD").unwrap();

        assert_eq!(component.id.source, "kicad");
        assert_eq!(component.id.name, "R_0805_2012Metric");
        assert_eq!(component.library, "Resistor_SMD");
        assert_eq!(component.category, Some("SMD".to_string()));
        assert_eq!(
            component.metadata.description,
            Some("Resistor SMD 0805".to_string())
        );
        assert!(component.footprint_data.is_some());

        // Clean up
        fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_extract_footprint_name() {
        let sexpr = r#"(footprint "TestComponent" (layer "F.Cu"))"#;
        let value = lexpr::from_str(sexpr).unwrap();
        let name = extract_footprint_name(&value).unwrap();
        assert_eq!(name, "TestComponent");
    }

    #[test]
    fn test_extract_field() {
        let sexpr = r#"(footprint "Test" (descr "Test Description") (layer "F.Cu"))"#;
        let value = lexpr::from_str(sexpr).unwrap();

        let descr = extract_field(&value, "descr");
        assert_eq!(descr, Some("Test Description".to_string()));

        let layer = extract_field(&value, "layer");
        assert_eq!(layer, Some("F.Cu".to_string()));
    }
}
