param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('Format', 'Clippy', 'Test')]
  [string]$Action
)

$ErrorActionPreference = 'Stop'
$defaultBin = Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin'
$localBin = Join-Path $env:USERPROFILE '.rustup\toolchains\stable-local\bin'
$manifest = Join-Path $PSScriptRoot '..\src-tauri\Cargo.toml'
$buildRoot = Join-Path $PSScriptRoot '..\.local-build'
New-Item -ItemType Directory -Force -Path $buildRoot | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $buildRoot 'cargo-target'
$env:TEMP = Join-Path $buildRoot 'temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null

if ($Action -eq 'Format') {
  $rustfmt = Join-Path $defaultBin 'rustfmt.exe'
  $sources = Get-ChildItem (Join-Path $PSScriptRoot '..\src-tauri\src') -Filter '*.rs' -Recurse | ForEach-Object FullName
  & $rustfmt --edition 2021 --check @sources
  exit $LASTEXITCODE
}

if ($Action -eq 'Clippy') {
  $env:CARGO = Join-Path $localBin 'cargo.exe'
  $env:RUSTC = Join-Path $defaultBin 'rustc.exe'
  & (Join-Path $defaultBin 'cargo-clippy.exe') clippy --manifest-path $manifest --offline -- -D warnings
  exit $LASTEXITCODE
}

& (Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe') run stable-local cargo test --manifest-path $manifest --offline
exit $LASTEXITCODE
