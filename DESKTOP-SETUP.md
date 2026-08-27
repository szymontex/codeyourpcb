# CodeYourPCB Desktop - Quick Start

**Zero configuration. Double-click and it runs.**

## First run

### Windows

1. **Setup (once):**
   ```
   Double-click: setup-windows.bat
   ```
   - Checks for Node.js (if missing, links to the installer)
   - Installs Rust automatically
   - Installs every dependency

2. **Run dev mode:**
   ```
   Double-click: dev-windows.bat
   ```

3. **Build the installer (.msi):**
   ```
   Double-click: build-windows.bat
   ```

### macOS

1. **Setup (once):**
   ```bash
   ./setup-macos.sh
   ```
   If double-clicking does nothing, open Terminal and paste the line above.

2. **Run dev mode:**
   ```bash
   ./dev-macos.sh
   ```

3. **Build the installer (.dmg):**
   ```bash
   ./build-macos.sh
   ```

### Linux (Ubuntu/Debian)

1. **Setup (once):**
   ```bash
   ./setup-linux.sh
   ```
   - Installs the GTK dependencies automatically (needs sudo)
   - Checks that Tauri compiles

2. **Run dev mode:**
   ```bash
   ./dev-linux.sh
   ```

3. **Build the installers (.deb + .rpm):**
   ```bash
   ./build-linux.sh
   ```
   The AppImage is not built by default: its bundler fails inside
   `linuxdeploy-plugin-gtk.sh`, a vendored script this project does not own, and
   Tauri reports the whole build as failed when one bundler dies. Ask for it with
   `CYPCB_APPIMAGE=1 ./build-linux.sh` once that script is fixed upstream.

## What a build leaves behind

Tauri names the file from `productName` and `version` in
`src-tauri/tauri.conf.json`, so raising the version changes the name. The
directories are fixed, and the build scripts list what they produced:

### Windows
- `target/release/bundle/msi/` - one `.msi`

### macOS
- `target/release/bundle/dmg/` - one `.dmg`

### Linux
- `target/release/bundle/deb/` - `.deb`, for a system install
- `target/release/bundle/rpm/` - `.rpm`, the same for RPM distributions
- `target/release/bundle/appimage/` - `.AppImage`, only with `CYPCB_APPIMAGE=1`

## If something does not work

### Windows - no Node.js
Download and install: https://nodejs.org/ (v20 LTS)

### macOS - no Node.js
```bash
# Option 1: Homebrew
brew install node

# Option 2: download from
https://nodejs.org/
```

### Linux - no Node.js
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

### Linux - GTK compilation errors
`setup-linux.sh` installs these automatically, but if something went wrong:
```bash
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config
```

## What there is to try

After `dev-*.bat` or `dev-*.sh` you get a window with:

- Native menu bar (File/Edit/View/Help)
- Keyboard shortcuts (Ctrl+O = Open, Ctrl+S = Save, and the rest)
- Native file dialogs (`.cypcb` files only)
- Window management (maximize, minimize, fullscreen)
- Theme toggle (Ctrl+Shift+T)

**Worth checking:**
1. File > Open - a native dialog filtered to `.cypcb`
2. File > Save - writes the file
3. View > Toggle Theme - dark and light
4. View > Toggle Fullscreen (F11)
5. Minimize and maximize through the native window controls

## System requirements

- **Windows:** 10/11
- **macOS:** 10.15+ (Catalina or newer)
- **Linux:** Ubuntu 20.04+, Debian 11+, or equivalent

## Reporting a problem

If something does not work:
1. Copy the terminal output
2. Say which setup step failed
3. Include the system details (OS, Node and Rust versions)

## Notes

- **The first build is slow** (10-20 minutes) - Rust compiles everything. The
  next ones are faster.
- **Dev mode** hot-reloads: change the code and the window refreshes.
- **A production build** starts faster than dev mode, because it is optimised.
