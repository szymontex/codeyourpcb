use crate::error::LibraryError;
use crate::models::{Component, ComponentMetadata, LibraryInfo};
use rusqlite::{params, Connection};

/// SQLite schema for library management with FTS5 full-text search
pub const LIBRARY_SCHEMA: &str = r#"
-- Libraries table: tracks all library sources
CREATE TABLE IF NOT EXISTS libraries (
    source TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT,
    version TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    component_count INTEGER DEFAULT 0,
    PRIMARY KEY (source, name)
);

-- Components table: stores all component data
CREATE TABLE IF NOT EXISTS components (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    name TEXT NOT NULL,
    library TEXT NOT NULL,
    category TEXT,
    footprint_data TEXT,
    description TEXT,
    datasheet_url TEXT,
    manufacturer TEXT,
    mpn TEXT,
    value TEXT,
    package TEXT,
    step_model_path TEXT,
    metadata_json TEXT,
    UNIQUE(source, name),
    FOREIGN KEY (source, library) REFERENCES libraries(source, name)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_components_category ON components(category);
CREATE INDEX IF NOT EXISTS idx_components_manufacturer ON components(manufacturer);
CREATE INDEX IF NOT EXISTS idx_components_value ON components(value);

-- FTS5 virtual table for full-text search with BM25 ranking
CREATE VIRTUAL TABLE IF NOT EXISTS components_fts USING fts5(
    source,
    name,
    category,
    description,
    manufacturer,
    mpn,
    value,
    package,
    content=components,
    content_rowid=rowid
);

-- Triggers to keep FTS5 in sync with components table
CREATE TRIGGER IF NOT EXISTS components_ai AFTER INSERT ON components BEGIN
    INSERT INTO components_fts(rowid, source, name, category, description, manufacturer, mpn, value, package)
    VALUES (new.rowid, new.source, new.name, new.category, new.description, new.manufacturer, new.mpn, new.value, new.package);
END;

CREATE TRIGGER IF NOT EXISTS components_ad AFTER DELETE ON components BEGIN
    DELETE FROM components_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS components_au AFTER UPDATE ON components BEGIN
    UPDATE components_fts SET
        source = new.source,
        name = new.name,
        category = new.category,
        description = new.description,
        manufacturer = new.manufacturer,
        mpn = new.mpn,
        value = new.value,
        package = new.package
    WHERE rowid = old.rowid;
END;
"#;

/// Initialize the library database schema
pub fn initialize_schema(conn: &Connection) -> Result<(), LibraryError> {
    conn.execute_batch(LIBRARY_SCHEMA)?;
    Ok(())
}

/// Insert a library into the database
pub fn insert_library(conn: &Connection, lib: &LibraryInfo) -> Result<(), LibraryError> {
    conn.execute(
        "INSERT OR REPLACE INTO libraries (source, name, path, version, enabled, component_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &lib.source,
            &lib.name,
            &lib.path,
            &lib.version,
            lib.enabled as i32,
            lib.component_count,
        ],
    )?;
    Ok(())
}

/// List all libraries in the database
pub fn list_libraries(conn: &Connection) -> Result<Vec<LibraryInfo>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT source, name, path, version, enabled, component_count
         FROM libraries
         ORDER BY source, name",
    )?;

    let libraries = stmt
        .query_map([], |row| {
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

/// Insert a single component into the database
pub fn insert_component(conn: &Connection, component: &Component) -> Result<(), LibraryError> {
    let metadata_json = serde_json::to_string(&component.metadata)
        .map_err(|e| LibraryError::Parse(format!("Failed to serialize metadata: {}", e)))?;

    conn.execute(
        "INSERT OR REPLACE INTO components
         (source, name, library, category, footprint_data, description, datasheet_url,
          manufacturer, mpn, value, package, step_model_path, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            &component.id.source,
            &component.id.name,
            &component.library,
            &component.category,
            &component.footprint_data,
            &component.metadata.description,
            &component.metadata.datasheet_url,
            &component.metadata.manufacturer,
            &component.metadata.mpn,
            &component.metadata.value,
            &component.metadata.package,
            &component.metadata.step_model_path,
            &metadata_json,
        ],
    )?;

    Ok(())
}

/// Insert multiple components in a single transaction
pub fn insert_components_batch(
    conn: &mut Connection,
    components: &[Component],
) -> Result<usize, LibraryError> {
    let tx = conn.transaction()?;

    for component in components {
        let metadata_json = serde_json::to_string(&component.metadata)
            .map_err(|e| LibraryError::Parse(format!("Failed to serialize metadata: {}", e)))?;

        tx.execute(
            "INSERT OR REPLACE INTO components
             (source, name, library, category, footprint_data, description, datasheet_url,
              manufacturer, mpn, value, package, step_model_path, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &component.id.source,
                &component.id.name,
                &component.library,
                &component.category,
                &component.footprint_data,
                &component.metadata.description,
                &component.metadata.datasheet_url,
                &component.metadata.manufacturer,
                &component.metadata.mpn,
                &component.metadata.value,
                &component.metadata.package,
                &component.metadata.step_model_path,
                &metadata_json,
            ],
        )?;
    }

    tx.commit()?;
    Ok(components.len())
}

/// Get a component by source and name
pub fn get_component(
    conn: &Connection,
    source: &str,
    name: &str,
) -> Result<Option<Component>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT source, name, library, category, footprint_data, description, datasheet_url,
                manufacturer, mpn, value, package, step_model_path, metadata_json
         FROM components
         WHERE source = ?1 AND name = ?2",
    )?;

    let mut rows = stmt.query(params![source, name])?;

    if let Some(row) = rows.next()? {
        let metadata_json: String = row.get(12)?;
        let metadata: ComponentMetadata = serde_json::from_str(&metadata_json)
            .map_err(|e| LibraryError::Parse(format!("Failed to parse metadata: {}", e)))?;

        Ok(Some(Component {
            id: crate::models::ComponentId {
                source: row.get(0)?,
                name: row.get(1)?,
            },
            library: row.get(2)?,
            category: row.get(3)?,
            footprint_data: row.get(4)?,
            metadata,
        }))
    } else {
        Ok(None)
    }
}

/// Delete all components for a library
pub fn delete_library_components(
    conn: &Connection,
    source: &str,
    library: &str,
) -> Result<usize, LibraryError> {
    let count = conn.execute(
        "DELETE FROM components WHERE source = ?1 AND library = ?2",
        params![source, library],
    )?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ComponentId, ComponentMetadata};

    #[test]
    fn test_schema_initialization() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"libraries".to_string()));
        assert!(tables.contains(&"components".to_string()));
        assert!(tables.contains(&"components_fts".to_string()));
    }

    #[test]
    fn test_component_crud() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert a library
        let library = LibraryInfo {
            source: "test".to_string(),
            name: "TestLib".to_string(),
            path: Some("/path/to/lib".to_string()),
            version: Some("1.0".to_string()),
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        // Insert a component
        let component = Component {
            id: ComponentId::new("test", "R_0805"),
            library: "TestLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: Some("(footprint ...)".to_string()),
            metadata: ComponentMetadata {
                description: Some("0805 Resistor".to_string()),
                datasheet_url: None,
                manufacturer: Some("TestCorp".to_string()),
                mpn: Some("TC-R0805-10K".to_string()),
                value: Some("10k".to_string()),
                package: Some("0805".to_string()),
                step_model_path: None,
            },
        };

        insert_component(&conn, &component).unwrap();

        // Retrieve the component
        let retrieved = get_component(&conn, "test", "R_0805").unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id.source, "test");
        assert_eq!(retrieved.id.name, "R_0805");
        assert_eq!(retrieved.library, "TestLib");
        assert_eq!(retrieved.category, Some("Resistors".to_string()));
        assert_eq!(
            retrieved.metadata.value,
            Some("10k".to_string())
        );
    }

    #[test]
    fn test_batch_insert() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert library
        let library = LibraryInfo {
            source: "test".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        // Create multiple components
        let components = vec![
            Component {
                id: ComponentId::new("test", "R_0805"),
                library: "TestLib".to_string(),
                category: Some("Resistors".to_string()),
                footprint_data: None,
                metadata: ComponentMetadata {
                    value: Some("10k".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("test", "C_0805"),
                library: "TestLib".to_string(),
                category: Some("Capacitors".to_string()),
                footprint_data: None,
                metadata: ComponentMetadata {
                    value: Some("100nF".to_string()),
                    ..Default::default()
                },
            },
        ];

        // Batch insert
        let count = insert_components_batch(&mut conn, &components).unwrap();
        assert_eq!(count, 2);

        // Verify both components exist
        assert!(get_component(&conn, "test", "R_0805").unwrap().is_some());
        assert!(get_component(&conn, "test", "C_0805").unwrap().is_some());
    }

    #[test]
    fn test_delete_library_components() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert library and components
        let library = LibraryInfo {
            source: "test".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        let component = Component {
            id: ComponentId::new("test", "R_0805"),
            library: "TestLib".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };
        insert_component(&conn, &component).unwrap();

        // Delete library components
        let count = delete_library_components(&conn, "test", "TestLib").unwrap();
        assert_eq!(count, 1);

        // Verify component is gone
        assert!(get_component(&conn, "test", "R_0805").unwrap().is_none());
    }

    #[test]
    fn test_fts5_trigger_sync() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert library
        let library = LibraryInfo {
            source: "test".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        // Insert a component
        let component = Component {
            id: ComponentId::new("test", "R_0805"),
            library: "TestLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("Surface mount resistor".to_string()),
                value: Some("10k".to_string()),
                ..Default::default()
            },
        };
        insert_component(&conn, &component).unwrap();

        // Query FTS5 to verify trigger synced the data
        let mut stmt = conn
            .prepare("SELECT source, name FROM components_fts WHERE components_fts MATCH 'resistor'")
            .unwrap();

        let results: Vec<String> = stmt
            .query_map([], |row| {
                let source: String = row.get(0)?;
                let name: String = row.get(1)?;
                Ok(format!("{}::{}", source, name))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "test::R_0805");
    }
}
