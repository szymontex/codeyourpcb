@echo off
echo ============================================
echo CodeYourPCB Desktop - Windows Setup
echo ============================================
echo.

REM Check Node.js
echo [1/5] Checking Node.js...
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Node.js not found!
    echo.
    echo Please install Node.js from: https://nodejs.org/
    echo Recommended: v20 LTS or newer
    echo.
    pause
    exit /b 1
)
echo [OK] Node.js found:
node --version

REM Check Rust
echo.
echo [2/5] Checking Rust...
cargo --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [WARN] Rust not found. Installing via rustup...
    echo.
    powershell -Command "Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe"
    rustup-init.exe -y
    del rustup-init.exe
    call "%USERPROFILE%\.cargo\env.bat"
    cargo --version >nul 2>&1
    if %errorlevel% neq 0 (
        echo [ERROR] Rust installation failed!
        pause
        exit /b 1
    )
)
echo [OK] Rust found:
cargo --version

REM Install npm dependencies
echo.
echo [3/5] Installing frontend dependencies...
cd viewer
call npm install
if %errorlevel% neq 0 (
    echo [ERROR] npm install failed!
    cd ..
    pause
    exit /b 1
)
cd ..
echo [OK] Frontend dependencies installed

REM Verify Tauri CLI
echo.
echo [4/5] Verifying Tauri CLI...
cd viewer
call npm run tauri -- --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [WARN] Tauri CLI not working, reinstalling...
    call npm install --save-dev @tauri-apps/cli@2
)
cd ..
echo [OK] Tauri CLI ready

REM Build verification (skip actual compile due to GTK requirement)
echo.
echo [5/5] Verifying project structure...
if not exist "src-tauri\Cargo.toml" (
    echo [ERROR] Tauri project not found at src-tauri/
    pause
    exit /b 1
)
echo [OK] Tauri project structure verified

echo.
echo ============================================
echo Setup complete!
echo ============================================
echo.
echo To run in development mode:
echo   - Double-click: dev-windows.bat
echo.
echo To build installer:
echo   - Double-click: build-windows.bat
echo.
pause
