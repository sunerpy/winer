//! Core events to the window, and the few things the shell does itself when they arrive.

use tauri::{AppHandle, Emitter as _, Manager as _, Runtime};
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};
use winer_core::{
    Service,
    bridge::Bridge,
    view::{Connection, Event, Patch},
};

use crate::{elevation, hotkey, plugin_host, tray, window};

/// Every core [`Event`], unchanged.
pub(crate) const EVENT: &str = "winer://event";
/// The window missed events and must read the snapshot again.
pub(crate) const RESYNC: &str = "winer://resync";
/// A new [`winer_core::view::UpdateStatus`].
pub(crate) const UPDATE: &str = "winer://update";
/// A new [`winer_core::view::HotkeyStatus`].
pub(crate) const HOTKEY: &str = "winer://hotkey";

pub(crate) fn forward<R: Runtime>(app: AppHandle<R>, service: Service, bridge: Bridge) {
    let mut events = service.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    react(&app, &service, &bridge, &event);
                    if let Err(error) = app.emit(EVENT, &event) {
                        warn!(%error, "event not delivered to the window");
                    }
                }
                Err(RecvError::Lagged(missed)) => {
                    warn!(missed, "the window fell behind; asking it to resync");
                    let _ = app.emit(RESYNC, ());
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

fn react<R: Runtime>(app: &AppHandle<R>, service: &Service, bridge: &Bridge, event: &Event) {
    match event {
        // A client came up: set up the loader and the plugin, and point it at this session's
        // bridge. A loader linked just now starts with the client's interface, which is restarted
        // for it while the player is idle; otherwise it starts with the client's next launch.
        // Only an administrator can create the link: without those rights winer restarts
        // elevated, as for an elevated client, and the elevated copy links on its connection.
        Event::Update(update)
            if matches!(
                update.patch,
                Patch::Connection(Connection::Connected { .. })
            ) =>
        {
            let (app, service, bridge) = (app.clone(), service.clone(), bridge.clone());
            tauri::async_runtime::spawn(async move {
                let outcome = {
                    let (app, service) = (app.clone(), service.clone());
                    tauri::async_runtime::spawn_blocking(move || {
                        plugin_host::refresh(&service, &bridge, &app.state::<plugin_host::Host>())
                    })
                    .await
                    .unwrap_or_default()
                };
                if outcome.needs_elevation {
                    elevation::ask_once(&app, "only an administrator can link the loader");
                }
                if outcome.linked {
                    match service.restart_client_ui_when_idle().await {
                        Ok(true) => info!("client interface restarted to load the new loader"),
                        Ok(false) => info!("the loader starts with the client's next launch"),
                        Err(error) => warn!(%error, "client interface not restarted"),
                    }
                }
            });
        }
        // The client runs elevated and winer does not: restart elevated rather than ask first.
        Event::Update(update)
            if matches!(update.patch, Patch::Connection(Connection::AccessDenied)) =>
        {
            elevation::ask_once(app, "the client runs elevated");
        }
        Event::Settings(settings) => {
            tray::sync(app, settings);
            hotkey::apply(app, settings.general.hotkey.clone());
            // Callout: its own shortcut.
            hotkey::apply_callout(app, settings.automation.callout.hotkey.clone());
        }
        // Social: a click in the client asked for a player's history, which the window opens.
        Event::OpenHistory { .. } => window::show(app),
        Event::Update(update) => {
            if let Patch::Phase(phase) = update.patch {
                hotkey::on_phase(app, phase);
            }
        }
        _ => {}
    }
}
