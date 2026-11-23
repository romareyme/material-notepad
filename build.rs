//! Embed the Windows icon and version strings into the executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("FileDescription", "Material Notepad");
        resource.set("ProductName", "Material Notepad");
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        resource.set("FileVersion", env!("CARGO_PKG_VERSION"));
        resource.set("OriginalFilename", "material-notepad.exe");
        resource.set("LegalCopyright", "GPL-3.0-or-later");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Windows resources not compiled: {error}");
        }
    }
}
