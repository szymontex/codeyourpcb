use crate::error::PlatformError;
use async_trait::async_trait;

/// Platform-agnostic key-value storage with table namespacing.
///
/// Provides persistent storage abstraction over:
/// - Native: SQLite database
/// - Web: Browser localStorage (IndexedDB upgrade path documented for v1.2+)
///
/// Storage is organized into tables (namespaces) with string keys and byte values.
/// Tables are created implicitly on first use.
///
/// # Example
/// ```no_run
/// use cypcb_platform::Storage;
///
/// # async fn example(storage: impl Storage) {
/// // Initialize storage
/// storage.init().await.unwrap();
///
/// // Store preferences in "settings" table
/// storage.set_string("settings", "theme", "dark").await.unwrap();
///
/// // Retrieve preferences
/// if let Some(theme) = storage.get_string("settings", "theme").await.unwrap() {
///     println!("Theme: {}", theme);
/// }
///
/// // List all keys in a table
/// let keys = storage.list_keys("settings").await.unwrap();
/// # }
/// ```
#[async_trait(?Send)]
pub trait Storage {
    /// Initialize the storage backend.
    ///
    /// Must be called before any other operations. Idempotent (safe to call multiple times).
    ///
    /// - Native: Creates SQLite database file and schema if needed
    /// - Web: No-op (localStorage always available)
    async fn init(&self) -> Result<(), PlatformError>;

    /// Retrieve a value by key from a table.
    ///
    /// # Arguments
    /// * `table` - Namespace for the key (e.g., "settings", "cache", "projects")
    /// * `key` - Key to look up
    ///
    /// # Returns
    /// * `Ok(Some(value))` - Key exists, returns raw bytes
    /// * `Ok(None)` - Key does not exist
    /// * `Err(_)` - Storage error (I/O, permissions, etc.)
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>, PlatformError>;

    /// Retrieve a UTF-8 string value by key from a table.
    ///
    /// Convenience method for string values. Returns error if value exists but is not valid UTF-8.
    ///
    /// # Arguments
    /// * `table` - Namespace for the key
    /// * `key` - Key to look up
    ///
    /// # Returns
    /// * `Ok(Some(string))` - Key exists and value is valid UTF-8
    /// * `Ok(None)` - Key does not exist
    /// * `Err(_)` - Storage error or invalid UTF-8
    async fn get_string(&self, table: &str, key: &str) -> Result<Option<String>, PlatformError>;

    /// Store a value by key in a table.
    ///
    /// Creates or overwrites the key. Table is created implicitly if it doesn't exist.
    ///
    /// # Arguments
    /// * `table` - Namespace for the key
    /// * `key` - Key to store under
    /// * `value` - Raw bytes to store
    ///
    /// # Returns
    /// * `Ok(())` - Value stored successfully
    /// * `Err(_)` - Storage error (I/O, disk full, quota exceeded)
    async fn set(&self, table: &str, key: &str, value: &[u8]) -> Result<(), PlatformError>;

    /// Store a UTF-8 string value by key in a table.
    ///
    /// Convenience method for string values.
    ///
    /// # Arguments
    /// * `table` - Namespace for the key
    /// * `key` - Key to store under
    /// * `value` - String to store
    ///
    /// # Returns
    /// * `Ok(())` - Value stored successfully
    /// * `Err(_)` - Storage error
    async fn set_string(&self, table: &str, key: &str, value: &str) -> Result<(), PlatformError>;

    /// Delete a key from a table.
    ///
    /// Idempotent (succeeds even if key doesn't exist).
    ///
    /// # Arguments
    /// * `table` - Namespace containing the key
    /// * `key` - Key to delete
    ///
    /// # Returns
    /// * `Ok(())` - Key deleted (or didn't exist)
    /// * `Err(_)` - Storage error
    async fn delete(&self, table: &str, key: &str) -> Result<(), PlatformError>;

    /// List all keys in a table.
    ///
    /// Returns empty vector if table doesn't exist or is empty.
    ///
    /// # Arguments
    /// * `table` - Namespace to list keys from
    ///
    /// # Returns
    /// * `Ok(keys)` - Vector of all keys in the table (may be empty)
    /// * `Err(_)` - Storage error
    async fn list_keys(&self, table: &str) -> Result<Vec<String>, PlatformError>;
}
