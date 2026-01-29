@echo off
echo ============================================
echo Starting CodeYourPCB Desktop (Development)
echo ============================================
echo.
echo NOTE: This will start the Vite dev server and Tauri window.
echo Press Ctrl+C to stop.
echo.

cd viewer
call npm run dev:desktop

pause
