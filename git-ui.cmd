@echo off
rem Builds (if needed) and launches the Action-Based-Generation git UI.
rem
rem The GNU Rust toolchain needs MinGW's binutils on PATH for `dlltool`, which
rem the Windows crates use to build import libraries. This script puts them
rem there so you do not have to touch your system PATH.

setlocal
cd /d "%~dp0"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

set "WINLIBS=%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
if exist "%WINLIBS%\as.exe" set "PATH=%WINLIBS%;%PATH%"

echo Building abg-git-ui (the first build takes a minute)...
cargo build --release -p abg-git-ui
if errorlevel 1 (
    echo.
    echo Build failed - see the errors above.
    pause
    exit /b 1
)

start "" "target\release\abg-git-ui.exe"
