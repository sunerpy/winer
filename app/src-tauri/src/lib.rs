//! The desktop shell: one window, a tray icon, a global shortcut, the `lcu` asset protocol and the
//! commands the frontend calls. Everything about the League client itself lives in `winer-core`.

mod assets;
mod commands;
mod diagnostics;
mod elevation;
mod events;
mod game_chat;
mod hotkey;
mod logging;
mod plugin_host;
mod storage;
mod tray;
mod updater;
mod window;

use std::{error::Error, path::PathBuf};

use tauri::{App, Manager};
use winer_core::{Service, bridge::Bridge};

/// The app's version, from the root package.json (`build.rs`).
pub(crate) const VERSION: &str = env!("WINER_VERSION");
/// The in-client plugin, embedded by `build.rs`.
pub(crate) const PLUGIN_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/plugin.js"));
/// Pengu Loader's in-client DLL, which winer links into a client that has no loader yet
/// (`vendor/pengu-loader/README.md`).
pub(crate) const PENGU_CORE: &[u8] = include_bytes!("../../../vendor/pengu-loader/core.dll");
pub(crate) const PENGU_VERSION: &str = "1.1.6";
/// The release page the window may send the user to; the window never opens a URL it was given.
pub(crate) const RELEASES_URL: &str = "https://github.com/sunerpy/winer/releases";
/// The documentation's home; its pages are `DocsPage`'s.
pub(crate) const DOCS_URL: &str = "https://firlab.app/winer/";

pub(crate) struct Paths {
    pub log_dir: PathBuf,
    pub settings: PathBuf,
}

pub fn run() {
    elevation::wait_for_replaced_instance();
    // Autostart passes `--minimized`: come up in the tray, not in the user's face.
    let start_hidden = std::env::args().any(|arg| arg == "--minimized");
    tauri::Builder::default()
        // First, so a second launch hands over to the running one before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            window::show(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkey::on_shortcut)
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol("lcu", assets::protocol)
        .invoke_handler(commands::handler())
        .on_window_event(window::on_event)
        .setup(move |app| setup(app, start_hidden))
        .run(tauri::generate_context!())
        .expect("winer failed to start");
}

fn setup(app: &mut App, start_hidden: bool) -> Result<(), Box<dyn Error>> {
    let paths = Paths {
        log_dir: app.path().app_log_dir()?,
        settings: app.path().app_config_dir()?.join("settings.json"),
    };
    // Storage: the log within its limits (`logging.rs`), checked again once a day.
    let logs = logging::Logs::open(&paths.log_dir)?;
    app.manage(logging::init(&logs)?);
    logs.maintain();
    tracing::info!(
        version = VERSION,
        elevated = elevation::is_elevated(),
        "winer starting"
    );
    // Storage: what the last cleanup marked goes before the window's WebView starts (`storage.rs`).
    let local = app.path().app_local_data_dir()?;
    let storage = storage::Storage::new(
        logs,
        local.clone(),
        app.path().app_config_dir()?,
        std::env::temp_dir(),
    );
    storage.at_start();
    app.manage(storage);

    let service = Service::new(
        &paths.settings,
        tauri::async_runtime::handle().inner().clone(),
    );
    service.set_backup_dir(local.join(storage::BACKUPS));
    let bridge = tauri::async_runtime::block_on(Bridge::start(service.clone(), VERSION))?;
    app.manage(service.clone());
    app.manage(bridge.clone());
    app.manage(paths);
    app.manage(updater::Updater::default());
    app.manage(plugin_host::Host::new(local.join(storage::PENGU)));
    app.manage(hotkey::Hotkey::default());
    // On the main thread here, so it is registered before the window shows.
    hotkey::apply(app.handle(), service.settings().general.hotkey);
    // Callout: the shortcut that sends it, none by default.
    hotkey::apply_callout(app.handle(), service.settings().automation.callout.hotkey);

    // Subscribed before the service starts: the core's events are not replayed, and the first
    // `Connected` is what sets up the loader and points the plugin at this session's bridge.
    events::forward(app.handle().clone(), service.clone(), bridge);
    service.start();
    window::create(app)?;
    tray::create(app.handle(), &service.settings())?;
    updater::check_in_background(app.handle().clone());
    if !(start_hidden && service.settings().general.close_to_tray) {
        window::show(app.handle());
    }
    Ok(())
}
