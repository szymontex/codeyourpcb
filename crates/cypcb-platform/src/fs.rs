use crate::error::PlatformError;
use async_trait::async_trait;

/// Platform-agnostic file handle.
///
/// Represents a file that can be read from or written to.
/// Does not expose platform-specific details like absolute paths (unavailable in WASM).
/// Note: No Send+Sync bounds to support single-threaded WASM environments.
pub trait FileHandle {
    /// Returns the name of the file (filename with extension, no path).
    fn name(&self) -> &str;
}

/// Platform-agnostic filesystem operations.
///
/// Provides async file I/O without exposing platform details.
/// Implementations exist for:
/// - Native (uses `rfd` + `tokio::fs`)
/// - Web/WASM (uses `rfd` with File System Access API fallback)
#[async_trait(?Send)] // ?Send for WASM compatibility (single-threaded)
pub trait FileSystem {
    /// The platform-specific file handle type.
    type Handle: FileHandle;

    /// Show a file picker dialog and return the selected file.
    ///
    /// # Arguments
    /// * `filters` - File type filters as (description, extensions) pairs.
    ///   Example: `&[("Text files", &["txt", "md"]), ("All files", &["*"])]`
    ///
    /// # Returns
    /// * `Ok(Some(handle))` - User selected a file
    /// * `Ok(None)` - User cancelled the dialog
    /// * `Err(_)` - Dialog failed to open or other error
    async fn pick_file(
        &self,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError>;

    /// Show a save file dialog and return the file to save to.
    ///
    /// # Arguments
    /// * `default_name` - Suggested filename (e.g., "design.cypcb")
    /// * `filters` - File type filters as (description, extensions) pairs
    ///
    /// # Returns
    /// * `Ok(Some(handle))` - User selected/confirmed a file path
    /// * `Ok(None)` - User cancelled the dialog
    /// * `Err(_)` - Dialog failed to open or other error
    async fn pick_save_file(
        &self,
        default_name: &str,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError>;

    /// Read the entire contents of a file as bytes.
    ///
    /// # Arguments
    /// * `handle` - File handle obtained from `pick_file` or `pick_save_file`
    ///
    /// # Returns
    /// * `Ok(bytes)` - File contents as raw bytes
    /// * `Err(_)` - File could not be read (permissions, deleted, etc.)
    async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, PlatformError>;

    /// Read the entire contents of a file as a UTF-8 string.
    ///
    /// # Arguments
    /// * `handle` - File handle obtained from `pick_file` or `pick_save_file`
    ///
    /// # Returns
    /// * `Ok(string)` - File contents as UTF-8 string
    /// * `Err(_)` - File could not be read or contained invalid UTF-8
    async fn read_string(&self, handle: &Self::Handle) -> Result<String, PlatformError>;

    /// Write bytes to a file, overwriting existing contents.
    ///
    /// # Arguments
    /// * `handle` - File handle obtained from `pick_save_file`
    /// * `data` - Bytes to write
    ///
    /// # Returns
    /// * `Ok(())` - File was written successfully
    /// * `Err(_)` - Write failed (permissions, disk full, etc.)
    async fn write(&self, handle: &Self::Handle, data: &[u8]) -> Result<(), PlatformError>;
}
