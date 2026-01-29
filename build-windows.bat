@echo off
echo ============================================
echo Building CodeYourPCB Desktop Installer
echo ============================================
echo.
echo This will create a production installer for Windows.
echo Output will be in: src-tauri\target\release\bundle\
echo.
echo NOTE: This may take 10-20 minutes on first build.
echo.

cd viewer
call npm run build:desktop

echo.
echo ============================================
echo Build complete!
echo ============================================
echo.
echo Installer location:
dir ..\src-tauri\target\release\bundle\msi\*.msi /b
echo.
pause
