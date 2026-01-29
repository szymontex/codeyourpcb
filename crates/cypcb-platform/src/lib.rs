// Prevent simultaneous desktop and web features
#[cfg(all(feature = "desktop", feature = "web"))]
compile_error!("Features 'desktop' and 'web' cannot be enabled simultaneously");

pub mod error;
pub mod fs;

// Conditional compilation for filesystem implementation
#[cfg_attr(wasm, path = "fs_web.rs")]
#[cfg_attr(native, path = "fs_native.rs")]
mod fs_impl;

// Re-exports
pub use error::PlatformError;
pub use fs::{FileHandle, FileSystem};
pub use fs_impl::*;
