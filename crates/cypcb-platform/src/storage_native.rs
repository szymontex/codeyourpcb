use crate::error::PlatformError;
use crate::storage::Storage;
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::path::Path;

/// SQLite-based storage for native platforms.
///
/// Stores key-value pairs in a single SQLite database with table namespacing.
/// Schema: `kv_store(table_name TEXT, key TEXT, value BLOB, PRIMARY KEY (table_name, key))`
///
/// # Thread Safety
/// rusqlite::Connection is not Send, so we use `?Send` in async_trait.
/// For multi-threaded access, wrap in a synchronization primitive (Mutex, RwLock).
///
/// # Example
/// ```no_run
/// use cypcb_platform::SqliteStorage;
/// use std::path::Path;
///
/// # async fn example() {
/// let storage = SqliteStorage::new(Path::new("data.db")).unwrap();
/// storage.init().await.unwrap();
/// # }
/// ```
pub struct SqliteStorage {
    conn: rusqlite::Connection,
}

impl SqliteStorage {
    /// Create a new SQLite storage backend.
    ///
    /// Opens or creates a SQLite database at the given path.
    /// Call `init()` after construction to create schema.
    ///
    /// # Arguments
    /// * `path` - Path to SQLite database file (will be created if doesn't exist)
    ///
    /// # Returns
    /// * `Ok(storage)` - Storage backend ready for initialization
    /// * `Err(_)` - Could not open/create database file
    pub fn new(path: &Path) -> Result<Self, PlatformError> {
        let conn = rusqlite::Connection::open(path).map_err(|e| {
            PlatformError::Storage(format!("Failed to open SQLite database: {}", e))
        })?;
        Ok(Self { conn })
    }
}

#[async_trait(?Send)]
impl Storage for SqliteStorage {
    async fn init(&self) -> Result<(), PlatformError> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS kv_store (
                    table_name TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value BLOB NOT NULL,
                    PRIMARY KEY (table_name, key)
                )",
                [],
            )
            .map_err(|e| PlatformError::Storage(format!("Failed to create schema: {}", e)))?;
        Ok(())
    }

    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT value FROM kv_store WHERE table_name = ?1 AND key = ?2")
            .map_err(|e| PlatformError::Storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt
            .query_row([table, key], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| PlatformError::Storage(format!("Failed to query: {}", e)))?;

        Ok(result)
    }

    async fn get_string(&self, table: &str, key: &str) -> Result<Option<String>, PlatformError> {
        match self.get(table, key).await? {
            Some(bytes) => {
                let s = String::from_utf8(bytes)
                    .map_err(|e| PlatformError::Storage(format!("Invalid UTF-8: {}", e)))?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, table: &str, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO kv_store (table_name, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params![table, key, value],
            )
            .map_err(|e| PlatformError::Storage(format!("Failed to insert: {}", e)))?;
        Ok(())
    }

    async fn set_string(&self, table: &str, key: &str, value: &str) -> Result<(), PlatformError> {
        self.set(table, key, value.as_bytes()).await
    }

    async fn delete(&self, table: &str, key: &str) -> Result<(), PlatformError> {
        self.conn
            .execute(
                "DELETE FROM kv_store WHERE table_name = ?1 AND key = ?2",
                [table, key],
            )
            .map_err(|e| PlatformError::Storage(format!("Failed to delete: {}", e)))?;
        Ok(())
    }

    async fn list_keys(&self, table: &str) -> Result<Vec<String>, PlatformError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT key FROM kv_store WHERE table_name = ?1")
            .map_err(|e| PlatformError::Storage(format!("Failed to prepare query: {}", e)))?;

        let keys = stmt
            .query_map([table], |row| row.get::<_, String>(0))
            .map_err(|e| PlatformError::Storage(format!("Failed to query: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| PlatformError::Storage(format!("Failed to collect keys: {}", e)))?;

        Ok(keys)
    }
}
