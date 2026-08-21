fn main() {
    // `SMAppService` (platform/login_item.rs) lives in ServiceManagement, which
    // nothing else in the tree links, so the class would not be found at
    // runtime without this.
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=ServiceManagement");

    tauri_build::build()
}
