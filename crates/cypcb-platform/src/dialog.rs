use crate::error::PlatformError;
use std::path::PathBuf;

/// Dialog wrapper providing cross-platform message and file dialogs.
///
/// Uses rfd internally, which handles platform differences automatically:
/// - Desktop: Native OS dialogs (GTK3/Windows/macOS)
/// - Web: HTML-based dialogs
///
/// Note: On native platforms without GUI libraries (headless CI), dialog
/// methods will return NotSupported errors unless the `desktop` feature is enabled.
pub struct Dialog;

#[cfg(not(any(feature = "desktop", target_arch = "wasm32")))]
impl Dialog {
    /// Show an informational alert dialog.
    pub async fn alert(_title: &str, _message: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "Dialog support requires 'desktop' feature or WASM target".to_string(),
        ))
    }

    /// Show a confirmation dialog with Yes/No buttons.
    pub async fn confirm(_title: &str, _message: &str) -> Result<bool, PlatformError> {
        Err(PlatformError::NotSupported(
            "Dialog support requires 'desktop' feature or WASM target".to_string(),
        ))
    }

    /// Show a folder picker dialog.
    pub async fn pick_folder() -> Result<Option<PathBuf>, PlatformError> {
        Err(PlatformError::NotSupported(
            "Dialog support requires 'desktop' feature or WASM target".to_string(),
        ))
    }
}

#[cfg(any(feature = "desktop", target_arch = "wasm32"))]
impl Dialog {
    /// Show an informational alert dialog.
    ///
    /// # Arguments
    /// * `title` - Dialog window title
    /// * `message` - Message content to display
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Dialog;
    ///
    /// # async fn example() {
    /// Dialog::alert("Info", "Operation completed successfully").await.ok();
    /// # }
    /// ```
    pub async fn alert(title: &str, message: &str) -> Result<(), PlatformError> {
        rfd::AsyncMessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd::MessageLevel::Info)
            .show()
            .await;
        Ok(())
    }

    /// Show a confirmation dialog with Yes/No buttons.
    ///
    /// # Arguments
    /// * `title` - Dialog window title
    /// * `message` - Question to ask the user
    ///
    /// # Returns
    /// `Ok(true)` if user clicked Yes, `Ok(false)` if user clicked No
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Dialog;
    ///
    /// # async fn example() {
    /// let confirmed = Dialog::confirm("Confirm", "Delete this file?").await.unwrap();
    /// if confirmed {
    ///     // Proceed with deletion
    /// }
    /// # }
    /// ```
    pub async fn confirm(title: &str, message: &str) -> Result<bool, PlatformError> {
        let result = rfd::AsyncMessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            .await;

        Ok(result == rfd::MessageDialogResult::Yes)
    }

    /// Show a folder picker dialog.
    ///
    /// # Returns
    /// `Ok(Some(path))` if user selected a folder, `Ok(None)` if cancelled
    ///
    /// # Platform Notes
    /// - Desktop: Uses native folder picker
    /// - Web: Not supported. Always returns `Err(PlatformError::NotSupported)`.
    ///   Web apps should use file pickers or directory handles API directly.
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Dialog;
    ///
    /// # async fn example() {
    /// # #[cfg(not(target_arch = "wasm32"))]
    /// if let Some(folder) = Dialog::pick_folder().await.unwrap() {
    ///     println!("Selected: {:?}", folder);
    /// }
    /// # }
    /// ```
    pub async fn pick_folder() -> Result<Option<PathBuf>, PlatformError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let handle = rfd::AsyncFileDialog::new()
                .pick_folder()
                .await;

            Ok(handle.map(|h| h.path().to_path_buf()))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err(PlatformError::NotSupported(
                "Folder picking not supported in web browsers".to_string(),
            ))
        }
    }
}
