//! Embeds native application resources and tracks the Windows icon as a build input.

/// Rebuilds the executable resource when the authored ICO changes.
fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build();
}
