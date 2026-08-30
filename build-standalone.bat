@echo off
REM Build native desktop app (direct mode, in-process stocker-core).
REM Use this when PowerShell script execution policy blocks build-standalone.ps1.
cd /d "%~dp0"
cargo build -p stocker-web --release --no-default-features --features desktop
if errorlevel 1 exit /b 1
echo.
echo Executable: %~dp0target\release\stocker-web.exe
