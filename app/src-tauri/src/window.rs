//! The main window: shown from anywhere, and hidden rather than closed while the tray holds the app.

use tauri::{App, AppHandle, Manager, Runtime, WebviewWindowBuilder, Window, WindowEvent};
use winer_core::Service;

pub(crate) const MAIN: &str = "main";

/// Builds the main window from its `tauri.conf.json` entry (`create: false` there), so a QA build
/// can add browser arguments the config cannot express.
pub(crate) fn create<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN)
        .cloned()
        .expect("tauri.conf.json declares the main window");
    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::from_config(app.handle(), &config)?;
    #[cfg(feature = "qa")]
    if let Ok(port) = std::env::var("WINER_DEVTOOLS_PORT") {
        // Replaces wry's own default arguments, so they are restated.
        builder = builder.additional_browser_args(&format!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
        ));
        tracing::warn!(%port, "QA build: WebView2 DevTools port open");
    }
    builder.build().map(drop)
}

pub(crate) fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The close button's contract: with "close to tray" on, the window hides and the process stays
/// (acceptance asserts `IsWindowVisible` going false with the process alive, not an exit).
pub(crate) fn on_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let close_to_tray = window
            .app_handle()
            .try_state::<Service>()
            .is_none_or(|service| service.settings().general.close_to_tray);
        if close_to_tray {
            api.prevent_close();
            // Hidden, it is no longer above the game the hotkey pinned it over.
            crate::hotkey::unpin(window.app_handle());
            let _ = window.hide();
        } else {
            window.app_handle().exit(0);
        }
    }
}
