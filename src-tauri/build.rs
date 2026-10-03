fn main() {
    // Version shown in the tray menu. The release workflow sets APP_VERSION from the Git tag.
    println!("cargo:rerun-if-env-changed=APP_VERSION");
    tauri_build::build()
}
