mod menu;
pub mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Build and set the native menu
            let menu_bar = menu::create_app_menu();
            let tauri_menu = menu::build_tauri_menu(app, &menu_bar)?;
            app.set_menu(tauri_menu)?;

            // Register menu event handler
            app.on_menu_event(|app_handle, event| {
                menu::handle_menu_event(app_handle, &event);
            });

            // Handle file-opened events for .cypcb file association
            // Check CLI arguments for file paths when app is launched
            let args: Vec<String> = std::env::args().collect();
            for arg in args.iter().skip(1) {
                if arg.ends_with(".cypcb") {
                    if let Ok(content) = std::fs::read_to_string(arg) {
                        let _ = app.emit("file-opened", serde_json::json!({
                            "path": arg,
                            "content": content
                        }));
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::save_file,
            commands::save_file_as
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
