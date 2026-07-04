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

echo.
echo Done. Release zip is in %HERE%\..\release\
goto :eof

:err
popd
echo Build failed.
exit /b 1
