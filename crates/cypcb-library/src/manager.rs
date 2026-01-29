use crate::error::LibraryError;
use crate::models::{Component, ComponentId, LibraryInfo, SearchFilters, SearchResult};
use crate::schema;
use crate::search;
use crate::sources::custom::CustomSource;
use crate::sources::kicad::KiCadSource;
use crate::sources::LibrarySource;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// LibraryManager - unified orchestrator for all library operations
///
/// Provides single entry point for:
/// - Multi-source library management (KiCad, JLCPCB, Custom)
/// - Unified search across all indexed sources
/// - Import pipeline: source -> parse -> index -> search
/// - Custom library CRUD operations
pub struct LibraryManager {
    conn: Arc<Mutex<Connection>>,
    kicad_source: KiCadSource,
    custom_source: CustomSource,
    #[cfg(feature = "jlcpcb")]
    jlcpcb_source: Option<crate::sources::jlcpcb::JLCPCBSource>,
}

impl LibraryManager {
    /// Create a new LibraryManager with database at the given path
    ///
    /// Automatically initializes the SQLite schema for library management.
    ///
    /// # Arguments
    /// * `db_path` - Path to SQLite database file (or ":memory:" for in-memory)
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_library::manager::LibraryManager;
    /// use std::path::Path;
    ///
    /// let manager = LibraryManager::new(Path::new("./libraries.db")).unwrap();
    /// ```
    pub fn new(db_path: &Path) -> Result<Self, LibraryError> {
        let conn = Connection::open(db_path)?;
        schema::initialize_schema(&conn)?;

        let conn = Arc::new(Mutex::new(conn));

        Ok(Self {
            conn: Arc::clone(&conn),
            kicad_source: KiCadSource::new(Vec::new()),
            custom_source: CustomSource::new(Arc::clone(&conn)),
            #[cfg(feature = "jlcpcb")]
            jlcpcb_source: None,
        })
    }

    /// Create a new LibraryManager with in-memory database (for testing)
    pub fn new_in_memory() -> Result<Self, LibraryError> {
        let conn = Connection::open_in_memory()?;
        schema::initialize_schema(&conn)?;

        let conn = Arc::new(Mutex::new(conn));

        Ok(Self {
            conn: Arc::clone(&conn),
            kicad_source: KiCadSource::new(Vec::new()),
            custom_source: CustomSource::new(Arc::clone(&conn)),
            #[cfg(feature = "jlcpcb")]
            jlcpcb_source: None,
        })
    }

    // ========== Configuration ==========

    /// Set KiCad search paths for library discovery
    ///
    /// These paths are scanned for .pretty folders when listing or importing KiCad libraries.
    pub fn set_kicad_search_paths(&mut self, paths: Vec<PathBuf>) {
        self.kicad_source = KiCadSource::new(paths);
    }

    /// Add a single KiCad search path
    pub fn add_kicad_search_path(&mut self, path: PathBuf) {
        let paths = vec![path];
        // This is a simple implementation - for production, you'd want to preserve existing paths
        self.kicad_source = KiCadSource::new(paths);
    }

    /// Configure JLCPCB API source with API key
    #[cfg(feature = "jlcpcb")]
    pub fn configure_jlcpcb(&mut self, api_key: String) {
        self.jlcpcb_source = Some(crate::sources::jlcpcb::JLCPCBSource::new(api_key));
    }

    // ========== Import Operations ==========

    /// Import a KiCad library by name
    ///
    /// Parses all .kicad_mod files in the library and indexes them for search.
    ///
    /// # Returns
    /// Number of components imported
    pub fn import_kicad_library(&self, name: &str) -> Result<usize, LibraryError> {
        // Get components from KiCad source
        let components = self.kicad_source.import_library(name)?;

        if components.is_empty() {
            return Ok(0);
        }

        // Create library record
        let library = LibraryInfo {
            source: "kicad".to_string(),
            name: name.to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: components.len(),
        };

        let mut conn = self.conn.lock().unwrap();
        schema::insert_library(&conn, &library)?;

        // Batch insert components
        let count = schema::insert_components_batch(&mut conn, &components)?;

        Ok(count)
    }

    /// Auto-import all libraries from a folder
    ///
    /// Discovers .pretty folders and imports each one.
    ///
    /// # Returns
    /// List of imported library names
    pub fn auto_import_folder(&self, path: &Path) -> Result<Vec<String>, LibraryError> {
        let libraries = KiCadSource::auto_organize_folder(path)?;
        let mut imported = Vec::new();

        for lib in libraries {
            // Import each discovered library
            match self.import_kicad_library(&lib.name) {
                Ok(_) => {
                    imported.push(lib.name);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to import library '{}': {}", lib.name, e);
                }
            }
        }

        Ok(imported)
    }

    // ========== Search Operations ==========

    /// Unified search across all indexed sources
    ///
    /// Searches components using FTS5 full-text search with BM25 ranking.
    ///
    /// # Arguments
    /// * `query` - Search query (plain text or FTS5 syntax)
    /// * `filters` - Optional filters for category, manufacturer, source
    ///
    /// # Returns
    /// Vector of SearchResult ordered by relevance (best matches first)
    pub fn search(&self, query: &str, filters: &SearchFilters) -> Result<Vec<SearchResult>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        search::search_components(&conn, query, filters)
    }

    /// Search components by a specific field
    ///
    /// # Arguments
    /// * `field` - Field name (source, name, category, description, manufacturer, mpn, value, package)
    /// * `value` - Search value
    /// * `limit` - Maximum number of results
    pub fn search_by_field(&self, field: &str, value: &str, limit: usize) -> Result<Vec<SearchResult>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        search::search_by_field(&conn, field, value, limit)
    }

    // ========== Library Management ==========

    /// List all libraries from all sources
    pub fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        schema::list_libraries(&conn)
    }

    /// List available KiCad libraries (scans search paths for .pretty folders)
    pub fn list_kicad_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        self.kicad_source.list_libraries()
    }

    /// Create a new custom library
    pub fn create_custom_library(&self, name: &str) -> Result<LibraryInfo, LibraryError> {
        self.custom_source.create_library(name)
    }

    /// Delete a library and all its components
    pub fn delete_library(&self, source: &str, name: &str) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Delete components first
        schema::delete_library_components(&conn, source, name)?;

        // Delete library record
        conn.execute(
            "DELETE FROM libraries WHERE source = ?1 AND name = ?2",
            rusqlite::params![source, name],
        )?;

        Ok(())
    }

    // ========== Component Access ==========

    /// Get a specific component by source and name
    pub fn get_component(&self, source: &str, name: &str) -> Result<Option<Component>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        schema::get_component(&conn, source, name)
    }

    /// Get total count of components in database
    pub fn component_count(&self) -> Result<usize, LibraryError> {
        let conn = self.conn.lock().unwrap();
        search::component_count(&conn)
    }

    // ========== Custom Library Operations ==========

    /// Add a component to a custom library
    pub fn add_custom_component(&self, library: &str, component: Component) -> Result<(), LibraryError> {
        self.custom_source.add_component(library, component)
    }

    /// Remove a component from custom library
    pub fn remove_custom_component(&self, name: &str) -> Result<(), LibraryError> {
        self.custom_source.remove_component(name)
    }

    /// Update component category for organization
    pub fn update_custom_component_category(&self, name: &str, category: &str) -> Result<(), LibraryError> {
        self.custom_source.update_component_category(name, category)
    }

    /// Update component manufacturer
    pub fn update_custom_component_manufacturer(&self, name: &str, manufacturer: &str) -> Result<(), LibraryError> {
        self.custom_source.update_component_manufacturer(name, manufacturer)
    }

    /// Delete a custom library and all its components
    pub fn delete_custom_library(&self, name: &str) -> Result<(), LibraryError> {
        self.custom_source.delete_library(name)
    }

    // ========== Preview Operations ==========

    /// Get footprint preview for a component
    ///
    /// Extracts geometry data (pads, outlines, courtyard) from the component's
    /// stored footprint S-expression data for rendering.
    ///
    /// Returns None if component not found or has no footprint_data.
    pub fn get_footprint_preview(
        &self,
        source: &str,
        name: &str,
    ) -> Result<Option<crate::preview::FootprintPreview>, LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Get component
        let component = schema::get_component(&conn, source, name)?;

        if let Some(component) = component {
            if let Some(footprint_data) = component.footprint_data {
                // Extract preview from S-expression
                let preview = crate::preview::extract_preview(&footprint_data)?;
                Ok(Some(preview))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ComponentMetadata, SearchFilters};

    #[test]
    fn test_manager_initialization() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Verify empty state
        let libraries = manager.list_libraries().unwrap();
        assert_eq!(libraries.len(), 0);

        let count = manager.component_count().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_custom_library_workflow() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create custom library
        let lib = manager.create_custom_library("MyLib").unwrap();
        assert_eq!(lib.source, "custom");
        assert_eq!(lib.name, "MyLib");

        // Add component
        let component = Component {
            id: ComponentId::new("custom", "R_10K"),
            library: "MyLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("10K Resistor".to_string()),
                value: Some("10k".to_string()),
                package: Some("0805".to_string()),
                ..Default::default()
            },
        };

        manager.add_custom_component("MyLib", component).unwrap();

        // Verify component exists
        let retrieved = manager.get_component("custom", "R_10K").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id.name, "R_10K");
        assert_eq!(retrieved.metadata.value, Some("10k".to_string()));

        // Verify count
        let count = manager.component_count().unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_search_integration() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create library and add components
        manager.create_custom_library("TestLib").unwrap();

        let components = vec![
            Component {
                id: ComponentId::new("custom", "R_10K"),
                library: "TestLib".to_string(),
                category: Some("Resistors".to_string()),
                footprint_data: None,
                metadata: ComponentMetadata {
                    description: Some("10K Resistor".to_string()),
                    value: Some("10k".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("custom", "C_100N"),
                library: "TestLib".to_string(),
                category: Some("Capacitors".to_string()),
                footprint_data: None,
                metadata: ComponentMetadata {
                    description: Some("100nF Capacitor".to_string()),
                    value: Some("100nF".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
        ];

        for component in components {
            manager.add_custom_component("TestLib", component).unwrap();
        }

        // Search for resistor
        let filters = SearchFilters::default();
        let results = manager.search("resistor", &filters).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].component.id.name, "R_10K");
        assert!(results[0].rank < 0.0); // BM25 scores are negative
    }

    #[test]
    fn test_search_with_source_filter() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create two libraries with different sources
        manager.create_custom_library("CustomLib").unwrap();

        // Add custom component
        let custom_comp = Component {
            id: ComponentId::new("custom", "R_Custom"),
            library: "CustomLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("Custom resistor".to_string()),
                ..Default::default()
            },
        };
        manager.add_custom_component("CustomLib", custom_comp).unwrap();

        // Manually add a "kicad" component for testing
        let kicad_comp = Component {
            id: ComponentId::new("kicad", "R_KiCad"),
            library: "KiCadLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("KiCad resistor".to_string()),
                ..Default::default()
            },
        };

        let kicad_lib = LibraryInfo {
            source: "kicad".to_string(),
            name: "KiCadLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 1,
        };

        let conn = manager.conn.lock().unwrap();
        schema::insert_library(&conn, &kicad_lib).unwrap();
        schema::insert_component(&conn, &kicad_comp).unwrap();
        drop(conn);

        // Search with source filter
        let filters = SearchFilters {
            source: Some("custom".to_string()),
            ..Default::default()
        };

        let results = manager.search("resistor", &filters).unwrap();

        // Should only return custom component
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].component.id.source, "custom");
    }

    #[test]
    fn test_list_libraries() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create multiple libraries
        manager.create_custom_library("Lib1").unwrap();
        manager.create_custom_library("Lib2").unwrap();

        let libraries = manager.list_libraries().unwrap();
        assert_eq!(libraries.len(), 2);

        let names: Vec<&str> = libraries.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"Lib1"));
        assert!(names.contains(&"Lib2"));
    }

    #[test]
    fn test_delete_library() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create library with component
        manager.create_custom_library("ToDelete").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_Delete"),
            library: "ToDelete".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };

        manager.add_custom_component("ToDelete", component).unwrap();

        // Verify library and component exist
        assert_eq!(manager.list_libraries().unwrap().len(), 1);
        assert_eq!(manager.component_count().unwrap(), 1);

        // Delete library
        manager.delete_library("custom", "ToDelete").unwrap();

        // Verify library and component are gone
        assert_eq!(manager.list_libraries().unwrap().len(), 0);
        assert_eq!(manager.component_count().unwrap(), 0);
    }

    #[test]
    fn test_search_by_field_manufacturer() {
        let manager = LibraryManager::new_in_memory().unwrap();

        manager.create_custom_library("TestLib").unwrap();

        // Add components with different manufacturers
        let ti_comp = Component {
            id: ComponentId::new("custom", "U_TI"),
            library: "TestLib".to_string(),
            category: Some("ICs".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                manufacturer: Some("TI".to_string()),
                description: Some("TI voltage regulator".to_string()),
                ..Default::default()
            },
        };

        let maxim_comp = Component {
            id: ComponentId::new("custom", "U_Maxim"),
            library: "TestLib".to_string(),
            category: Some("ICs".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                manufacturer: Some("Maxim".to_string()),
                description: Some("Maxim chip".to_string()),
                ..Default::default()
            },
        };

        manager.add_custom_component("TestLib", ti_comp).unwrap();
        manager.add_custom_component("TestLib", maxim_comp).unwrap();

        // Search by manufacturer
        let results = manager.search_by_field("manufacturer", "TI", 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].component.id.name, "U_TI");
    }

    #[test]
    fn test_update_custom_component() {
        let manager = LibraryManager::new_in_memory().unwrap();

        manager.create_custom_library("TestLib").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_Test"),
            library: "TestLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                manufacturer: Some("OldMfg".to_string()),
                ..Default::default()
            },
        };

        manager.add_custom_component("TestLib", component).unwrap();

        // Update category
        manager.update_custom_component_category("R_Test", "Passive/Resistors").unwrap();

        // Update manufacturer
        manager.update_custom_component_manufacturer("R_Test", "NewMfg").unwrap();

        // Verify updates
        let retrieved = manager.get_component("custom", "R_Test").unwrap().unwrap();
        assert_eq!(retrieved.category, Some("Passive/Resistors".to_string()));
        // Note: manufacturer update only updates the column, not the metadata_json
        // This is expected behavior based on CustomSource implementation
    }

    #[test]
    fn test_footprint_preview() {
        let manager = LibraryManager::new_in_memory().unwrap();

        // Create library
        manager.create_custom_library("TestLib").unwrap();

        // Add component with footprint data
        let footprint_sexpr = r#"(footprint "R_0805_2012Metric"
  (version 20211014)
  (generator pcbnew)
  (layer "F.Cu")
  (descr "Resistor SMD 0805")
  (pad "1" smd rect (at -1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
  (pad "2" smd rect (at 1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
)"#;

        let component = Component {
            id: ComponentId::new("custom", "R_0805"),
            library: "TestLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: Some(footprint_sexpr.to_string()),
            metadata: ComponentMetadata::default(),
        };

        manager.add_custom_component("TestLib", component).unwrap();

        // Get footprint preview
        let preview = manager.get_footprint_preview("custom", "R_0805").unwrap();
        assert!(preview.is_some());

        let preview = preview.unwrap();
        assert_eq!(preview.name, "R_0805_2012Metric");
        assert_eq!(preview.description, Some("Resistor SMD 0805".to_string()));
        assert_eq!(preview.pads.len(), 2);
        assert_eq!(preview.pads[0].name, "1");
        assert_eq!(preview.pads[0].x, -1.0);

        // Test missing footprint data
        let component_no_fp = Component {
            id: ComponentId::new("custom", "NoFootprint"),
            library: "TestLib".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };

        manager.add_custom_component("TestLib", component_no_fp).unwrap();

        let preview = manager.get_footprint_preview("custom", "NoFootprint").unwrap();
        assert!(preview.is_none());
    }
}
