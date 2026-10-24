# Spec: Multi-Platform Release Packaging for Windows and macOS

## Requirements

### Requirement: Windows Packaging Scripts
The repository MUST provide Windows packaging scripts under `packaging/windows/` following the Cadcraft structure:
- `packaging/windows/package.ps1`: Builds release binaries for target architectures (`x64`, `x86`, `arm64`), produces a portable ZIP archive containing `boardviewcraft.exe`, and builds a WiX v5 MSI installer when `wix` is available.
- `packaging/windows/boardviewcraft.wxs`: WiX v5 XML source defining product ID, file attachments, start menu shortcuts, and `.cad` / `.tvw` file associations.
- `packaging/windows/package-cross.sh`: Bash script runnable on Linux (e.g. Kubuntu) using `cargo-xwin` or MinGW to produce `boardviewcraft-windows-x64-portable.zip`.

### Requirement: macOS Packaging Scripts
The repository MUST provide macOS packaging scripts under `packaging/macos/` following the Cadcraft structure:
- `packaging/macos/Info.plist.in`: Template for macOS bundle metadata, CFBundleIdentifier (`com.boardviewcraft.app`), high DPI support, and document types (`.cad`, `.tvw`).
- `packaging/macos/entitlements.plist`: Hardened runtime entitlements for macOS.
- `packaging/macos/package.sh`: Bash script to build Universal (`aarch64` + `x86_64`) or specific architecture binaries, construct `BoardViewCraft.app`, sign, and package into a drag-to-Applications `.dmg`.

### Requirement: CI/CD GitHub Actions Pipeline
The repository MUST provide `.github/workflows/release.yml`:
- Triggered on tag push (`v*.*.*`) or manual `workflow_dispatch`.
- Builds and packages on native matrix runners:
  - `ubuntu-latest` -> Linux `.deb` and `.tar.gz`
  - `windows-latest` -> Windows `.msi` and portable `.zip`
  - `macos-latest` -> macOS `.dmg` and portable `.zip`
- Uploads built artifacts to GitHub Releases automatically.
