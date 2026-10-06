//! The shell's half of the in-client plugin: which loader, the loader winer ships, the plugin
//! bundle, and the bootstrap file that tells the plugin where this session's bridge listens.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use tracing::{info, warn};
use winer_core::{
    CoreError, Service,
    bridge::Bridge,
    plugin::{self, Linked, Loader, PluginStatus},
};

use crate::{PENGU_CORE, PENGU_VERSION, PLUGIN_BUNDLE, elevation};

/// Where the loader winer ships lives, and what went wrong the last time it was set up.
pub(crate) struct Host {
    /// winer's own data folder for it: outside the install directory, so updating winer never
    /// has to replace a DLL the client holds open.
    pub dir: PathBuf,
    failure: Mutex<Option<Failure>>,
}

/// Why the last setup did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
    /// As the system put it.
    message: String,
    /// Only an administrator can create the link, and winer runs without those rights.
    needs_elevation: bool,
}

impl Failure {
    /// A failure no administrator rights would avoid.
    fn other(error: &std::io::Error) -> Self {
        Self {
            message: error.to_string(),
            needs_elevation: false,
        }
    }
}

/// What a setup came to, for its caller to act on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// A loader was linked just now; the client loads it when its interface starts again.
    pub linked: bool,
    /// The link was refused for want of administrator rights, which winer runs without.
    pub needs_elevation: bool,
}

impl Host {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            failure: Mutex::new(None),
        }
    }

    fn set_failure(&self, failure: Option<Failure>) {
        *self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = failure;
    }

    fn failure(&self) -> Option<Failure> {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn loader(service: &Service) -> Option<Loader> {
    let configured = service.settings().plugin.loader_dir;
    plugin::find_loader(
        service.client_dir().as_deref(),
        configured.as_deref().map(Path::new),
    )
}

pub(crate) fn status(service: &Service, bridge: &Bridge, host: &Host) -> PluginStatus {
    let loader = loader(service);
    let client_dir = service.client_dir();
    let failure = host.failure();
    PluginStatus {
        loader_dir: loader
            .as_ref()
            .map(|loader| loader.dir.display().to_string()),
        active: loader.as_ref().is_some_and(|loader| loader.active),
        managed: loader
            .as_ref()
            .is_some_and(|loader| same_dir(&loader.dir, &host.dir)),
        bundled_loader: PENGU_VERSION.into(),
        occupied: client_dir.as_deref().is_some_and(plugin::occupied),
        setup_error: failure.as_ref().map(|failure| failure.message.clone()),
        needs_elevation: failure.is_some_and(|failure| failure.needs_elevation),
        installed_version: loader
            .as_ref()
            .and_then(|loader| plugin::installed_version(&loader.dir)),
        bundled_version: plugin::bundle_version(PLUGIN_BUNDLE).unwrap_or_default(),
        current: loader
            .as_ref()
            .and_then(|loader| plugin::is_current(&loader.dir, PLUGIN_BUNDLE))
            .unwrap_or(false),
        connected: bridge.connected(),
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Turns automatic setup on and sets everything up now, if a client is connected. Also returns
/// what that came to, as [`refresh`] does.
pub(crate) fn enable(
    service: &Service,
    bridge: &Bridge,
    host: &Host,
) -> Result<(PluginStatus, Outcome), CoreError> {
    let mut settings = service.settings();
    settings.plugin.auto = true;
    service.set_settings(settings)?;
    let outcome = refresh(service, bridge, host);
    Ok((status(service, bridge, host), outcome))
}

/// Turns automatic setup off, removes the plugin, and unlinks the loader winer ships from the
/// client; a loader the user installed stays linked. Both take effect when the client's interface
/// next starts.
pub(crate) fn disable(
    service: &Service,
    bridge: &Bridge,
    host: &Host,
) -> Result<PluginStatus, CoreError> {
    let mut settings = service.settings();
    settings.plugin.auto = false;
    service.set_settings(settings)?;
    if let Some(loader) = loader(service) {
        plugin::uninstall(&loader.dir)?;
        info!(dir = %loader.dir.display(), "plugin removed");
    }
    if let Some(client_dir) = service.client_dir()
        && plugin::unlink(&client_dir, &host.dir)?
    {
        info!(client = %client_dir.display(), "winer's loader unlinked from the client");
    }
    host.set_failure(None);
    Ok(status(service, bridge, host))
}

/// On every client connection. With automatic setup on, the default: the loader is the one
/// already linked into the client or configured, else the one winer ships, placed in its data
/// folder and linked; the plugin is installed or brought to this build. With it off, only an
/// installed plugin is brought to this build. Either way the plugin learns this session's bridge.
/// Returns whether a loader was newly linked, which the client loads only when its interface
/// starts again, and whether linking one needs winer restarted elevated.
pub(crate) fn refresh(service: &Service, bridge: &Bridge, host: &Host) -> Outcome {
    let auto = service.settings().plugin.auto;
    let mut outcome = Outcome::default();
    if auto && let Some(client_dir) = service.client_dir() {
        match activate(service, host, &client_dir) {
            Ok(created) => {
                outcome.linked = created;
                host.set_failure(None);
            }
            Err(failure) => {
                warn!(
                    error = %failure.message,
                    needs_elevation = failure.needs_elevation,
                    "the loader was not set up"
                );
                outcome.needs_elevation = failure.needs_elevation;
                host.set_failure(Some(failure));
            }
        }
    }
    let Some(loader) = loader(service) else {
        return outcome;
    };
    let bundled = plugin::bundle_version(PLUGIN_BUNDLE);
    let installed = plugin::installed_version(&loader.dir);
    let stale = plugin::is_current(&loader.dir, PLUGIN_BUNDLE) != Some(true);
    if stale && (auto || installed.is_some()) {
        match plugin::install(&loader.dir, PLUGIN_BUNDLE) {
            Ok(()) => info!(from = ?installed, to = ?bundled, "plugin installed"),
            Err(error) => warn!(%error, "plugin not installed"),
        }
    }
    match plugin::write_bootstrap(&loader.dir, bridge.port(), bridge.token()) {
        Ok(true) => info!(port = bridge.port(), "plugin bootstrap written"),
        Ok(false) => {}
        Err(error) => warn!(%error, "plugin bootstrap not written"),
    }
    outcome
}

/// Links a loader into the client unless one already is: the configured folder's, or the one
/// winer ships, written to its data folder first. Returns whether it created the link.
fn activate(service: &Service, host: &Host, client_dir: &Path) -> Result<bool, Failure> {
    if let Some(active) = loader(service).filter(|loader| loader.active) {
        // Ours is kept current; a loader the user installed is theirs to update.
        if same_dir(&active.dir, &host.dir)
            && let Err(error) = plugin::place_loader(&host.dir, PENGU_CORE)
        {
            info!(%error, "the loader is in use; it is updated with the client closed");
        }
        return Ok(false);
    }
    let dir = match service.settings().plugin.loader_dir {
        Some(configured) if Path::new(&configured).join("core.dll").is_file() => {
            PathBuf::from(configured)
        }
        _ => {
            plugin::place_loader(&host.dir, PENGU_CORE).map_err(|error| Failure::other(&error))?;
            host.dir.clone()
        }
    };
    // The one write into the client's own folder, and so the one that can need an administrator.
    let linked = plugin::link(client_dir, &dir).map_err(|error| Failure {
        needs_elevation: plugin::needs_elevation(&error, elevation::is_elevated()),
        message: error.to_string(),
    })?;
    match linked {
        Linked::Created => {
            info!(loader = %dir.display(), client = %client_dir.display(), "loader linked");
            Ok(true)
        }
        Linked::Existing => Ok(false),
        Linked::Occupied => {
            warn!(client = %client_dir.display(), "version.dll belongs to something else");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest as _, Sha256};
    use winer_core::{Service, bridge::Bridge};

    use super::{Failure, Host, PENGU_CORE, disable, status};

    /// A refused link reaches the window as a request for administrator rights, beside the
    /// system's own words for it, until turning the features off clears it.
    #[test]
    fn a_link_refused_for_want_of_rights_tells_the_window_so() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tauri::async_runtime::handle().inner().clone(),
        );
        let bridge =
            tauri::async_runtime::block_on(Bridge::start(service.clone(), "0.0.0")).unwrap();
        let host = Host::new(dir.path().join("pengu"));
        let refused = "A required privilege is not held by the client. (os error 1314)";
        host.set_failure(Some(Failure {
            message: refused.into(),
            needs_elevation: true,
        }));
        let shown = status(&service, &bridge, &host);
        assert!(shown.needs_elevation);
        assert_eq!(shown.setup_error.as_deref(), Some(refused));

        host.set_failure(Some(Failure::other(&std::io::Error::from_raw_os_error(32))));
        let shown = status(&service, &bridge, &host);
        assert!(!shown.needs_elevation, "a file in use is not about rights");
        assert!(shown.setup_error.is_some());

        let shown = disable(&service, &bridge, &host).unwrap();
        assert!(!shown.needs_elevation && shown.setup_error.is_none());
    }

    /// The DLL the client loads is the release `vendor/pengu-loader/README.md` names, byte for byte.
    #[test]
    fn core_dll_is_the_vendored_release() {
        let hex: String = Sha256::digest(PENGU_CORE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            hex,
            "d56fcf8f182dd5d392214eb989869c53e6cb7e0bc2a2d111ea0a3b9d3c0a506f"
        );
    }
}
