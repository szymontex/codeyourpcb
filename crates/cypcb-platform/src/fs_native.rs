use crate::error::PlatformError;
use crate::fs::{FileHandle, FileSystem};
use async_trait::async_trait;
use std::path::PathBuf;

/// Native filesystem handle with path access.
///
/// Stores the full path to a file on the native filesystem.
#[derive(Debug, Clone)]
pub struct NativeHandle {
    path: PathBuf,
    name: String,
}

impl NativeHandle {
    /// Create a new native handle from a path.
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        NativeHandle { path, name }
    }

    /// Get the full path to the file (native-only, not on FileHandle trait).
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl FileHandle for NativeHandle {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Native filesystem implementation using rfd and tokio::fs.
///
/// Uses platform-native file dialogs (rfd) and async file I/O (tokio::fs).
#[derive(Debug, Clone, Copy)]
pub struct NativeFileSystem;

#[async_trait(?Send)]
impl FileSystem for NativeFileSystem {
    type Handle = NativeHandle;

    async fn pick_file(
        &self,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError> {
        #[cfg(feature = "native-dialogs")]
        {
            let mut dialog = rfd::AsyncFileDialog::new();

            // Add file filters
            for (description, extensions) in filters {
                dialog = dialog.add_filter(*description, extensions);
            }

            match dialog.pick_file().await {
                Some(handle) => Ok(Some(NativeHandle::new(handle.path().to_path_buf()))),
                None => Ok(None), // User cancelled
            }
        }

        #[cfg(not(feature = "native-dialogs"))]
        {
            let _ = filters; // Suppress unused warning
            Err(PlatformError::NotSupported(
                "File dialogs require 'native-dialogs' feature. \
                 On Linux, install pkg-config and GTK3 development libraries."
                    .to_string(),
            ))
        }
    }

    async fn pick_save_file(
        &self,
        default_name: &str,
        filters: &[(&str, &[&str])],
    ) -> Result<Option<Self::Handle>, PlatformError> {
        #[cfg(feature = "native-dialogs")]
        {
            let mut dialog = rfd::AsyncFileDialog::new().set_file_name(default_name);

            // Add file filters
            for (description, extensions) in filters {
                dialog = dialog.add_filter(*description, extensions);
            }

            match dialog.save_file().await {
                Some(handle) => Ok(Some(NativeHandle::new(handle.path().to_path_buf()))),
                None => Ok(None), // User cancelled
            }
        }

        #[cfg(not(feature = "native-dialogs"))]
        {
            let _ = (default_name, filters); // Suppress unused warning
            Err(PlatformError::NotSupported(
                "File dialogs require 'native-dialogs' feature. \
                 On Linux, install pkg-config and GTK3 development libraries."
                    .to_string(),
            ))
        }
    }

    async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, PlatformError> {
        tokio::fs::read(&handle.path)
            .await
            .map_err(PlatformError::Io)
    }

    async fn read_string(&self, handle: &Self::Handle) -> Result<String, PlatformError> {
        tokio::fs::read_to_string(&handle.path)
            .await
            .map_err(PlatformError::Io)
    }

    async fn write(&self, handle: &Self::Handle, data: &[u8]) -> Result<(), PlatformError> {
        tokio::fs::write(&handle.path, data)
            .await
            .map_err(PlatformError::Io)
    }
}
