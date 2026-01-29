use tauri_plugin_dialog::DialogExt;

/// File content returned by open_file command.
#[derive(serde::Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
}

/// Open a .cypcb file via native file picker dialog.
///
/// Returns the file path and content if a file was selected, or None if cancelled.
#[tauri::command]
pub async fn open_file(app: tauri::AppHandle) -> Result<Option<FileContent>, String> {
    let file = app
        .dialog()
        .file()
        .add_filter("CodeYourPCB", &["cypcb"])
        .blocking_pick_file();

    match file {
        Some(path) => {
            let content = std::fs::read_to_string(path.path())
                .map_err(|e| format!("Failed to read file: {}", e))?;

            Ok(Some(FileContent {
                path: path.path().to_string_lossy().to_string(),
                content,
            }))
        }
        None => Ok(None),
    }
}

/// Save content to a specified file path.
///
/// Returns Ok on success, or error message on failure.
#[tauri::command]
pub async fn save_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write file: {}", e))
}

/// Save content via native save file picker dialog.
///
/// Returns the chosen file path if saved, or None if cancelled.
#[tauri::command]
pub async fn save_file_as(app: tauri::AppHandle, content: String) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .add_filter("CodeYourPCB", &["cypcb"])
        .blocking_save_file();

    match file {
        Some(path) => {
            std::fs::write(path.path(), content)
                .map_err(|e| format!("Failed to write file: {}", e))?;

            Ok(Some(path.path().to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}
