use std::path::Path;

const HOMEBREW_LIBRARY_DIRECTORIES: [&str; 2] = ["/opt/homebrew/lib", "/usr/local/lib"];

fn main() {
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let library_directories = match pkg_config::Config::new().cargo_metadata(false).probe("mpv") {
        Ok(library) => library.link_paths,
        Err(_) => HOMEBREW_LIBRARY_DIRECTORIES
            .iter()
            .map(Path::new)
            .filter(|directory| directory.join("libmpv.dylib").exists())
            .map(Path::to_path_buf)
            .collect(),
    };
    for directory in library_directories {
        println!("cargo:rustc-link-search=native={}", directory.display());
    }
}
