use std::{env, fs, path::PathBuf};

fn main() {
    embed_plugin_bundle();
    app_version();
    tauri_build::build();
}

/// The app's version is the root package.json's alone (release-please bumps it there and nowhere
/// else); `WINER_VERSION` hands it to the code, since the crates keep a static `0.0.0`.
fn app_version() {
    let manifest = PathBuf::from("../../package.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest).expect("the root package.json is readable");
    let package: serde_json::Value = serde_json::from_str(&text).expect("package.json is JSON");
    let version = package["version"]
        .as_str()
        .expect("package.json has a version");
    println!("cargo:rustc-env=WINER_VERSION={version}");
}

/// The in-client plugin ships inside the binary, so installing it needs no download. A release
/// build refuses to go without it; a debug build gets a stub, so `cargo test` works before the
/// plugin has been built.
fn embed_plugin_bundle() {
    let bundle = PathBuf::from("../../plugin/dist/index.js");
    println!("cargo:rerun-if-changed={}", bundle.display());
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("plugin.js");
    let code = match fs::read_to_string(&bundle) {
        Ok(code) => code,
        Err(_) if env::var("PROFILE").as_deref() == Ok("release") => {
            panic!("plugin/dist/index.js is missing; run `pnpm --filter @winer/plugin build` first")
        }
        Err(_) => "/*! winer-plugin 0.0.0-dev */\nexport function init() {}\n".to_owned(),
    };
    fs::write(out, code).expect("OUT_DIR is writable");
}
