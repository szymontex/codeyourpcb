# CodeYourPCB Desktop - Quick Start

**Zero konfiguracji. Double-click i działa.**

## 🚀 Pierwsze uruchomienie

### Windows

1. **Setup (tylko raz):**
   ```
   Double-click: setup-windows.bat
   ```
   - Sprawdzi Node.js (jeśli brak → link do instalacji)
   - Zainstaluje Rust automatycznie
   - Zainstaluje wszystkie zależności

2. **Uruchom dev mode:**
   ```
   Double-click: dev-windows.bat
   ```

3. **Zbuduj instalator (.msi):**
   ```
   Double-click: build-windows.bat
   ```

### macOS

1. **Setup (tylko raz):**
   ```bash
   ./setup-macos.sh
   ```
   Jeśli nie działa double-click, otwórz Terminal i wklej powyższe.

2. **Uruchom dev mode:**
   ```bash
   ./dev-macos.sh
   ```

3. **Zbuduj instalator (.dmg):**
   ```bash
   ./build-macos.sh
   ```

### Linux (Ubuntu/Debian)

1. **Setup (tylko raz):**
   ```bash
   ./setup-linux.sh
   ```
   - Zainstaluje GTK dependencies automatycznie (wymaga sudo)
   - Sprawdzi kompilację Tauri

2. **Uruchom dev mode:**
   ```bash
   ./dev-linux.sh
   ```

3. **Zbuduj instalator (.AppImage + .deb):**
   ```bash
   ./build-linux.sh
   ```

## 📦 Co dostaniesz po build

### Windows
- `src-tauri/target/release/bundle/msi/CodeYourPCB_0.1.0_x64_en-US.msi`
- Instalator ~5-10MB

### macOS
- `src-tauri/target/release/bundle/dmg/CodeYourPCB_0.1.0_x64.dmg`
- Disk image ~8-12MB

### Linux
- `src-tauri/target/release/bundle/appimage/code-your-pcb_0.1.0_amd64.AppImage` (portable)
- `src-tauri/target/release/bundle/deb/code-your-pcb_0.1.0_amd64.deb` (instalacja systemowa)

## 🔧 Jeśli coś nie działa

### Windows - brak Node.js
Pobierz i zainstaluj: https://nodejs.org/ (v20 LTS)

### macOS - brak Node.js
```bash
# Opcja 1: Homebrew
brew install node

# Opcja 2: Pobierz z
https://nodejs.org/
```

### Linux - brak Node.js
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

### Linux - błędy kompilacji GTK
Skrypt `setup-linux.sh` instaluje to automatycznie, ale jeśli coś poszło nie tak:
```bash
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config
```

## 🎯 Co możesz testować

Po uruchomieniu `dev-*.bat/sh` zobaczysz okno z:

- ✅ Native menu bar (File/Edit/View/Help)
- ✅ Keyboard shortcuts (Ctrl+O = Open, Ctrl+S = Save, etc.)
- ✅ Native file dialogs (tylko .cypcb files)
- ✅ Window management (maximize, minimize, fullscreen)
- ✅ Theme toggle (Ctrl+Shift+T)

**Testuj:**
1. File > Open - powinien pokazać native dialog z filtrem .cypcb
2. File > Save - powinien zapisać plik
3. View > Toggle Theme - zmiana dark/light mode
4. View > Toggle Fullscreen (F11)
5. Minimize/maximize przez native window controls

## 📊 Wymagania systemowe

- **Windows:** 10/11
- **macOS:** 10.15+ (Catalina lub nowszy)
- **Linux:** Ubuntu 20.04+, Debian 11+, lub equivalent

## 🐛 Zgłaszanie problemów

Jeśli coś nie działa:
1. Skopiuj output z terminala
2. Sprawdź który krok setup nie przeszedł
3. Załącz informacje o systemie (OS, wersje Node/Rust)

## 💡 Pro Tips

- **Pierwsze buildy są wolne** (10-20 min) - Rust kompiluje wszystko. Kolejne będą szybsze.
- **Dev mode** (hot reload) - zmiana kodu → auto-refresh
- **Production build** - zoptymalizowany, mały (5-10MB), szybki startup
