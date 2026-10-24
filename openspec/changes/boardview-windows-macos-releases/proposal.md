# Proposal: Cross-Platform Releases for Windows and macOS (Cadcraft Style)

## Intent
Enable building, packaging, and publishing BoardViewCraft release artifacts for **Windows** and **macOS** matching the structure established in `cadcraft`. This enables distributing native installers, bundles, and portable binaries across all desktop platforms:
- **Linux**: `.tar.gz`, `.deb`, and AppImage (already existing under `packaging/linux`).
- **Windows**: `.msi` (WiX v5) and portable `.zip` (`x86_64-pc-windows-msvc` / `x86_64-pc-windows-gnu`).
- **macOS**: `.dmg` (Drag-to-Applications bundle) and `.zip` for Apple Silicon (`aarch64`) and Intel (`x86_64`) / Universal.
- **CI/CD Automation**: GitHub Actions workflow mirroring Cadcraft's `release.yml` with cross-platform matrix runners.
- **Local Building Guide**: Step-by-step instructions for packages needed on **Kubuntu 26.04** to build and package locally (using cross-compilation with MinGW / `cargo-xwin` and OSXCROSS or native CI).

## Scope

### In Scope
1. **Packaging Directory Structure** (matching `cadcraft/packaging`):
   - `packaging/windows/`:
     - `package.ps1` (PowerShell script for building, WiX packaging, and portable ZIP creation).
     - `boardviewcraft.wxs` (WiX installer definition for Windows MSI).
     - `package-mingw.sh` / cross-build script for building Windows binaries from Linux.
   - `packaging/macos/`:
     - `package.sh` (macOS `.app` bundle, `Info.plist.in`, and `.dmg` packaging script).
     - `Info.plist.in` and `entitlements.plist`.
     - `dmg/` styling configuration.
2. **GitHub Actions Workflow**:
   - `.github/workflows/release.yml` multi-platform build matrix (`windows-latest`, `macos-latest`, `ubuntu-latest`).
3. **Kubuntu 26.04 Tooling & Dependency Guide**:
   - Concrete list of packages and tools required on Kubuntu 26.04 to cross-compile and package locally.

### Out of Scope
- Procuring Apple Developer certificates or Microsoft Authenticode hardware tokens (scripts will support ad-hoc/unsigned local builds).
- Android or iOS mobile builds.

## Capabilities

### New Capabilities
- `boardview-release-packaging`: Comprehensive multi-platform release scripts and CI workflow for Linux, Windows, and macOS matching Cadcraft conventions.

## Approach
1. **Structure Packaging**:
   - Replicate the proven Cadcraft packaging layout in `BoardViewCraft/packaging/windows` and `BoardViewCraft/packaging/macos`.
   - Adapt `Info.plist.in` and WiX `.wxs` with BoardViewCraft metadata, icons, bundle ID (`com.boardviewcraft.app`), and file associations (`.cad`, `.tvw`).
2. **Workflow Automation**:
   - Add `.github/workflows/release.yml` executing Linux (`package.sh`), Windows (`package.ps1`), and macOS (`package.sh`) on native runner matrix.
3. **Kubuntu 26.04 Tooling Specification**:
   - Provide exact APT and Cargo commands needed on Kubuntu 26.04:
     - For Windows cross-compilation: `mingw-w64`, `cargo-xwin`, `lld`, `nsis`.
     - For macOS / dmg: `genisoimage`, `dmgbuild`, `osxcross` or GitHub Actions workflow dispatch.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `BoardViewCraft/packaging/windows/` | New | PowerShell packaging script, WiX `.wxs`, and local cross-compile helper |
| `BoardViewCraft/packaging/macos/` | New | Shell script for `.app` bundle and `.dmg` packaging, `Info.plist.in` |
| `BoardViewCraft/.github/workflows/release.yml` | New | GitHub Actions multi-platform release pipeline |
| `BoardViewCraft/packaging/README.md` | New | Complete guide for building and packaging on Kubuntu 26.04 |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| macOS `.dmg` generation directly on Linux | High | Native macOS tools (`hdiutil`, `codesign`) are macOS-only; recommend GitHub Actions macOS runners for official releases, while providing `cargo-xwin` / MinGW for local Windows builds on Linux. |
| Missing WiX on Linux | Medium | WiX v5 requires .NET / Windows; for local Linux Windows builds, provide portable `.zip` generation via `cargo-xwin` / `mingw-w64`, reserving `.msi` for Windows runners. |

## Rollback Plan
Remove the added packaging subdirectories and workflow files. Existing application code remains unaffected.

## Success Criteria
- [ ] `packaging/windows` and `packaging/macos` scripts created following Cadcraft standards.
- [ ] GitHub Actions `release.yml` configured to build Windows (`.msi` + portable `.zip`), macOS (`.dmg`), and Linux (`.deb` + `.tar.gz`).
- [ ] Detailed Kubuntu 26.04 setup guide documented with exact packages for local compilation.
