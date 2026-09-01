//! What `tauri-build` needs before the crate compiles: the Windows resource
//! file carrying the icon and the version, and the generated capability set.
fn main() {
    tauri_build::build();
}
