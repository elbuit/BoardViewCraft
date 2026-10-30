<#
.SYNOPSIS
  Build and package BoardViewCraft for Windows.

.DESCRIPTION
  Produces in $env:DIST (default: dist/release):
    boardviewcraft-<version>-windows-<arch>.msi            per-machine installer (WiX v5)
    boardviewcraft-<version>-windows-<arch>-portable.zip   portable executable archive

  Prerequisites on Windows:
    - Rust MSVC toolchain + target (e.g. rustup target add x86_64-pc-windows-msvc)
    - WiX v5:
        dotnet tool install --global wix --version 5.0.2
        wix extension add -g WixToolset.UI.wixext/5.0.2

.EXAMPLE
  pwsh packaging/windows/package.ps1 -Arch x64
#>
param(
  [ValidateSet('x64', 'x86', 'arm64')] [string] $Arch = 'x64',
  [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

$Version = $env:BVC_VERSION
if (-not $Version) {
  $inPkg = $false
  foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
    if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
  }
}
if (-not $Version) { $Version = "0.1.0" }
$MsiVersion = ($Version -split '-')[0]

$Target = switch ($Arch) {
  'x64'   { 'x86_64-pc-windows-msvc' }
  'x86'   { 'i686-pc-windows-msvc' }
  'arm64' { 'aarch64-pc-windows-msvc' }
}

$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

Write-Output "BoardViewCraft $Version for Windows $Arch ($Target)"

if (-not $SkipBuild) {
  Invoke-Native "Compiling BoardViewCraft release binary" {
    cargo build --manifest-path (Join-Path $Root 'Cargo.toml') --package boardviewcraft --release --target $Target --ignore-rust-version
  }
}

$ExeSrc = Join-Path $TargetDir "$Target\release\boardviewcraft.exe"
if (-not (Test-Path $ExeSrc)) {
  $ExeSrc = Join-Path $TargetDir "release\boardviewcraft.exe"
}

if (-not (Test-Path $ExeSrc)) {
  throw "Binary not found at $ExeSrc"
}

# 1. Create portable ZIP
$ZipName = "boardviewcraft-$Version-windows-$Arch-portable.zip"
$ZipOut = Join-Path $Dist $ZipName
$StageDir = Join-Path $TargetDir "windows-stage-$Arch"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
Copy-Item $ExeSrc -Destination (Join-Path $StageDir 'boardviewcraft.exe')

if (Test-Path (Join-Path $Root 'README.md')) {
  Copy-Item (Join-Path $Root 'README.md') -Destination (Join-Path $StageDir 'README.md')
}

$LicenseRtf = Join-Path $PSScriptRoot 'license.rtf'
if (Test-Path $LicenseRtf) {
  Copy-Item $LicenseRtf -Destination (Join-Path $StageDir 'license.rtf')
}
if (Test-Path (Join-Path $Root 'LICENSE-MIT')) {
  Copy-Item (Join-Path $Root 'LICENSE-MIT') -Destination (Join-Path $StageDir 'LICENSE-MIT')
}
if (Test-Path (Join-Path $Root 'LICENSE-APACHE')) {
  Copy-Item (Join-Path $Root 'LICENSE-APACHE') -Destination (Join-Path $StageDir 'LICENSE-APACHE')
}

if (Test-Path $ZipOut) { Remove-Item -Force $ZipOut }
Compress-Archive -Path "$StageDir\*" -DestinationPath $ZipOut -Force
Write-Output "Created portable ZIP: $ZipOut"

# 2. Build WiX MSI if wix is installed
if (Get-Command 'wix' -ErrorAction SilentlyContinue) {
  $MsiOut = Join-Path $Dist "boardviewcraft-$Version-windows-$Arch.msi"
  $WxsFile = Join-Path $PSScriptRoot 'boardviewcraft.wxs'
  Invoke-Native "Building WiX MSI installer" {
    wix build $WxsFile -ext WixToolset.UI.wixext/5.0.2 -d "Version=$MsiVersion" -d "SourceDir=$StageDir" -o $MsiOut
  }
  Write-Output "Created MSI installer: $MsiOut"
} else {
  Write-Warning "WiX tool not detected. Skipping MSI generation (install with: dotnet tool install --global wix)"
}
