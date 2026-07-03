@echo off
setlocal enabledelayedexpansion

if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul
if errorlevel 1 (
  echo ERROR: cargo not found. Install Rust from https://rustup.rs and reopen the terminal.
  exit /b 1
)

set "HERE=%~dp0.."
set "SRC=%HERE%\src"

echo [1/2] Building plugin binary (release)...
pushd "%SRC%"
cargo run -p xtask -- dist
if errorlevel 1 goto :err

echo [2/2] Packaging release...
cargo run -p xtask -- package
if errorlevel 1 goto :err
popd

if not exist "%HERE%\bin\ffmpeg.exe" (
  echo.
  echo WARNING: %HERE%\bin\ffmpeg.exe is missing ^(downloads will not work without it^).
  echo It is vendored in the repo, so a fresh clone should already have it.
  echo To rebuild the minimal ffmpeg.exe, run on a Mac: tools/build_ffmpeg_win.sh
  echo ^(needs: brew install mingw-w64^), then commit bin\ffmpeg.exe and re-run this script.
)

echo.
echo Done. Release zip is in %HERE%\..\release\
goto :eof

:err
popd
echo Build failed.
exit /b 1
