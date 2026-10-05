//! The shell's half of the in-client plugin: which loader, which bundle, and the bootstrap file
//! that tells the plugin where this session's bridge listens.

use std::path::Path;

use tracing::{info, warn};
use winer_core::{
    CoreError, Service,
    bridge::Bridge,
    plugin::{self, Loader, PluginStatus},
};

use crate::PLUGIN_BUNDLE;

fn loader(service: &Service) -> Option<Loader> {
    let configured = service.settings().plugin.loader_dir;
    plugin::find_loader(
        service.client_dir().as_deref(),
        configured.as_deref().map(Path::new),
    )
}

pub(crate) fn status(service: &Service, bridge: &Bridge) -> PluginStatus {
    let loader = loader(service);
    PluginStatus {
        loader_dir: loader
            .as_ref()
            .map(|loader| loader.dir.display().to_string()),
        active: loader.as_ref().is_some_and(|loader| loader.active),
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

pub(crate) fn install(service: &Service, bridge: &Bridge) -> Result<PluginStatus, CoreError> {
    let loader =
        loader(service).ok_or_else(|| CoreError::Invalid("Pengu Loader was not found".into()))?;
    plugin::install(&loader.dir, PLUGIN_BUNDLE)?;
    plugin::write_bootstrap(&loader.dir, bridge.port(), bridge.token())?;
    info!(dir = %loader.dir.display(), "plugin installed");
    Ok(status(service, bridge))
}

pub(crate) fn uninstall(service: &Service, bridge: &Bridge) -> Result<PluginStatus, CoreError> {
    if let Some(loader) = loader(service) {
        plugin::uninstall(&loader.dir)?;
        info!(dir = %loader.dir.display(), "plugin removed");
    }
    Ok(status(service, bridge))
}

/// On every client connection: an installed winer plugin is brought to this build's bundle (by
/// content: two builds can share a version) and given this session's bridge address. A plugin
/// that is not installed stays not installed, and a foreign file in its folder is left alone.
pub(crate) fn refresh(service: &Service, bridge: &Bridge) {
    let Some(loader) = loader(service) else {
        return;
    };
    let bundled = plugin::bundle_version(PLUGIN_BUNDLE);
    let installed = plugin::installed_version(&loader.dir);
    if installed.is_some() && plugin::is_current(&loader.dir, PLUGIN_BUNDLE) == Some(false) {
        match plugin::install(&loader.dir, PLUGIN_BUNDLE) {
            Ok(()) => info!(from = ?installed, to = ?bundled, "plugin updated"),
            Err(error) => warn!(%error, "plugin update failed"),
        }
    }
    match plugin::write_bootstrap(&loader.dir, bridge.port(), bridge.token()) {
        Ok(true) => info!(port = bridge.port(), "plugin bootstrap written"),
        Ok(false) => {}
        Err(error) => warn!(%error, "plugin bootstrap not written"),
    }
}
