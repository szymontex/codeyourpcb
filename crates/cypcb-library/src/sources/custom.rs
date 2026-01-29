use crate::error::LibraryError;
use crate::models::{Component, ComponentId, LibraryInfo};
use crate::schema;
use crate::sources::LibrarySource;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Custom library source for user-created component libraries
///
/// Custom libraries allow users to:
/// - Create custom component libraries with custom:: namespace
/// - Organize components by manufacturer or function categories
/// - Add, remove, and update components through API
pub struct CustomSource {
    conn: Arc<Mutex<Connection>>,
}

impl CustomSource {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Create a new custom library
    pub fn create_library(&self, name: &str) -> Result<LibraryInfo, LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Check if library already exists
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM libraries WHERE source = ?1 AND name = ?2",
                rusqlite::params!["custom", name],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            return Err(LibraryError::Parse(format!(
                "Library '{}' already exists",
                name
            )));
        }

        let lib = LibraryInfo {
            source: "custom".to_string(),
            name: name.to_string(),
            path: None,
            version: Some(chrono::Utc::now().to_rfc3339()),
            enabled: true,
            component_count: 0,
        };

        schema::insert_library(&conn, &lib)?;
        Ok(lib)
    }

    /// Add a component to a custom library
    pub fn add_component(
        &self,
        library: &str,
        component: Component,
    ) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Validate library exists and is a custom library
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM libraries WHERE source = ?1 AND name = ?2",
                rusqlite::params!["custom", library],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !exists {
            return Err(LibraryError::NotFound(format!(
                "Custom library '{}' not found",
                library
            )));
        }

        // Ensure component has custom source
        if component.id.source != "custom" {
            return Err(LibraryError::Parse(format!(
                "Component must have source='custom', got '{}'",
                component.id.source
            )));
        }

        // Insert component
        schema::insert_component(&conn, &component)?;

        // Update library component count
        conn.execute(
            "UPDATE libraries SET component_count = component_count + 1
             WHERE source = ?1 AND name = ?2",
            rusqlite::params!["custom", library],
        )?;

        Ok(())
    }

    /// Remove a component from custom library
    pub fn remove_component(&self, name: &str) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Get the library name before deleting
        let library: Option<String> = conn
            .query_row(
                "SELECT library FROM components WHERE source = ?1 AND name = ?2",
                rusqlite::params!["custom", name],
                |row| row.get(0),
            )
            .ok();

        // Delete component
        let deleted = conn.execute(
            "DELETE FROM components WHERE source = ?1 AND name = ?2",
            rusqlite::params!["custom", name],
        )?;

        if deleted == 0 {
            return Err(LibraryError::NotFound(format!(
                "Component 'custom::{}' not found",
                name
            )));
        }

        // Update library component count
        if let Some(lib_name) = library {
            conn.execute(
                "UPDATE libraries SET component_count = component_count - 1
                 WHERE source = ?1 AND name = ?2",
                rusqlite::params!["custom", &lib_name],
            )?;
        }

        Ok(())
    }

    /// Update component category for organization by function
    pub fn update_component_category(
        &self,
        name: &str,
        category: &str,
    ) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        let updated = conn.execute(
            "UPDATE components SET category = ?1 WHERE source = ?2 AND name = ?3",
            rusqlite::params![category, "custom", name],
        )?;

        if updated == 0 {
            return Err(LibraryError::NotFound(format!(
                "Component 'custom::{}' not found",
                name
            )));
        }

        Ok(())
    }

    /// Update component manufacturer for organization
    pub fn update_component_manufacturer(
        &self,
        name: &str,
        manufacturer: &str,
    ) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Update both the manufacturer column and the metadata_json
        let updated = conn.execute(
            "UPDATE components SET manufacturer = ?1 WHERE source = ?2 AND name = ?3",
            rusqlite::params![manufacturer, "custom", name],
        )?;

        if updated == 0 {
            return Err(LibraryError::NotFound(format!(
                "Component 'custom::{}' not found",
                name
            )));
        }

        Ok(())
    }

    /// Delete a custom library and all its components
    pub fn delete_library(&self, name: &str) -> Result<(), LibraryError> {
        let conn = self.conn.lock().unwrap();

        // Delete all components first (foreign key constraint)
        conn.execute(
            "DELETE FROM components WHERE source = ?1 AND library = ?2",
            rusqlite::params!["custom", name],
        )?;

        // Delete library
        let deleted = conn.execute(
            "DELETE FROM libraries WHERE source = ?1 AND name = ?2",
            rusqlite::params!["custom", name],
        )?;

        if deleted == 0 {
            return Err(LibraryError::NotFound(format!(
                "Custom library '{}' not found",
                name
            )));
        }

        Ok(())
    }
}

impl LibrarySource for CustomSource {
    fn source_name(&self) -> &str {
        "custom"
    }

    fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT source, name, path, version, enabled, component_count
             FROM libraries
             WHERE source = ?1
             ORDER BY name",
        )?;

        let libraries = stmt
            .query_map(rusqlite::params!["custom"], |row| {
                Ok(LibraryInfo {
                    source: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    version: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                    component_count: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(libraries)
    }

    fn import_library(&self, name: &str) -> Result<Vec<Component>, LibraryError> {
        let conn = self.conn.lock().unwrap();

        // For custom libraries, components already exist in DB
        // This is a no-op that returns existing components
        let mut stmt = conn.prepare(
            "SELECT source, name, library, category, footprint_data,
                    description, datasheet_url, manufacturer, mpn, value, package,
                    step_model_path, metadata_json
             FROM components
             WHERE source = ?1 AND library = ?2",
        )?;

        let components = stmt
            .query_map(rusqlite::params!["custom", name], |row| {
                let metadata_json: String = row.get(12)?;
                let metadata = serde_json::from_str(&metadata_json).unwrap_or_default();

                Ok(Component {
                    id: ComponentId::new(row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    library: row.get(2)?,
                    category: row.get(3)?,
                    footprint_data: row.get(4)?,
                    metadata,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ComponentMetadata;
    use crate::schema::initialize_schema;

    #[test]
    fn test_create_custom_library() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let source = CustomSource::new(Arc::new(Mutex::new(conn)));

        let lib = source.create_library("MyComponents").unwrap();
        assert_eq!(lib.source, "custom");
        assert_eq!(lib.name, "MyComponents");
        assert_eq!(lib.component_count, 0);

        // Should fail to create duplicate
        let result = source.create_library("MyComponents");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_retrieve_component() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let source = CustomSource::new(Arc::new(Mutex::new(conn)));
        source.create_library("MyComponents").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_10K"),
            library: "MyComponents".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("10K Resistor".to_string()),
                value: Some("10k".to_string()),
                package: Some("0805".to_string()),
                ..Default::default()
            },
        };

        source.add_component("MyComponents", component).unwrap();

        // Retrieve via import_library
        let components = source.import_library("MyComponents").unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].id.name, "R_10K");
        assert_eq!(components[0].category, Some("Resistors".to_string()));
    }

    #[test]
    fn test_update_category() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let source = CustomSource::new(Arc::new(Mutex::new(conn)));
        source.create_library("MyComponents").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_10K"),
            library: "MyComponents".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };

        source.add_component("MyComponents", component).unwrap();

        // Update category
        source
            .update_component_category("R_10K", "Passive/Resistors/0805")
            .unwrap();

        let components = source.import_library("MyComponents").unwrap();
        assert_eq!(
            components[0].category,
            Some("Passive/Resistors/0805".to_string())
        );
    }

    #[test]
    fn test_delete_component() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let source = CustomSource::new(Arc::new(Mutex::new(conn)));
        source.create_library("MyComponents").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_10K"),
            library: "MyComponents".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };

        source.add_component("MyComponents", component).unwrap();
        assert_eq!(source.import_library("MyComponents").unwrap().len(), 1);

        source.remove_component("R_10K").unwrap();
        assert_eq!(source.import_library("MyComponents").unwrap().len(), 0);
    }

    #[test]
    fn test_delete_library() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let source = CustomSource::new(Arc::new(Mutex::new(conn)));
        source.create_library("MyComponents").unwrap();

        let component = Component {
            id: ComponentId::new("custom", "R_10K"),
            library: "MyComponents".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };

        source.add_component("MyComponents", component).unwrap();

        // Delete library
        source.delete_library("MyComponents").unwrap();

        // Should not find library
        let libraries = source.list_libraries().unwrap();
        assert_eq!(libraries.len(), 0);

        // Components should also be deleted
        let components = source.import_library("MyComponents").unwrap();
        assert_eq!(components.len(), 0);
    }
}
