@echo off
rem Builds (if needed) and launches the generator viewer.
rem
rem The GNU Rust toolchain needs MinGW's binutils on PATH for `dlltool`. This
rem script puts them there so you do not have to touch your system PATH.

setlocal
cd /d "%~dp0"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

set "WINLIBS=%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
if exist "%WINLIBS%\as.exe" set "PATH=%WINLIBS%;%PATH%"

echo Building abg-viewer...
cargo build --release -p abg-viewer
if errorlevel 1 (
    echo.
    echo Build failed - see the errors above.
    pause
    exit /b 1
)

start "" "target\release\abg-viewer.exe"
