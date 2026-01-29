use cypcb_platform::{MenuBar, Menu, MenuItem as PlatformMenuItem};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

/// Create the application menu structure using the platform MenuBar data model.
pub fn create_app_menu() -> MenuBar {
    MenuBar::new()
        .add_menu(
            Menu::new("File")
                .add_item(PlatformMenuItem::action("file.new", "New").with_shortcut("Ctrl+N"))
                .add_item(PlatformMenuItem::action("file.open", "Open...").with_shortcut("Ctrl+O"))
                .separator()
                .add_item(PlatformMenuItem::action("file.save", "Save").with_shortcut("Ctrl+S"))
                .add_item(PlatformMenuItem::action("file.save_as", "Save As...").with_shortcut("Ctrl+Shift+S"))
                .separator()
                .add_item(PlatformMenuItem::action("file.quit", "Quit").with_shortcut("Ctrl+Q"))
        )
        .add_menu(
            Menu::new("Edit")
                .add_item(PlatformMenuItem::action("edit.undo", "Undo").with_shortcut("Ctrl+Z"))
                .add_item(PlatformMenuItem::action("edit.redo", "Redo").with_shortcut("Ctrl+Shift+Z"))
                .separator()
                .add_item(PlatformMenuItem::action("edit.cut", "Cut").with_shortcut("Ctrl+X"))
                .add_item(PlatformMenuItem::action("edit.copy", "Copy").with_shortcut("Ctrl+C"))
                .add_item(PlatformMenuItem::action("edit.paste", "Paste").with_shortcut("Ctrl+V"))
        )
        .add_menu(
            Menu::new("View")
                .add_item(PlatformMenuItem::action("view.zoom_in", "Zoom In").with_shortcut("Ctrl+="))
                .add_item(PlatformMenuItem::action("view.zoom_out", "Zoom Out").with_shortcut("Ctrl+-"))
                .add_item(PlatformMenuItem::action("view.fit", "Fit to Window").with_shortcut("Ctrl+0"))
                .separator()
                .add_item(PlatformMenuItem::action("view.fullscreen", "Toggle Fullscreen").with_shortcut("F11"))
                .separator()
                .add_item(PlatformMenuItem::action("view.theme", "Toggle Theme").with_shortcut("Ctrl+Shift+T"))
        )
        .add_menu(
            Menu::new("Help")
                .add_item(PlatformMenuItem::action("help.about", "About CodeYourPCB"))
        )
}

/// Convert the platform MenuBar into Tauri native menu.
///
/// Translates the declarative menu structure from `cypcb_platform::MenuBar` into
/// Tauri's native menu API, converting Ctrl+ shortcuts to CmdOrCtrl+ for cross-platform compatibility.
pub fn build_tauri_menu(app: &tauri::App, menu_bar: &MenuBar) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let mut builder = MenuBuilder::new(app);

    for menu in &menu_bar.items {
        let submenu = build_submenu(app, menu)?;
        builder = builder.item(&submenu);
    }

    builder.build()
}

/// Recursively build a submenu from the platform Menu data model.
fn build_submenu(app: &tauri::App, menu: &Menu) -> tauri::Result<tauri::menu::Submenu<tauri::Wry>> {
    let mut sub = SubmenuBuilder::new(app, &menu.label);

    for item in &menu.items {
        match item {
            PlatformMenuItem::Action { id, label, shortcut, enabled } => {
                // Translate Ctrl+ to CmdOrCtrl+ for cross-platform compatibility
                let accelerator = shortcut.as_ref().map(|s| s.replace("Ctrl+", "CmdOrCtrl+"));

                let mi = MenuItemBuilder::with_id(id, label)
                    .enabled(*enabled)
                    .accelerator(accelerator.as_deref().unwrap_or(""))
                    .build(app)?;

                sub = sub.item(&mi);
            }
            PlatformMenuItem::Separator => {
                sub = sub.separator();
            }
            PlatformMenuItem::Submenu(nested) => {
                let nested_sub = build_submenu(app, nested)?;
                sub = sub.item(&nested_sub);
            }
        }
    }

    sub.build()
}

/// Handle menu events - quit, fullscreen, or emit to frontend.
pub fn handle_menu_event(app_handle: &tauri::AppHandle, event: &tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "file.quit" => {
            app_handle.exit(0);
        }
        "view.fullscreen" => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.is_fullscreen().and_then(|is_fullscreen| {
                    window.set_fullscreen(!is_fullscreen)
                });
            }
        }
        id => {
            // Emit all other menu actions to the frontend for handling
            let _ = app_handle.emit("menu-action", id);
        }
    }
}
