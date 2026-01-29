/// Platform facade providing unified access to all platform services.
///
/// This is the single entry point for application code to access platform abstractions.
/// Application code should never import platform-specific types directly (NativeFileSystem,
/// SqliteStorage, etc.) - instead, use the Platform struct which provides the right
/// implementation for the current platform.
///
/// # Platform Implementations
///
/// ## Native (desktop)
/// ```no_run
/// use cypcb_platform::Platform;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let platform = Platform::new_native(Path::new("./app_data"))?;
///
/// // Use filesystem
/// let handle = platform.fs().pick_file(None, None).await?;
///
/// // Use storage
/// platform.storage().init().await?;
/// platform.storage().set_string("settings", "theme", "dark").await?;
///
/// // Use dialog
/// if platform.dialog().confirm("Confirm", "Delete file?").await? {
///     // proceed...
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Web (WASM)
/// ```no_run
/// use cypcb_platform::Platform;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let platform = Platform::new_web("my-app-db")?;
///
/// // Same API as native - platform differences are hidden
/// let handle = platform.fs().pick_file(None, None).await?;
/// platform.storage().set_string("settings", "theme", "dark").await?;
/// # Ok(())
/// # }
/// ```
use crate::dialog::Dialog;
use crate::error::PlatformError;
use crate::fs::FileSystem;
use crate::storage::Storage;

// Native platform imports
#[cfg(native)]
use crate::fs_impl::NativeFileSystem;
#[cfg(native)]
use crate::storage_impl::SqliteStorage;

// Web platform imports
#[cfg(wasm)]
use crate::fs_impl::WebFileSystem;
#[cfg(wasm)]
use crate::storage_impl::WebStorageImpl;

/// Platform facade aggregating all platform services.
///
/// Provides access to:
/// - FileSystem: Cross-platform file operations
/// - Dialog: Native/web dialogs
/// - Storage: Persistent key-value storage
///
/// The Platform struct uses conditional compilation to provide the right
/// implementation for each platform without requiring application code to
/// handle platform differences.
#[cfg(native)]
pub struct Platform {
    fs: NativeFileSystem,
    dialog: Dialog,
    storage: SqliteStorage,
}

#[cfg(wasm)]
pub struct Platform {
    fs: WebFileSystem,
    dialog: Dialog,
    storage: WebStorageImpl,
}

// Native implementation
#[cfg(native)]
impl Platform {
    /// Create a new Platform instance for native (desktop) platforms.
    ///
    /// # Arguments
    /// * `storage_path` - Directory path for SQLite database file
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Platform;
    /// use std::path::Path;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let platform = Platform::new_native(Path::new("./app_data"))?;
    /// platform.storage().init().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_native(storage_path: &std::path::Path) -> Result<Self, PlatformError> {
        let fs = NativeFileSystem;
        let dialog = Dialog;
        let storage = SqliteStorage::new(storage_path)?;

        Ok(Self {
            fs,
            dialog,
            storage,
        })
    }

    /// Get a reference to the FileSystem implementation.
    ///
    /// Returns the native filesystem implementation backed by tokio::fs and rfd.
    pub fn fs(&self) -> &NativeFileSystem {
        &self.fs
    }

    /// Get a reference to the Dialog implementation.
    ///
    /// Returns the dialog implementation backed by rfd.
    pub fn dialog(&self) -> &Dialog {
        &self.dialog
    }

    /// Get a reference to the Storage implementation.
    ///
    /// Returns the SQLite storage implementation.
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }
}

// Web implementation
#[cfg(wasm)]
impl Platform {
    /// Create a new Platform instance for web (WASM) platforms.
    ///
    /// # Arguments
    /// * `db_name` - Name for the localStorage namespace
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Platform;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let platform = Platform::new_web("my-app")?;
    /// platform.storage().init().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_web(db_name: &str) -> Result<Self, PlatformError> {
        let fs = WebFileSystem;
        let dialog = Dialog;
        let storage = WebStorageImpl::new()?;

        // Note: db_name parameter reserved for future IndexedDB upgrade
        // Currently using localStorage which doesn't need explicit DB names
        let _ = db_name;

        Ok(Self {
            fs,
            dialog,
            storage,
        })
    }

    /// Get a reference to the FileSystem implementation.
    ///
    /// Returns the web filesystem implementation backed by rfd's File System Access API.
    pub fn fs(&self) -> &WebFileSystem {
        &self.fs
    }

    /// Get a reference to the Dialog implementation.
    ///
    /// Returns the dialog implementation backed by rfd's web dialogs.
    pub fn dialog(&self) -> &Dialog {
        &self.dialog
    }

    /// Get a reference to the Storage implementation.
    ///
    /// Returns the localStorage storage implementation.
    pub fn storage(&self) -> &WebStorageImpl {
        &self.storage
    }
}
