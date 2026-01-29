use crate::error::PlatformError;
use crate::storage::Storage;
use async_trait::async_trait;
use web_sys::Storage as WebStorage;

/// localStorage-based storage for web platforms.
///
/// Uses browser localStorage with `{table}::{key}` prefixing for namespacing.
/// localStorage has ~5MB quota (browser-dependent).
///
/// # Future: IndexedDB Upgrade Path
/// When library management (Phase 10) requires >5MB storage, this can be upgraded to IndexedDB
/// while maintaining the same Storage trait interface. localStorage is simpler for v1.1.
///
/// # Example
/// ```no_run
/// use cypcb_platform::WebStorageImpl;
///
/// # async fn example() {
/// let storage = WebStorageImpl::new().unwrap();
/// storage.init().await.unwrap();
/// # }
/// ```
pub struct WebStorageImpl {
    storage: WebStorage,
}

impl WebStorageImpl {
    /// Create a new web storage backend using browser localStorage.
    ///
    /// # Returns
    /// * `Ok(storage)` - Storage backend ready for use
    /// * `Err(_)` - localStorage not available (private browsing, browser too old)
    pub fn new() -> Result<Self, PlatformError> {
        let window = web_sys::window().ok_or_else(|| {
            PlatformError::Web("No window object (not running in browser)".to_string())
        })?;

        let storage = window.local_storage()
            .map_err(|e| PlatformError::Web(format!("Failed to access localStorage: {:?}", e)))?
            .ok_or_else(|| {
                PlatformError::NotSupported(
                    "localStorage not available (private browsing?)".to_string()
                )
            })?;

        Ok(Self { storage })
    }

    /// Generate the prefixed key for localStorage.
    ///
    /// Format: `{table}::{key}`
    fn prefixed_key(table: &str, key: &str) -> String {
        format!("{}::{}", table, key)
    }
}

#[async_trait(?Send)]
impl Storage for WebStorageImpl {
    async fn init(&self) -> Result<(), PlatformError> {
        // localStorage is always ready, no initialization needed
        Ok(())
    }

    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let prefixed = Self::prefixed_key(table, key);

        match self.storage.get_item(&prefixed) {
            Ok(Some(value)) => {
                // localStorage stores strings. We base64-encode binary data.
                // For now, assume values are UTF-8 strings (v1.1 use case: settings, preferences).
                // When binary storage needed (libraries), add base64 encoding.
                Ok(Some(value.into_bytes()))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(PlatformError::Web(format!(
                "Failed to get item from localStorage: {:?}",
                e
            ))),
        }
    }

    async fn get_string(&self, table: &str, key: &str) -> Result<Option<String>, PlatformError> {
        let prefixed = Self::prefixed_key(table, key);

        match self.storage.get_item(&prefixed) {
            Ok(value) => Ok(value),
            Err(e) => Err(PlatformError::Web(format!(
                "Failed to get item from localStorage: {:?}",
                e
            ))),
        }
    }

    async fn set(&self, table: &str, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        let prefixed = Self::prefixed_key(table, key);

        // For v1.1, assume binary data is actually UTF-8 text
        // When true binary storage needed, add base64 encoding
        let value_str = String::from_utf8(value.to_vec()).map_err(|e| {
            PlatformError::Storage(format!(
                "Binary storage not yet implemented (UTF-8 only): {}",
                e
            ))
        })?;

        self.storage
            .set_item(&prefixed, &value_str)
            .map_err(|e| {
                // Common error: QuotaExceededError when localStorage full (~5MB)
                PlatformError::Storage(format!("Failed to set item in localStorage: {:?}", e))
            })?;

        Ok(())
    }

    async fn set_string(&self, table: &str, key: &str, value: &str) -> Result<(), PlatformError> {
        let prefixed = Self::prefixed_key(table, key);

        self.storage
            .set_item(&prefixed, value)
            .map_err(|e| {
                PlatformError::Storage(format!("Failed to set item in localStorage: {:?}", e))
            })?;

        Ok(())
    }

    async fn delete(&self, table: &str, key: &str) -> Result<(), PlatformError> {
        let prefixed = Self::prefixed_key(table, key);

        self.storage.remove_item(&prefixed).map_err(|e| {
            PlatformError::Storage(format!("Failed to remove item from localStorage: {:?}", e))
        })?;

        Ok(())
    }

    async fn list_keys(&self, table: &str) -> Result<Vec<String>, PlatformError> {
        let prefix = format!("{}::", table);
        let mut keys = Vec::new();

        // Iterate through all localStorage keys
        let length = self
            .storage
            .length()
            .map_err(|e| PlatformError::Web(format!("Failed to get localStorage length: {:?}", e)))?;

        for i in 0..length {
            if let Ok(Some(full_key)) = self.storage.key(i) {
                if full_key.starts_with(&prefix) {
                    // Strip the prefix to get the original key
                    let key = full_key[prefix.len()..].to_string();
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }
}
