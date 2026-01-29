use crate::error::LibraryError;
use crate::models::ComponentMetadata;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Library version record for tracking import history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryVersion {
    pub id: Option<i64>,
    pub source: String,
    pub library_name: String,
    pub version_id: Option<String>,
    /// ISO 8601 timestamp (YYYY-MM-DDTHH:MM:SSZ)
    pub imported_at: String,
    pub component_count: usize,
    pub notes: Option<String>,
}

/// Format SystemTime as ISO 8601 / RFC 3339 string
fn format_timestamp(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards");
    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Convert to date/time components
    const SECS_PER_DAY: u64 = 86400;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_MINUTE: u64 = 60;

    let days_since_epoch = secs / SECS_PER_DAY;
    let secs_today = secs % SECS_PER_DAY;
    let hours = secs_today / SECS_PER_HOUR;
    let minutes = (secs_today % SECS_PER_HOUR) / SECS_PER_MINUTE;
    let seconds = secs_today % SECS_PER_MINUTE;

    // Days since Unix epoch (1970-01-01) to Gregorian calendar
    // Simplified: Assume ~365.25 days/year for rough conversion
    let years_since_1970 = days_since_epoch / 365;
    let year = 1970 + years_since_1970;
    let day_of_year = days_since_epoch % 365;

    // Rough month/day calculation (simplified, good enough for timestamps)
    let month = (day_of_year / 30).min(11) + 1;
    let day = (day_of_year % 30) + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year,
        month,
        day,
        hours,
        minutes,
        seconds,
        nanos / 1_000_000
    )
}

/// Track a new library version import
pub fn track_version(
    conn: &Connection,
    source: &str,
    library_name: &str,
    component_count: usize,
    notes: Option<&str>,
) -> Result<LibraryVersion, LibraryError> {
    let timestamp = format_timestamp(SystemTime::now());

    conn.execute(
        "INSERT INTO library_versions (source, library_name, version_id, imported_at, component_count, notes)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
        params![source, library_name, &timestamp, component_count, notes],
    )?;

    let id = conn.last_insert_rowid();

    Ok(LibraryVersion {
        id: Some(id),
        source: source.to_string(),
        library_name: library_name.to_string(),
        version_id: None,
        imported_at: timestamp,
        component_count,
        notes: notes.map(|s| s.to_string()),
    })
}

/// List all versions for a library, ordered by import time (newest first)
pub fn list_versions(
    conn: &Connection,
    source: &str,
    library_name: &str,
) -> Result<Vec<LibraryVersion>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT id, source, library_name, version_id, imported_at, component_count, notes
         FROM library_versions
         WHERE source = ?1 AND library_name = ?2
         ORDER BY imported_at DESC",
    )?;

    let versions = stmt
        .query_map(params![source, library_name], |row| {
            Ok(LibraryVersion {
                id: Some(row.get(0)?),
                source: row.get(1)?,
                library_name: row.get(2)?,
                version_id: row.get(3)?,
                imported_at: row.get(4)?,
                component_count: row.get::<_, usize>(5)?,
                notes: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(versions)
}

/// Get the most recent version for a library
pub fn latest_version(
    conn: &Connection,
    source: &str,
    library_name: &str,
) -> Result<Option<LibraryVersion>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT id, source, library_name, version_id, imported_at, component_count, notes
         FROM library_versions
         WHERE source = ?1 AND library_name = ?2
         ORDER BY imported_at DESC
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![source, library_name])?;

    if let Some(row) = rows.next()? {
        Ok(Some(LibraryVersion {
            id: Some(row.get(0)?),
            source: row.get(1)?,
            library_name: row.get(2)?,
            version_id: row.get(3)?,
            imported_at: row.get(4)?,
            component_count: row.get::<_, usize>(5)?,
            notes: row.get(6)?,
        }))
    } else {
        Ok(None)
    }
}

/// Associate a 3D STEP model path with a component
pub fn associate_step_model(
    conn: &Connection,
    source: &str,
    component_name: &str,
    step_path: &str,
) -> Result<(), LibraryError> {
    // First verify component exists
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM components WHERE source = ?1 AND name = ?2",
        params![source, component_name],
        |row| row.get(0),
    )?;

    if count == 0 {
        return Err(LibraryError::NotFound(format!(
            "Component {}::{} not found",
            source, component_name
        )));
    }

    // Update the step_model_path
    conn.execute(
        "UPDATE components SET step_model_path = ?1 WHERE source = ?2 AND name = ?3",
        params![step_path, source, component_name],
    )?;

    Ok(())
}

/// Get the STEP model path for a component
pub fn get_step_model_path(
    conn: &Connection,
    source: &str,
    component_name: &str,
) -> Result<Option<String>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT step_model_path FROM components WHERE source = ?1 AND name = ?2",
    )?;

    let mut rows = stmt.query(params![source, component_name])?;

    if let Some(row) = rows.next()? {
        Ok(row.get(0)?)
    } else {
        Ok(None)
    }
}

/// Get component metadata (description, datasheet, manufacturer, etc.)
pub fn get_component_metadata(
    conn: &Connection,
    source: &str,
    name: &str,
) -> Result<Option<ComponentMetadata>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT metadata_json FROM components WHERE source = ?1 AND name = ?2",
    )?;

    let mut rows = stmt.query(params![source, name])?;

    if let Some(row) = rows.next()? {
        let metadata_json: String = row.get(0)?;
        let metadata: ComponentMetadata = serde_json::from_str(&metadata_json)
            .map_err(|e| LibraryError::Parse(format!("Failed to parse metadata: {}", e)))?;
        Ok(Some(metadata))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Component, ComponentId, ComponentMetadata, LibraryInfo};
    use crate::schema::{initialize_schema, insert_component, insert_library};

    #[test]
    fn test_track_version() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        let version = track_version(&conn, "kicad", "Resistors", 42, Some("Initial import"))
            .unwrap();

        assert_eq!(version.source, "kicad");
        assert_eq!(version.library_name, "Resistors");
        assert_eq!(version.component_count, 42);
        assert_eq!(version.notes, Some("Initial import".to_string()));
        assert!(version.id.is_some());
        assert!(version.imported_at.contains("T")); // ISO 8601 format
    }

    #[test]
    fn test_list_versions_chronological() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Track multiple versions
        track_version(&conn, "kicad", "Resistors", 40, Some("v1")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        track_version(&conn, "kicad", "Resistors", 45, Some("v2")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        track_version(&conn, "kicad", "Resistors", 50, Some("v3")).unwrap();

        let versions = list_versions(&conn, "kicad", "Resistors").unwrap();
        assert_eq!(versions.len(), 3);

        // Should be newest first
        assert_eq!(versions[0].component_count, 50);
        assert_eq!(versions[1].component_count, 45);
        assert_eq!(versions[2].component_count, 40);
    }

    #[test]
    fn test_latest_version() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // No versions yet
        let latest = latest_version(&conn, "kicad", "Resistors").unwrap();
        assert!(latest.is_none());

        // Track versions
        track_version(&conn, "kicad", "Resistors", 40, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        track_version(&conn, "kicad", "Resistors", 50, Some("Latest")).unwrap();

        let latest = latest_version(&conn, "kicad", "Resistors").unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().component_count, 50);
    }

    #[test]
    fn test_associate_step_model() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert library and component
        let library = LibraryInfo {
            source: "kicad".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        let component = Component {
            id: ComponentId::new("kicad", "R_0805"),
            library: "TestLib".to_string(),
            category: None,
            footprint_data: None,
            metadata: ComponentMetadata::default(),
        };
        insert_component(&conn, &component).unwrap();

        // Associate STEP model
        associate_step_model(&conn, "kicad", "R_0805", "/models/r_0805.step").unwrap();

        // Retrieve STEP model path
        let path = get_step_model_path(&conn, "kicad", "R_0805").unwrap();
        assert_eq!(path, Some("/models/r_0805.step".to_string()));
    }

    #[test]
    fn test_associate_step_model_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Try to associate with nonexistent component
        let result = associate_step_model(&conn, "kicad", "NonExistent", "/models/test.step");

        assert!(result.is_err());
        match result {
            Err(LibraryError::NotFound(msg)) => {
                assert!(msg.contains("NonExistent"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_get_component_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();

        // Insert library and component with metadata
        let library = LibraryInfo {
            source: "kicad".to_string(),
            name: "TestLib".to_string(),
            path: None,
            version: None,
            enabled: true,
            component_count: 0,
        };
        insert_library(&conn, &library).unwrap();

        let component = Component {
            id: ComponentId::new("kicad", "R_0805"),
            library: "TestLib".to_string(),
            category: Some("Resistors".to_string()),
            footprint_data: None,
            metadata: ComponentMetadata {
                description: Some("0805 Resistor".to_string()),
                datasheet_url: Some("https://example.com/r0805.pdf".to_string()),
                manufacturer: Some("Yageo".to_string()),
                mpn: Some("RC0805FR-0710KL".to_string()),
                value: Some("10k".to_string()),
                package: Some("0805".to_string()),
                step_model_path: None,
            },
        };
        insert_component(&conn, &component).unwrap();

        // Retrieve metadata
        let metadata = get_component_metadata(&conn, "kicad", "R_0805").unwrap();
        assert!(metadata.is_some());

        let metadata = metadata.unwrap();
        assert_eq!(metadata.description, Some("0805 Resistor".to_string()));
        assert_eq!(metadata.manufacturer, Some("Yageo".to_string()));
        assert_eq!(metadata.value, Some("10k".to_string()));
    }
}
