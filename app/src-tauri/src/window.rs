//! The main window: shown from anywhere, and hidden rather than closed while the tray holds the app.

use tauri::{App, AppHandle, Manager, Runtime, WebviewWindowBuilder, Window, WindowEvent};
use winer_core::Service;

pub(crate) const MAIN: &str = "main";

/// wry's own browser arguments, restated: passing any replaces them (`docs/platform-notes.md`).
const WRY_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
/// The WebView's page cache at most (`--disk-cache-size`, one of WebView2's documented flags).
/// The window's own pages and pictures come through winer's protocols, which WebView2 does not
/// cache on disk (measured empty after a day of use), so this only bounds what anything else would
/// leave there.
pub(crate) const DISK_CACHE_BYTES: u64 = 32 * 1024 * 1024;

/// What the window's WebView starts with; a QA build adds the DevTools port.
fn browser_args(devtools_port: Option<&str>) -> String {
    let mut args = format!("{WRY_ARGS} --disk-cache-size={DISK_CACHE_BYTES}");
    if let Some(port) = devtools_port {
        args.push_str(&format!(" --remote-debugging-port={port}"));
    }
    args
}

/// Builds the main window from its `tauri.conf.json` entry (`create: false` there), so it can have
/// browser arguments the config cannot express.
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
    let mut devtools_port = None::<String>;
    #[cfg(feature = "qa")]
    if let Ok(port) = std::env::var("WINER_DEVTOOLS_PORT") {
        tracing::warn!(%port, "QA build: WebView2 DevTools port open");
        devtools_port = Some(port);
    }
    WebviewWindowBuilder::from_config(app.handle(), &config)?
        .additional_browser_args(&browser_args(devtools_port.as_deref()))
        .build()
        .map(drop)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_webview_starts_with_wrys_arguments_and_a_bounded_page_cache() {
        assert_eq!(
            browser_args(None),
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
             --disk-cache-size=33554432"
        );
        assert_eq!(
            browser_args(Some("9333")),
            format!("{} --remote-debugging-port=9333", browser_args(None))
        );
    }
}
