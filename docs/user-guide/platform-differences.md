# Platform Differences: Desktop vs Web

CodeYourPCB is available as both a desktop application (Tauri-based) and a web application (browser-based). This guide explains the differences and helps you choose the right platform for your needs.

## Feature Comparison Table

| Feature | Desktop (Tauri) | Web (Browser) |
|---------|----------------|---------------|
| **Installation** | MSI/DMG/AppImage installer | No installation (browser) |
| **Offline Support** | Full offline support | Requires initial load |
| **File Access** | Native filesystem with OS dialogs | File System Access API (Chrome/Edge/Safari) + download fallback (Firefox) |
| **Storage** | SQLite database | localStorage (v1.1) |
| **Library Capacity** | Unlimited (disk-based) | ~5MB quota (browser-dependent) |
| **FreeRouting Integration** | Local JAR execution | Not available |
| **Autorouting** | Full FreeRouting support | Manual routing only |
| **File Dialogs** | Native OS dialogs (platform-specific) | Browser file picker |
| **Menus** | Native OS menu bar (top of screen on macOS, in-window on Windows/Linux) | HTML-based menu bar |
| **Keyboard Shortcuts** | OS-native (Cmd on macOS, Ctrl on Windows/Linux) | Browser shortcuts |
| **Theme Detection** | OS preference + manual toggle | OS preference + manual toggle |
| **Editor** | Monaco with full LSP features | Monaco with full LSP features |
| **Syntax Highlighting** | Yes (.cypcb Monarch) | Yes (.cypcb Monarch) |
| **Auto-completion** | Yes (keywords, properties, layers) | Yes (keywords, properties, layers) |
| **Inline Diagnostics** | Yes (parse errors, DRC warnings) | Yes (parse errors, DRC warnings) |
| **Hover Tooltips** | Yes (keyword documentation) | Yes (keyword documentation) |
| **3D Viewer** | Three.js WebGL rendering | Three.js WebGL rendering |
| **DRC Checks** | Full rule checking | Full rule checking |
| **Gerber Export** | Direct file write | Download to browser downloads folder |
| **DSN/SES Export** | Direct file write | Download to browser downloads folder |
| **Print/PDF** | Native print dialog | Browser print dialog |
| **Window Management** | Native OS window controls | Browser tabs and windows |
| **Performance** | Native speed (no WASM overhead) | WASM performance (near-native) |
| **Updates** | Manual installer updates | Instant (refresh browser) |
| **Security** | OS-level permissions | Browser sandboxing |

## Desktop Application

### Advantages

**1. Native File System Access**
- Open and save files directly without picker dialogs
- Recent files menu with full path access
- Drag-and-drop file loading
- Watch files for external changes

**2. FreeRouting Integration**
- Execute FreeRouting JAR locally
- Automated DSN → SES pipeline
- No upload/download steps
- Full autorouter capabilities

**3. Unlimited Storage**
- SQLite database on disk (no size limits)
- Import entire KiCad library (100,000+ components)
- Store 3D models and previews locally
- No browser quota restrictions

**4. Native OS Integration**
- File associations (.cypcb files open in app)
- Native menu bar (macOS top screen, Windows/Linux in-window)
- System dialogs (open, save, about)
- OS theme detection (dark mode)
- Native window controls (minimize, maximize, close)

**5. Better Performance**
- No WASM compilation overhead
- Native code execution
- Larger memory allocation
- Better multithreading (not single-threaded WASM)

**6. Offline Operation**
- No internet required after installation
- All features work offline
- Local library database
- Local FreeRouting execution

### Disadvantages

**1. Installation Required**
- Must download installer for platform (MSI, DMG, AppImage)
- Requires administrator privileges (Windows/macOS)
- Takes disk space (~50MB installed)

**2. Platform-Specific Builds**
- Separate installers for Windows, macOS, Linux
- Platform-specific bugs possible
- Update distribution more complex

**3. Manual Updates**
- Check for updates manually
- Download and install new versions
- Not instant like web app

### System Requirements

**Windows:**
- Windows 10 or later
- WebView2 runtime (usually pre-installed)
- 100MB disk space

**macOS:**
- macOS 10.15 (Catalina) or later
- 100MB disk space
- Unsigned app warning (requires Gatekeeper bypass)

**Linux:**
- GTK3 libraries (libgtk-3-0)
- WebKit2GTK (libwebkit2gtk-4.1-0)
- 100MB disk space
- Distribution-specific dependencies

### Installation

**Windows:**
```bash
# Download codeyourpcb-setup.msi
# Double-click to install
# Follow installer wizard
```

**macOS:**
```bash
# Download CodeYourPCB.dmg
# Open DMG and drag app to Applications
# Right-click app → Open (first launch only, bypass Gatekeeper)
```

**Linux:**
```bash
# Download codeyourpcb.AppImage
chmod +x codeyourpcb.AppImage
./codeyourpcb.AppImage
```

## Web Application

### Advantages

**1. No Installation**
- Open URL in browser
- Instant access
- No administrator privileges required
- No disk space used

**2. Instant Updates**
- Always latest version
- Refresh browser to update
- No installer downloads
- Automatic deployment

**3. Cross-Platform**
- Works on any OS with modern browser
- Same experience everywhere
- No platform-specific bugs
- Consistent rendering

**4. Easy Sharing**
- Share URL for immediate access
- No "install this app first" friction
- Demo and onboarding simpler
- Collaboration via URL state sharing

**5. Browser Security**
- Sandboxed execution
- No system-wide permissions
- Browser handles security updates
- Lower malware risk

### Disadvantages

**1. Requires Internet (Initial Load)**
- Need connection to load app first time
- Cached after first visit (works offline after)
- Library updates require connection

**2. No FreeRouting**
- Autorouting not available in browser
- Must route manually or export DSN and use desktop
- No automated routing pipeline

**3. Storage Limitations**
- localStorage ~5MB quota (browser-dependent)
- Can't import entire KiCad library
- Limited to essential components
- IndexedDB upgrade planned (Phase 16+)

**4. File Access Quirks**
- Firefox requires download fallback (no File System Access API)
- Can't watch files for external changes
- Recent files limited to handles browser stored
- File picker required for each open

**5. Browser Compatibility**
- Best in Chrome/Edge (File System Access API)
- Safari works but limited features
- Firefox requires download workaround
- Older browsers unsupported

### Browser Requirements

**Recommended:**
- Chrome 86+ or Edge 86+
- Firefox 90+ (with download fallback)
- Safari 15.2+

**Required Features:**
- WebAssembly support
- WebGL 2.0 (for 3D rendering)
- ES2020 JavaScript support
- localStorage API

**Optional Enhancements:**
- File System Access API (Chrome/Edge) for save-in-place
- Web Workers for background processing

## Shared Features

Both platforms share identical core functionality:

### Monaco Editor Integration
- Full-featured code editor
- .cypcb syntax highlighting (Monarch tokenizer)
- Auto-completion (keywords, properties, layers, units)
- Inline diagnostics (parse errors as red squiggles, DRC warnings as yellow)
- Hover tooltips (documentation for keywords)
- 300ms debounced live preview
- Draggable divider for layout adjustment (200px min editor, 30% min canvas)

### LSP-Like Features (via WASM)
- Real-time diagnostics from WASM engine
- No backend server required
- Parse errors appear as editor markers
- DRC violations shown as warnings
- Same experience desktop and web

### 3D Viewer
- Three.js WebGL rendering
- Component outlines with shadows
- Board outline and dimensions
- Pan, zoom, rotate controls
- Mouse and touchpad support

### DRC (Design Rule Check)
- Clearance violations
- Trace width violations
- Drill size violations
- Minimum spacing checks
- Identical rules desktop and web

### Export Formats
- Gerber files (RS-274X)
- Drill files (Excellon)
- DSN files (Specctra)
- SES files (routing import)

### Theme Support
- Light and dark modes
- OS preference detection
- Manual toggle (Ctrl+T / Cmd+T)
- Monaco editor theme sync
- Canvas theme awareness
- PCB electrical colors unchanged (copper red/blue)

## File Operations Comparison

### Desktop: Native Filesystem

**Open file:**
```
File → Open (Ctrl+O / Cmd+O)
→ Native OS dialog
→ Select .cypcb file
→ File loaded, path remembered
```

**Save file:**
```
File → Save (Ctrl+S / Cmd+S)
→ Direct write to original path
→ No dialog if file already saved
```

**Save as:**
```
File → Save As (Ctrl+Shift+S / Cmd+Shift+S)
→ Native OS dialog
→ Choose new location
→ File written
```

### Web: File System Access API (Chrome/Edge)

**Open file:**
```
Open File button
→ Browser file picker
→ Select .cypcb file
→ FileSystemFileHandle stored
```

**Save file:**
```
Save button (Ctrl+S / Cmd+S)
→ No dialog (uses stored handle)
→ Direct write to original file
→ Permission prompt (first time only)
```

**Save as:**
```
Save As button (Ctrl+Shift+S)
→ Browser save dialog
→ Choose location
→ New FileSystemFileHandle stored
```

### Web: Download Fallback (Firefox)

**Open file:**
```
Open File button
→ Browser file picker
→ Select .cypcb file
→ File loaded (handle not stored)
```

**Save file:**
```
Save button (Ctrl+S / Cmd+S)
→ File downloads to browser downloads folder
→ User must move file manually
→ Every save is "save as"
```

## Storage Implementation

### Desktop: SQLite

**Location:**
- Linux: `~/.local/share/codeyourpcb/libraries.db`
- macOS: `~/Library/Application Support/codeyourpcb/libraries.db`
- Windows: `C:\Users\<user>\AppData\Local\codeyourpcb\libraries.db`

**Features:**
- Full SQL queries
- FTS5 full-text search
- Transactions
- ACID guarantees
- No size limits

**Schema:**
```sql
CREATE TABLE components (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    name TEXT NOT NULL,
    category TEXT,
    footprint_data TEXT NOT NULL,
    description TEXT,
    manufacturer TEXT,
    mpn TEXT,
    package TEXT,
    metadata_json TEXT,
    thumbnail BLOB,
    model_3d_path TEXT,
    UNIQUE(source, name)
);

CREATE VIRTUAL TABLE components_fts USING fts5(
    name, category, description, manufacturer, mpn, package,
    content='components',
    content_rowid='id'
);
```

### Web: localStorage

**Storage:**
- Key-value store in browser
- ~5MB quota (varies by browser)
- Synchronous API

**Limitations:**
- Can't store entire KiCad library
- No SQL queries or joins
- No full-text search ranking
- Must serialize/deserialize JSON

**Future:** Upgrade to IndexedDB for larger capacity and async API (Phase 16+).

## Menu Systems

### Desktop: Native OS Menus

**macOS:**
- Menu bar at top of screen (system-wide)
- Follows macOS Human Interface Guidelines
- Native keyboard shortcuts (Cmd+O, Cmd+S)
- About menu in app name menu

**Windows/Linux:**
- Menu bar inside application window
- Follows Windows/GTK conventions
- Native keyboard shortcuts (Ctrl+O, Ctrl+S)
- About menu in Help menu

**Implementation:**
- Tauri native menu builder
- IPC events to JavaScript
- Dispatches custom events to main app

### Web: HTML Menus

**Layout:**
- Fixed header with menu buttons
- Dropdown menus on click
- CSS styled to match theme

**Keyboard Shortcuts:**
- Implemented in JavaScript
- Same shortcuts as desktop (Ctrl+O, Ctrl+S)
- Browser shortcuts may conflict

**Implementation:**
- HTML/CSS/JavaScript
- Event listeners for menu items
- No IPC required (direct function calls)

## Performance Characteristics

### Desktop Performance

**Parsing:**
- Native Rust compilation
- No WASM overhead
- ~1ms for typical .cypcb file

**Rendering:**
- WebGL in native webview
- Full GPU acceleration
- 60 FPS canvas updates

**Memory:**
- Native memory allocation
- No 4GB WASM limit
- Can load large boards

### Web Performance

**Parsing:**
- WASM compilation (near-native)
- ~1-2ms for typical .cypcb file
- Single-threaded execution

**Rendering:**
- WebGL in browser
- Full GPU acceleration (same as desktop)
- 60 FPS canvas updates

**Memory:**
- WASM 32-bit address space
- 4GB theoretical limit (sufficient for PCB designs)
- Browser garbage collection for JavaScript heap

## When to Use Desktop vs Web

### Use Desktop If:

- You need **FreeRouting integration** for autorouting
- You work **offline frequently**
- You want to import **entire KiCad library** (100K+ components)
- You prefer **native OS integration** (file associations, system dialogs)
- You need **maximum performance** (no WASM overhead)
- You have **large, complex boards** with thousands of components
- You want **file watching** (detect external changes)

### Use Web If:

- You want **zero installation** (instant access)
- You're **sharing/demoing** designs (send URL)
- You work on **multiple machines** (no installation on each)
- You prefer **instant updates** (no installer downloads)
- You're on a **locked-down system** (can't install software)
- You have **simple 2-4 layer boards** (web storage sufficient)
- You're **learning** CodeYourPCB (lower barrier to entry)

### Use Both:

- Start with **web** for learning and prototyping
- Move to **desktop** when you need autorouting
- Use **desktop** at workstation, **web** when traveling
- **Demo in web**, **produce in desktop**

## Migration Path

**Web → Desktop:**
1. Download and install desktop app
2. Export .cypcb file from web (File → Save)
3. Open in desktop app
4. Import full library if needed
5. Continue working with all features

**Desktop → Web:**
1. Ensure .cypcb file is accessible
2. Open web app
3. Upload .cypcb file
4. Work continues (minus autorouting)
5. Download updated file when done

**Files are fully compatible** - same .cypcb format works on both platforms.

## Platform Detection (for Developers)

CodeYourPCB uses feature flags and runtime detection:

**Compile-time:**
```rust
#[cfg(feature = "desktop")]
use tauri::api::dialog;

#[cfg(feature = "web")]
use web_sys::FileSystemFileHandle;
```

**Runtime (JavaScript):**
```typescript
function isDesktop(): boolean {
    return typeof window.__TAURI__ !== 'undefined';
}

if (isDesktop()) {
    // Desktop-specific code
} else {
    // Web-specific code
}
```

## Known Limitations

### Desktop

- **Linux dependencies**: Requires GTK3/WebKit2GTK system libraries
- **Unsigned app**: macOS warns on first launch (requires Gatekeeper bypass)
- **Manual updates**: User must download and install new versions
- **Installer size**: ~40MB download, ~100MB installed

### Web

- **No autorouting**: FreeRouting requires local execution
- **Storage quota**: ~5MB localStorage limit
- **File API inconsistency**: Firefox lacks File System Access API
- **Browser compatibility**: Requires modern browser (Chrome 86+, Firefox 90+)
- **Initial load time**: Must download WASM and assets (cached after first visit)

## Summary

**Desktop:**
- Native speed and OS integration
- Full FreeRouting autorouting
- Unlimited library storage
- Best for production work

**Web:**
- Zero installation, instant access
- Cross-platform consistency
- Limited library size (~5MB)
- Best for learning and sharing

**Both platforms share:**
- Monaco editor with full LSP features
- 3D viewer with WebGL rendering
- DRC checks and export formats
- Theme support (light/dark modes)
- Identical .cypcb file format

Choose based on your workflow needs - both are fully capable PCB design tools.

For more information:
- [Getting Started](getting-started.md) - Learn to create .cypcb files
- [Library Management](library-management.md) - Import and search components
- [Project Structure](project-structure.md) - Organize files for version control
