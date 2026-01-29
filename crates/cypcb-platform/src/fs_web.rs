use crate::error::PlatformError;
use crate::fs::{FileHandle, FileSystem};
use async_trait::async_trait;

/// Web filesystem handle wrapping an rfd FileHandle.
///
/// In WASM/web environment, we use rfd which provides cross-platform
/// file access. The rfd FileHandle abstracts over File System Access API
/// and fallback mechanisms.
#[derive(Debug, Clone)]
pub struct WebHandle {
    name: String,
    inner: rfd::FileHandle,
}

impl WebHandle {
    /// Create a new web handle from an rfd FileHandle.
    pub fn new(inner: rfd::FileHandle) -> Self {
        let name = inner.file_name();
        WebHandle { name, inner }
    }

    /// Get the inner rfd FileHandle (for internal use).
    pub fn inner(&self) -> &rfd::FileHandle {
        &self.inner
    }
}

impl FileHandle for WebHandle {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Web filesystem implementation using rfd.
///
/// Uses rfd's WASM support which provides:
/// - File System Access API when available (Chrome/Edge)
/// - Fallback to input element for file picking
/// - Blob download for file saving
#[derive(Debug, Clone, Copy)]
pub struct WebFileSystem;

#[async_trait(?Send)] // Single-threaded WASM environment
impl FileSystem for WebFileSystem {
    type Handle = WebHandle;

    async fn pick_file(
        &self,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError> {
        let mut dialog = rfd::AsyncFileDialog::new();

        // Add file filters
        for (description, extensions) in filters {
            dialog = dialog.add_filter(*description, extensions);
        }

        match dialog.pick_file().await {
            Some(handle) => Ok(Some(WebHandle::new(handle))),
            None => Ok(None), // User cancelled
        }
    }

    async fn pick_save_file(
        &self,
        default_name: &str,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError> {
        let mut dialog = rfd::AsyncFileDialog::new().set_file_name(default_name);

        // Add file filters
        for (description, extensions) in filters {
            dialog = dialog.add_filter(*description, extensions);
        }

        match dialog.save_file().await {
            Some(handle) => Ok(Some(WebHandle::new(handle))),
            None => Ok(None), // User cancelled
        }
    }

    async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, PlatformError> {
        Ok(handle.inner.read().await)
    }

    async fn read_string(&self, handle: &Self::Handle) -> Result<String, PlatformError> {
        let bytes = self.read(handle).await?;
        String::from_utf8(bytes).map_err(|e| {
            PlatformError::Web(format!("File is not valid UTF-8: {}", e))
        })
    }

    async fn write(&self, handle: &Self::Handle, data: &[u8]) -> Result<(), PlatformError> {
        handle
            .inner
            .write(data)
            .await
            .map_err(|e| PlatformError::Web(format!("Failed to write file: {:?}", e)))
    }
}
