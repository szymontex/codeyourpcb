use serde::{Serialize, Deserialize};

/// Declarative menu bar structure for cross-platform menu rendering.
///
/// MenuBar represents the top-level menu structure that both desktop (Tauri native menus)
/// and web (HTML menus) can render from the same data model.
///
/// This is intentionally data-only - actual rendering is deferred to:
/// - Phase 12: Desktop native menu implementation
/// - Phase 13: Web HTML menu implementation
///
/// # Example
/// ```no_run
/// use cypcb_platform::{MenuBar, Menu, MenuItem};
///
/// let menu = MenuBar::new()
///     .add_menu(
///         Menu::new("File")
///             .add_item(MenuItem::action("new", "New")
///                 .with_shortcut("Ctrl+N"))
///             .add_item(MenuItem::action("open", "Open")
///                 .with_shortcut("Ctrl+O"))
///             .separator()
///             .add_item(MenuItem::action("quit", "Quit")
///                 .with_shortcut("Ctrl+Q"))
///     );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuBar {
    pub items: Vec<Menu>,
}

/// A top-level menu containing menu items.
///
/// Represents a single menu in the menu bar (e.g., "File", "Edit", "View").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Menu {
    pub label: String,
    pub items: Vec<MenuItem>,
}

/// A menu item within a menu.
///
/// Can be an action (clickable item), separator, or submenu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuItem {
    /// Clickable menu item that triggers an action.
    Action {
        /// Unique identifier for the action (e.g., "file.new", "edit.copy")
        id: String,
        /// Display label for the menu item
        label: String,
        /// Keyboard shortcut (e.g., "Ctrl+S", "Cmd+O")
        shortcut: Option<String>,
        /// Whether the item is currently enabled
        enabled: bool,
    },
    /// Visual separator between menu items
    Separator,
    /// Nested submenu
    Submenu(Menu),
}

impl MenuBar {
    /// Create a new empty menu bar.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add a menu to the menu bar (builder pattern).
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::{MenuBar, Menu};
    ///
    /// let menu_bar = MenuBar::new()
    ///     .add_menu(Menu::new("File"))
    ///     .add_menu(Menu::new("Edit"));
    /// ```
    pub fn add_menu(mut self, menu: Menu) -> Self {
        self.items.push(menu);
        self
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    /// Create a new menu with the given label.
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::Menu;
    ///
    /// let file_menu = Menu::new("File");
    /// ```
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Add a menu item to this menu (builder pattern).
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::{Menu, MenuItem};
    ///
    /// let menu = Menu::new("File")
    ///     .add_item(MenuItem::action("new", "New"));
    /// ```
    pub fn add_item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add a separator to this menu (builder pattern).
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::{Menu, MenuItem};
    ///
    /// let menu = Menu::new("File")
    ///     .add_item(MenuItem::action("new", "New"))
    ///     .separator()
    ///     .add_item(MenuItem::action("quit", "Quit"));
    /// ```
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::Separator);
        self
    }
}

impl MenuItem {
    /// Create an action menu item.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the action
    /// * `label` - Display text for the menu item
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::MenuItem;
    ///
    /// let item = MenuItem::action("file.save", "Save");
    /// ```
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            enabled: true,
        }
    }

    /// Add a keyboard shortcut to an action item (builder pattern).
    ///
    /// Shortcuts should use platform-agnostic format (e.g., "Ctrl+S").
    /// Platform-specific rendering code will convert to native format (Cmd+S on macOS).
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::MenuItem;
    ///
    /// let item = MenuItem::action("file.save", "Save")
    ///     .with_shortcut("Ctrl+S");
    /// ```
    pub fn with_shortcut(self, shortcut: impl Into<String>) -> Self {
        match self {
            Self::Action { id, label, enabled, .. } => Self::Action {
                id,
                label,
                shortcut: Some(shortcut.into()),
                enabled,
            },
            other => other,
        }
    }

    /// Mark an action item as disabled (builder pattern).
    ///
    /// Disabled items are shown but not clickable.
    ///
    /// # Example
    /// ```no_run
    /// use cypcb_platform::MenuItem;
    ///
    /// let item = MenuItem::action("file.save", "Save")
    ///     .disabled();
    /// ```
    pub fn disabled(self) -> Self {
        match self {
            Self::Action { id, label, shortcut, .. } => Self::Action {
                id,
                label,
                shortcut,
                enabled: false,
            },
            other => other,
        }
    }
}
