# Packaging and Release Guide

This guide describes how to build releases of BoardViewCraft across all supported desktop platforms (Linux, Windows, macOS).

---

## 1. Setup on Kubuntu 26.04

To compile and package releases locally on Kubuntu 26.04:

### A. Core Linux Build Tools
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libx11-dev libxcursor-dev \
    libxrandr-dev libxi-dev libxinerama-dev libwayland-dev libxkbcommon-dev zip
```

### B. Windows Cross-Compilation (Choose one or both)

#### Option 1: `cargo-xwin` (Recommended — produces MSVC binaries matching Windows native builds)
```bash
sudo apt install -y clang lld
cargo install cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

#### Option 2: `mingw-w64` (GNU toolchain)
```bash
sudo apt install -y mingw-w64 nsis
rustup target add x86_64-pc-windows-gnu
```

---

## 2. Local Packaging Commands

### Linux Releases (.tar.gz and .deb)
From the repository root:
```bash
packaging/linux/package.sh
```
Output located in `dist/release/`:
- `boardviewcraft-<version>-linux-x86_64.tar.gz`
- `boardviewcraft_<version>_amd64.deb`

### Windows Releases from Linux
Run the cross-packaging script:
```bash
packaging/windows/package-cross.sh
```
Output located in `dist/release/`:
- `boardviewcraft-<version>-windows-x64-portable.zip`

### Windows Releases on Windows
On a Windows system or Windows VM with WiX v5 installed:
```powershell
pwsh packaging/windows/package.ps1 -Arch x64
```
Output located in `dist/release/`:
- `boardviewcraft-<version>-windows-x64.msi` (Installer)
- `boardviewcraft-<version>-windows-x64-portable.zip`

### macOS Releases on macOS
On a macOS system:
```bash
packaging/macos/package.sh --arch universal
```
Output located in `dist/release/`:
- `boardviewcraft-<version>-macos-universal.dmg` (Drag-to-Applications)
- `boardviewcraft-<version>-macos-universal.zip`

---

## 3. Automated GitHub Releases

Automated releases for all platforms are configured in `.github/workflows/release.yml`.

Whenever you push a version tag:
```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will simultaneously spin up:
- **Ubuntu Runner**: builds Linux `.deb` and `.tar.gz`.
- **Windows Runner**: builds Windows `.msi` and portable `.zip`.
- **macOS Runner**: builds Universal macOS `.dmg` and `.zip`.

All artifacts are automatically gathered and published to the GitHub Release page.
