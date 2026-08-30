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

/// Parses a .kicad_mod file into a Component.
///
/// Through `cypcb-kicad`, which is the reader the rest of this project uses
/// for the same files. This used `lexpr` and could not read the ones KiCad
/// writes today: `(tedit 5E1BAA69)` is a hexadecimal timestamp, a generic
/// S-expression reader takes `5E1BAA69` for a malformed float, and every
/// modern footprint was refused with `invalid number at line 1 column 42`.
/// Measured on three files pulled out of this repository - two of the three
/// failed, and the third was a hand-written fixture.
///
/// The pads, the courtyard and the geometry come back too; what is kept here
/// is what a search needs, and the raw text so a preview can draw it.
fn parse_kicad_mod(path: &Path, library: &str) -> Result<Component, LibraryError> {
    let content = fs::read_to_string(path)?;

    let footprint = cypcb_kicad::import_footprint_from_str(&content)
        .map_err(|e| LibraryError::Parse(format!("{e}")))?;

    // A footprint with pads on the top copper is surface mount, one with a
    // drilled pad is through hole. The file's own `layer` said only where the
    // body was drawn, which is `F.Cu` for both.
    let category = if footprint.pads.iter().any(|pad| pad.drill.is_some()) {
        Some("Through-Hole".to_string())
    } else if footprint.pads.is_empty() {
        None
    } else {
        Some("SMD".to_string())
    };

    let description = if footprint.description.is_empty() {
        None
    } else {
        Some(footprint.description.clone())
    };

    Ok(Component {
        id: ComponentId::new("kicad", &footprint.name),
        library: library.to_string(),
        category,
        footprint_data: Some(content),
        metadata: ComponentMetadata {
            description,
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_kicad_mod() {
        // A KiCad 5 footprint, which is what `cypcb-kicad` reads. The KiCad 6
        // form is refused today and the test below says so with the reason.
        let sexpr = r#"(module R_0805_2012Metric
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

    /// The two tests that stood here read `lexpr` values through helpers this
    /// file no longer has: the reader was replaced by `cypcb-kicad`, which is
    /// what the rest of this project uses for the same files. What they
    /// checked - a name and a description coming out of an S-expression - is
    /// checked by the test above through the reader that now does it, and by
    /// `the_library_takes_footprints_in_and_gives_them_back` on real KiCad
    /// footprints out of this repository.
    #[test]
    fn a_footprint_the_old_reader_refused_is_read_now() {
        // `(tedit 5E1BAA69)` is a hexadecimal timestamp. A generic
        // S-expression reader takes it for a malformed float and refuses the
        // whole file - `invalid number at line 1 column 42` - which is every
        // footprint KiCad wrote for years.
        let sexpr = r#"(module Test_Part (layer F.Cu) (tedit 5E1BAA69)
  (descr "a part with a timestamp")
  (fp_text reference REF** (at 0 0) (layer F.SilkS) (effects (font (size 1 1) (thickness 0.15))))
  (pad 1 smd rect (at -1 0) (size 1 0.95) (layers F.Cu F.Paste F.Mask))
)"#;
        let file = std::env::temp_dir().join("cypcb-library-tedit.kicad_mod");
        fs::write(&file, sexpr).unwrap();

        let component = parse_kicad_mod(&file, "Test").unwrap();
        assert_eq!(component.id.name, "Test_Part");
        assert_eq!(component.category, Some("SMD".to_string()));

        fs::remove_file(file).unwrap();
    }

    /// A footprint written by KiCad 6 or later, which is most of what a
    /// person has on disk.
    ///
    /// This was the gap: KiCad 6 renamed the head of the list to `footprint`
    /// and put `(version ...)` and `(generator ...)` at the top of it, and the
    /// reader refused the file with `unknown element in module: version`. It
    /// reads them now, and the head is no longer renamed here - the reader
    /// takes both spellings, so this path has nothing to translate.
    #[test]
    fn a_kicad_6_footprint_is_read() {
        let sexpr = r#"(footprint "R_0805_2012Metric"
  (version 20211014)
  (generator pcbnew)
  (layer "F.Cu")
  (descr "Resistor SMD 0805")
  (pad "1" smd rect (at -1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
)"#;
        let file = std::env::temp_dir().join("cypcb-library-kicad6.kicad_mod");
        fs::write(&file, sexpr).unwrap();

        let component = parse_kicad_mod(&file, "Resistor_SMD").unwrap();
        assert_eq!(component.id.name, "R_0805_2012Metric");
        assert_eq!(
            component.metadata.description.as_deref(),
            Some("Resistor SMD 0805")
        );
        assert_eq!(component.category, Some("SMD".to_string()));

        fs::remove_file(file).unwrap();
    }
}
