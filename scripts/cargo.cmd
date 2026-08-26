@echo off
setlocal
chcp 65001 >nul

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo Visual Studio Build Tools were not found. 1>&2
  exit /b 1
)

for /f "usebackq tokens=*" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS_PATH=%%i"
if not defined VS_PATH (
  echo Visual C++ x64 build tools were not found. 1>&2
  exit /b 1
)

call "%VS_PATH%\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 exit /b %errorlevel%

where cargo >nul 2>nul
if errorlevel 1 (
  set "CARGO_EXE=%USERPROFILE%\.cargo\bin\cargo.exe"
) else (
  set "CARGO_EXE=cargo"
)

if not exist "%USERPROFILE%\.cargo\bin\cargo.exe" if "%CARGO_EXE%"=="%USERPROFILE%\.cargo\bin\cargo.exe" (
  echo Rust was not found. Install it from https://rustup.rs. 1>&2
  exit /b 1
)

"%CARGO_EXE%" %*
