// Prevent simultaneous desktop and web features
#[cfg(all(feature = "desktop", feature = "web"))]
compile_error!("Features 'desktop' and 'web' cannot be enabled simultaneously");

pub mod error;
pub mod fs;
pub mod dialog;
pub mod storage;
pub mod menu;
pub mod platform;

// Conditional compilation for filesystem implementation
#[cfg_attr(wasm, path = "fs_web.rs")]
#[cfg_attr(native, path = "fs_native.rs")]
mod fs_impl;

// Conditional compilation for storage implementation
#[cfg_attr(wasm, path = "storage_web.rs")]
#[cfg_attr(native, path = "storage_native.rs")]
mod storage_impl;

// Re-exports
pub use error::PlatformError;
pub use fs::{FileHandle, FileSystem};
pub use fs_impl::*;
pub use dialog::Dialog;
pub use storage::Storage;
pub use storage_impl::*;
pub use menu::{MenuBar, Menu, MenuItem};
pub use platform::Platform;
