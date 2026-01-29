mod menu;
pub mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Build and set the native menu
            let menu_bar = menu::create_app_menu();
            let tauri_menu = menu::build_tauri_menu(app, &menu_bar)?;
            app.set_menu(tauri_menu)?;

            // Register menu event handler
            app.on_menu_event(|app_handle, event| {
                menu::handle_menu_event(app_handle, &event);
            });

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
