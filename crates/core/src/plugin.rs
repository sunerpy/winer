//! Installing the in-client plugin into Pengu Loader and handing it the bridge's address.
//!
//! Pengu Loader v1.1 is a portable folder; it is activated for one client install by a
//! `version.dll` symlink in the client directory that points at the loader's `core.dll`. Plugins
//! live in `<loader>\plugins\<name>\index.js`. See `docs/platform-notes.md`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use ts_rs::TS;

use crate::settings::write_atomic;

pub const PLUGIN_NAME: &str = "winer";
/// The first line of a bundle names its version: `/*! winer-plugin 0.2.0 */`.
const BANNER: &str = "/*! winer-plugin ";

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatus {
    pub loader_dir: Option<String>,
    /// The loader is linked into the connected client, so plugins load with it.
    pub active: bool,
    pub installed_version: Option<String>,
    pub bundled_version: String,
    /// The installed plugin is byte-for-byte this build's bundle. Two builds can share a version.
    pub current: bool,
    /// Plugin contexts connected to the bridge right now.
    pub connected: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loader {
    pub dir: PathBuf,
    pub active: bool,
}

/// The loader for `client_dir`: the configured folder if it holds a loader, else wherever the
/// client's `version.dll` link points.
pub fn find_loader(client_dir: Option<&Path>, configured: Option<&Path>) -> Option<Loader> {
    let linked = client_dir
        .and_then(|dir| fs::read_link(dir.join("version.dll")).ok())
        .and_then(|target| target.parent().map(Path::to_path_buf));
    if let Some(dir) =
        configured.filter(|dir| dir.join("core.dll").is_file() || dir.join("plugins").is_dir())
    {
        let active = linked
            .as_deref()
            .is_some_and(|linked| same_dir(linked, dir));
        return Some(Loader {
            dir: dir.to_path_buf(),
            active,
        });
    }
    linked
        .filter(|dir| dir.is_dir())
        .map(|dir| Loader { dir, active: true })
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

pub fn plugin_dir(loader: &Path) -> PathBuf {
    loader.join("plugins").join(PLUGIN_NAME)
}

pub fn bundle_version(bundle: &str) -> Option<String> {
    let first = bundle.lines().next()?.trim();
    Some(
        first
            .strip_prefix(BANNER)?
            .strip_suffix("*/")?
            .trim()
            .to_owned(),
    )
}

pub fn installed_version(loader: &Path) -> Option<String> {
    let entry = fs::read_to_string(plugin_dir(loader).join("index.js")).ok()?;
    bundle_version(&entry)
}

/// Whether winer's plugin is installed and is exactly `bundle`; `None` when it is not installed.
pub fn is_current(loader: &Path, bundle: &str) -> Option<bool> {
    let entry = fs::read(plugin_dir(loader).join("index.js")).ok()?;
    Some(entry == bundle.as_bytes())
}

pub fn install(loader: &Path, bundle: &str) -> io::Result<()> {
    write_atomic(&plugin_dir(loader).join("index.js"), bundle.as_bytes())
}

pub fn uninstall(loader: &Path) -> io::Result<()> {
    match fs::remove_dir_all(plugin_dir(loader)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Writes the bridge address beside an installed plugin. Does nothing when it is not installed:
/// creating the folder would make the loader see a plugin without an entry.
pub fn write_bootstrap(loader: &Path, port: u16, token: &str) -> io::Result<bool> {
    let dir = plugin_dir(loader);
    if !dir.join("index.js").is_file() {
        return Ok(false);
    }
    let record = serde_json::json!({ "port": port, "token": token });
    write_atomic(&dir.join("bootstrap.json"), record.to_string().as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLE: &str = "/*! winer-plugin 1.2.3 */\nexport function init() {}\n";

    #[test]
    fn the_bundle_banner_carries_the_version() {
        assert_eq!(bundle_version(BUNDLE).as_deref(), Some("1.2.3"));
        assert_eq!(bundle_version("export {}"), None);
    }

    #[test]
    fn install_bootstrap_and_uninstall_round_trip() {
        let loader = tempfile::tempdir().unwrap();
        assert!(
            !write_bootstrap(loader.path(), 1, "t").unwrap(),
            "no bootstrap without a plugin"
        );
        assert!(!plugin_dir(loader.path()).exists());

        assert_eq!(is_current(loader.path(), BUNDLE), None);
        install(loader.path(), BUNDLE).unwrap();
        assert_eq!(installed_version(loader.path()).as_deref(), Some("1.2.3"));
        assert_eq!(is_current(loader.path(), BUNDLE), Some(true));
        let rebuilt = BUNDLE.replace("init() {}", "init() { start(); }");
        assert_eq!(
            is_current(loader.path(), &rebuilt),
            Some(false),
            "same version, new build"
        );
        assert!(write_bootstrap(loader.path(), 4242, "secret").unwrap());
        let bootstrap: serde_json::Value = serde_json::from_slice(
            &fs::read(plugin_dir(loader.path()).join("bootstrap.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            bootstrap,
            serde_json::json!({"port": 4242, "token": "secret"})
        );

        uninstall(loader.path()).unwrap();
        uninstall(loader.path()).unwrap();
        assert_eq!(installed_version(loader.path()), None);
    }

    #[test]
    fn a_configured_folder_is_used_only_when_it_holds_a_loader() {
        let loader = tempfile::tempdir().unwrap();
        assert_eq!(find_loader(None, Some(loader.path())), None);
        fs::create_dir(loader.path().join("plugins")).unwrap();
        assert_eq!(
            find_loader(None, Some(loader.path())),
            Some(Loader {
                dir: loader.path().to_path_buf(),
                active: false
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_client_link_names_an_active_loader() {
        let root = tempfile::tempdir().unwrap();
        let (loader, client) = (root.path().join("pengu"), root.path().join("LeagueClient"));
        fs::create_dir_all(loader.join("plugins")).unwrap();
        fs::create_dir_all(&client).unwrap();
        fs::write(loader.join("core.dll"), b"").unwrap();
        std::os::unix::fs::symlink(loader.join("core.dll"), client.join("version.dll")).unwrap();
        assert_eq!(
            find_loader(Some(&client), None),
            Some(Loader {
                dir: loader.clone(),
                active: true
            })
        );
        assert_eq!(
            find_loader(Some(&client), Some(&loader)).map(|found| found.active),
            Some(true)
        );
    }
}
