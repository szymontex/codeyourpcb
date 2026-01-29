use crate::error::LibraryError;
use crate::models::{Component, ComponentId, ComponentMetadata, SearchFilters, SearchResult};
use rusqlite::Connection;

/// Search components using FTS5 full-text search with BM25 ranking
///
/// # Arguments
/// * `conn` - SQLite connection
/// * `query` - Search query (plain text or FTS5 syntax)
/// * `filters` - Optional filters for category, manufacturer, source
///
/// # Returns
/// Vector of SearchResult ordered by relevance (best matches first)
pub fn search_components(
    conn: &Connection,
    query: &str,
    filters: &SearchFilters,
) -> Result<Vec<SearchResult>, LibraryError> {
    // Sanitize and validate query
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    // For FTS5, plain text search just uses the query as-is
    // FTS5 will tokenize and search across all indexed fields
    // For field-specific queries, the caller should pass "field:value" format
    let fts_query = if query.contains(':') {
        // Field-specific query, pass through
        query.to_string()
    } else if query.ends_with('*') {
        // Prefix query, pass through
        query.to_string()
    } else {
        // Plain text query - escape double quotes for safety
        query.replace('"', "\"\"")
    };

    // Build base SQL with FTS5 MATCH and BM25 ranking
    // Note: bm25() returns NEGATIVE scores; lower (more negative) = better match
    let mut sql = String::from(
        "SELECT c.source, c.name, c.library, c.category, c.footprint_data,
                c.description, c.datasheet_url, c.manufacturer, c.mpn,
                c.value, c.package, c.step_model_path, c.metadata_json,
                bm25(components_fts) as rank
         FROM components c
         JOIN components_fts fts ON c.rowid = fts.rowid
         WHERE components_fts MATCH ?1",
    );

    // Build parameter list starting with query
    let mut params: Vec<String> = vec![fts_query.clone()];
    let mut param_index = 2;

    // Add optional filters
    if let Some(ref source) = filters.source {
        sql.push_str(&format!(" AND c.source = ?{}", param_index));
        params.push(source.clone());
        param_index += 1;
    }

    if let Some(ref category) = filters.category {
        sql.push_str(&format!(" AND c.category = ?{}", param_index));
        params.push(category.clone());
        param_index += 1;
    }

    if let Some(ref manufacturer) = filters.manufacturer {
        sql.push_str(&format!(" AND c.manufacturer = ?{}", param_index));
        params.push(manufacturer.clone());
        param_index += 1;
    }

    // Order by rank (ascending, since lower = better) and apply limit
    sql.push_str(&format!(" ORDER BY rank LIMIT ?{}", param_index));
    let limit_str = filters.limit.to_string();
    params.push(limit_str);

    // Execute query
    let mut stmt = conn.prepare(&sql)?;

    // Convert to refs for rusqlite params
    let param_refs: Vec<&dyn rusqlite::ToSql> = params
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();

    let results = stmt
        .query_map(param_refs.as_slice(), |row| {
            let metadata_json: String = row.get(12)?;
            let metadata: ComponentMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?;

            let component = Component {
                id: ComponentId {
                    source: row.get(0)?,
                    name: row.get(1)?,
                },
                library: row.get(2)?,
                category: row.get(3)?,
                footprint_data: row.get(4)?,
                metadata,
            };

            let rank: f64 = row.get(13)?;

            Ok(SearchResult { component, rank })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Search components by a specific field
///
/// Uses FTS5 field-specific query syntax: field:value
///
/// # Arguments
/// * `conn` - SQLite connection
/// * `field` - Field name (source, name, category, description, manufacturer, mpn, value, package)
/// * `value` - Search value
/// * `limit` - Maximum number of results
///
/// # Returns
/// Vector of SearchResult ordered by relevance
pub fn search_by_field(
    conn: &Connection,
    field: &str,
    value: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, LibraryError> {
    // Validate field name
    let valid_fields = [
        "source",
        "name",
        "category",
        "description",
        "manufacturer",
        "mpn",
        "value",
        "package",
    ];

    if !valid_fields.contains(&field) {
        return Err(LibraryError::NotSupported(format!(
            "Field '{}' is not supported for search. Valid fields: {}",
            field,
            valid_fields.join(", ")
        )));
    }

    // Build FTS5 field-specific query
    let fts_query = format!("{}:{}", field, value);

    // Use search_components with field query
    let filters = SearchFilters {
        limit,
        ..Default::default()
    };

    search_components(conn, &fts_query, &filters)
}

/// Rebuild the FTS5 search index
///
/// Useful after bulk operations or if the index becomes corrupted.
/// This operation can take time on large databases.
pub fn rebuild_index(conn: &Connection) -> Result<(), LibraryError> {
    conn.execute("INSERT INTO components_fts(components_fts) VALUES('rebuild')", [])?;
    Ok(())
}

/// Get the total count of components in the database
pub fn component_count(conn: &Connection) -> Result<usize, LibraryError> {
    let count: usize = conn.query_row("SELECT COUNT(*) FROM components", [], |row| row.get(0))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LibraryInfo;
    use crate::schema::{initialize_schema, insert_component, insert_library};

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert test library
        let library = LibraryInfo {
            source: "test".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        // Insert test components
        let components = vec![
            Component {
                id: ComponentId::new("test", "R_0805_10K"),
                library: "TestLib".to_string(),
                category: Some("Resistors".to_string()),
                footprint_data: Some("(footprint R_0805)".to_string()),
                metadata: ComponentMetadata {
                    description: Some("Surface mount resistor 10k ohm".to_string()),
                    manufacturer: Some("TestCorp".to_string()),
                    mpn: Some("TC-R0805-10K".to_string()),
                    value: Some("10k".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("test", "R_0805_1K"),
                library: "TestLib".to_string(),
                category: Some("Resistors".to_string()),
                footprint_data: Some("(footprint R_0805)".to_string()),
                metadata: ComponentMetadata {
                    description: Some("Surface mount resistor 1k ohm".to_string()),
                    manufacturer: Some("TestCorp".to_string()),
                    mpn: Some("TC-R0805-1K".to_string()),
                    value: Some("1k".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("test", "C_0805_100N"),
                library: "TestLib".to_string(),
                category: Some("Capacitors".to_string()),
                footprint_data: Some("(footprint C_0805)".to_string()),
                metadata: ComponentMetadata {
                    description: Some("Ceramic capacitor 100nF".to_string()),
                    manufacturer: Some("CapCorp".to_string()),
                    mpn: Some("CC-C0805-100N".to_string()),
                    value: Some("100nF".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("test", "LED_0805_RED"),
                library: "TestLib".to_string(),
                category: Some("LEDs".to_string()),
                footprint_data: Some("(footprint LED_0805)".to_string()),
                metadata: ComponentMetadata {
                    description: Some("Red LED indicator".to_string()),
                    manufacturer: Some("LightCo".to_string()),
                    mpn: Some("LC-LED-RED".to_string()),
                    package: Some("0805".to_string()),
                    ..Default::default()
                },
            },
            Component {
                id: ComponentId::new("test", "U_SOT23_TI"),
                library: "TestLib".to_string(),
                category: Some("ICs".to_string()),
                footprint_data: Some("(footprint SOT-23)".to_string()),
                metadata: ComponentMetadata {
                    description: Some("Voltage regulator".to_string()),
                    manufacturer: Some("TI".to_string()),
                    mpn: Some("TPS7A0518".to_string()),
                    package: Some("SOT-23".to_string()),
                    ..Default::default()
                },
            },
        ];

        for component in components {
            insert_component(&conn, &component).unwrap();
        }

        conn
    }

    #[test]
    fn test_search_resistor() {
        let conn = setup_test_db();

        let filters = SearchFilters::default();
        let results = search_components(&conn, "resistor", &filters).unwrap();

        // Should return 2 resistor components
        assert_eq!(results.len(), 2);
        assert!(results[0].component.category == Some("Resistors".to_string()));
        assert!(results[1].component.category == Some("Resistors".to_string()));

        // Verify ranking (both should have negative rank)
        assert!(results[0].rank < 0.0);
    }

    #[test]
    fn test_search_with_source_filter() {
        let conn = setup_test_db();

        // Add component from different source
        let other_lib = LibraryInfo {
            source: "other".to_string(),
            name: "OtherLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &other_lib).unwrap();

        let other_component = Component {
            id: ComponentId::new("other", "R_0805"),
            library: "OtherLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("resistor from other source".to_string()),
                ..Default::default()
            },
        };
        insert_component(&conn, &other_component).unwrap();

        // Search with source filter
        let filters = SearchFilters {
            source: Some("test".to_string()),
            ..Default::default()
        };
        let results = search_components(&conn, "resistor", &filters).unwrap();

        // Should only return components from "test" source
        assert!(results.len() >= 2);
        for result in results {
            assert_eq!(result.component.id.source, "test");
        }
    }

    #[test]
    fn test_search_by_field_manufacturer() {
        let conn = setup_test_db();

        let results = search_by_field(&conn, "manufacturer", "TI", 10).unwrap();

        // Should return only TI component
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].component.metadata.manufacturer,
            Some("TI".to_string())
        );
    }

    #[test]
    fn test_empty_query() {
        let conn = setup_test_db();

        let filters = SearchFilters::default();
        let results = search_components(&conn, "", &filters).unwrap();

        // Empty query should return empty results
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_invalid_field() {
        let conn = setup_test_db();

        let result = search_by_field(&conn, "invalid_field", "test", 10);

        // Should return error for invalid field
        assert!(result.is_err());
        match result {
            Err(LibraryError::NotSupported(msg)) => {
                assert!(msg.contains("invalid_field"));
            }
            _ => panic!("Expected NotSupported error"),
        }
    }

    #[test]
    fn test_component_count() {
        let conn = setup_test_db();

        let count = component_count(&conn).unwrap();

        // Should have 5 test components
        assert_eq!(count, 5);
    }

    #[test]
    fn test_rebuild_index() {
        let conn = setup_test_db();

        // Should complete without error
        rebuild_index(&conn).unwrap();

        // Verify search still works after rebuild
        let filters = SearchFilters::default();
        let results = search_components(&conn, "resistor", &filters).unwrap();
        assert!(results.len() >= 2);
    }
}
