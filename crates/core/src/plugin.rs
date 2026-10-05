//! Pengu Loader and the in-client plugin: the loader winer ships, linking it into the client,
//! installing the plugin and handing it the bridge's address.
//!
//! Pengu Loader v1.1 is a portable folder; it is activated for one client install by a
//! `version.dll` symlink in the client directory that points at the loader's `core.dll`, which
//! the client loads when its interface starts. Plugins live in `<loader>\plugins\<name>\index.js`.
//! See `docs/platform-notes.md`.

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
    /// The loader is the one winer ships, kept in winer's own data folder.
    pub managed: bool,
    /// The version of the loader winer ships.
    pub bundled_loader: String,
    /// The client's `version.dll` is a file of something else, so winer links no loader there.
    pub occupied: bool,
    /// Why the last automatic setup did not finish, as the system put it.
    pub setup_error: Option<String>,
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

/// What linking a loader into a client found there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linked {
    /// `version.dll` now points at the loader.
    Created,
    /// It already pointed at a loader that exists, this one or another the user installed: that
    /// one stays.
    Existing,
    /// A file of that name that is not a link: something else uses it, and it is left alone.
    Occupied,
}

/// Puts `core`, Pengu Loader's `core.dll`, in `dir` unless the file there is exactly it. Returns
/// whether it wrote. The client keeps a loaded loader open, so replacing one fails while the
/// client runs; the next connection without a client holding it tries again.
pub fn place_loader(dir: &Path, core: &[u8]) -> io::Result<bool> {
    let path = dir.join("core.dll");
    if fs::read(&path).is_ok_and(|existing| existing == core) {
        return Ok(false);
    }
    write_atomic(&path, core)?;
    Ok(true)
}

/// Whether the client's `version.dll` is a file that is not a link.
pub fn occupied(client_dir: &Path) -> bool {
    fs::symlink_metadata(client_dir.join("version.dll"))
        .is_ok_and(|meta| !meta.file_type().is_symlink())
}

/// Activates the loader in `loader` for the client in `client_dir` the way Pengu Loader itself
/// does: `version.dll` there becomes a symbolic link to `<loader>\core.dll`. A link whose target
/// is gone is replaced. On Windows a symbolic link needs an elevated process (or Developer Mode).
pub fn link(client_dir: &Path, loader: &Path) -> io::Result<Linked> {
    let path = client_dir.join("version.dll");
    match fs::symlink_metadata(&path) {
        Ok(meta) if !meta.file_type().is_symlink() => return Ok(Linked::Occupied),
        Ok(_) if fs::metadata(&path).is_ok() => return Ok(Linked::Existing),
        Ok(_) => fs::remove_file(&path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    symlink(&loader.join("core.dll"), &path)?;
    Ok(Linked::Created)
}

/// Removes the client's `version.dll` when it is a link to `loader`'s core; anything else stays.
/// Returns whether it removed one.
pub fn unlink(client_dir: &Path, loader: &Path) -> io::Result<bool> {
    let path = client_dir.join("version.dll");
    let ours = fs::read_link(&path)
        .ok()
        .and_then(|target| target.parent().map(Path::to_path_buf))
        .is_some_and(|dir| same_dir(&dir, loader));
    if ours {
        fs::remove_file(&path)?;
    }
    Ok(ours)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
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
    fn the_loader_is_placed_once_and_linked_only_where_nothing_else_is() {
        let root = tempfile::tempdir().unwrap();
        let (loader, client) = (root.path().join("pengu"), root.path().join("LeagueClient"));
        fs::create_dir_all(&client).unwrap();
        assert!(
            place_loader(&loader, b"core").unwrap(),
            "written the first time"
        );
        assert!(
            !place_loader(&loader, b"core").unwrap(),
            "the same bytes are left alone"
        );
        assert!(place_loader(&loader, b"newer").unwrap());

        assert_eq!(link(&client, &loader).unwrap(), Linked::Created);
        assert_eq!(
            fs::read_link(client.join("version.dll")).unwrap(),
            loader.join("core.dll")
        );
        assert_eq!(link(&client, &loader).unwrap(), Linked::Existing);
        assert_eq!(
            find_loader(Some(&client), None).map(|found| found.active),
            Some(true)
        );

        let other = root.path().join("other");
        assert!(
            !unlink(&client, &other).unwrap(),
            "another loader's link is not ours to remove"
        );
        assert!(unlink(&client, &loader).unwrap());
        assert!(fs::symlink_metadata(client.join("version.dll")).is_err());

        std::os::unix::fs::symlink(other.join("core.dll"), client.join("version.dll")).unwrap();
        assert_eq!(
            link(&client, &loader).unwrap(),
            Linked::Created,
            "a link to a loader that is gone is replaced"
        );
        fs::remove_file(client.join("version.dll")).unwrap();
        fs::write(client.join("version.dll"), b"someone else").unwrap();
        assert!(occupied(&client));
        assert_eq!(link(&client, &loader).unwrap(), Linked::Occupied);
        assert_eq!(
            fs::read(client.join("version.dll")).unwrap(),
            b"someone else"
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
