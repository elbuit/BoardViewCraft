//! Windows only: embed the app icon and version info (VERSIONINFO) into `boardviewcraft.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from Linux or macOS still links, unless `BOARDVIEWCRAFT_REQUIRE_WINRES=1` turns
//! it into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/boardviewcraft.ico");
    println!("cargo:rerun-if-env-changed=BOARDVIEWCRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/boardviewcraft.ico")
        .set("ProductName", "BoardViewCraft")
        .set("FileDescription", "BoardViewCraft CAD board viewer")
        .set("OriginalFilename", "boardviewcraft.exe")
        .set("InternalName", "boardviewcraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("BOARDVIEWCRAFT_REQUIRE_WINRES").is_some() {
            println!("cargo::error=embedding Windows resources failed: {e}");
            return;
        }
        println!("cargo:warning=boardviewcraft.exe built without icon/version resources: {e}");
    }
}
