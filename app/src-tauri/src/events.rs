//! Core events to the window, and the few things the shell does itself when they arrive.

use tauri::{AppHandle, Emitter as _, Runtime};
use tokio::sync::broadcast::error::RecvError;
use tracing::warn;
use winer_core::{
    Service,
    bridge::Bridge,
    view::{Connection, Event, Patch},
};

use crate::{plugin_host, tray};

/// Every core [`Event`], unchanged.
pub(crate) const EVENT: &str = "winer://event";
/// The window missed events and must read the snapshot again.
pub(crate) const RESYNC: &str = "winer://resync";
/// A new [`winer_core::view::UpdateStatus`].
pub(crate) const UPDATE: &str = "winer://update";

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
        // A client came up: point its plugin at this session's bridge.
        Event::Update(update)
            if matches!(
                update.patch,
                Patch::Connection(Connection::Connected { .. })
            ) =>
        {
            let (service, bridge) = (service.clone(), bridge.clone());
            tauri::async_runtime::spawn_blocking(move || plugin_host::refresh(&service, &bridge));
        }
        Event::Settings(settings) => tray::sync(app, settings),
        _ => {}
    }
}
