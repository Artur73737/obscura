@echo off
REM Build Obscura with the `stealth` feature on Windows (TLS impersonation via
REM BoringSSL/wreq). Needed for search engines that fingerprint TLS (Bing works
REM with this; Google additionally IP-blocks and needs a residential --proxy).
REM
REM Requires, on PATH or installed:
REM   - Visual Studio Build Tools (MSVC x64)  -> vcvars64.bat below
REM   - NASM        (scoop install nasm)      -> BoringSSL assembly
REM   - LLVM        (scoop install llvm)      -> libclang for bindgen
REM   - Ninja       (pip/scoop)               -> cmake generator
REM
REM Adjust the paths below to your machine if they differ.

call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set "PATH=%PATH%;%USERPROFILE%\scoop\shims"
set CMAKE_GENERATOR=Ninja
set "LIBCLANG_PATH=%USERPROFILE%\scoop\apps\llvm\current\bin"

cd /d "%~dp0.."
cargo build --release -p obscura-cli --features stealth
