//! Signed updates, driven from Rust. The window only renders [`UpdateStatus`]; it never touches
//! the updater plugin itself.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(any(windows, test))]
use tauri::utils::config::{BundleType, PluginConfig};
use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt as _};
use tracing::{info, warn};
use winer_core::view::{ErrorCode, IpcError, UpdateStatus};

use crate::{VERSION, events};

/// Download progress is published at most once per this many bytes, and always at the end.
const PROGRESS_STEP: u64 = 512 * 1024;
/// The first check on its own waits this long after start, so it never competes with start-up.
const FIRST_CHECK: Duration = Duration::from_secs(20);
/// Between checks on its own while winer keeps running.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

enum Pending {
    /// Found by a manifest check, not downloaded yet.
    Available(Update),
    /// Downloaded and signature-verified. The bytes stay in memory until the user restarts.
    Ready { update: Update, bytes: Vec<u8> },
}

#[derive(Default)]
pub(crate) struct Updater {
    status: Mutex<UpdateStatus>,
    busy: AtomicBool,
    /// What the last check found, or what was downloaded since, kept so Install does not ask the
    /// server twice.
    pending: Mutex<Option<Pending>>,
    /// How the last check went, the quiet ones included: those leave `status` alone.
    last_check: Mutex<Option<LastCheck>>,
}

/// One check for updates as the self-check reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LastCheck {
    /// Epoch milliseconds.
    pub at: i64,
    pub ok: bool,
}

impl Updater {
    pub(crate) fn status(&self) -> UpdateStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn last_check(&self) -> Option<LastCheck> {
        *self
            .last_check
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn checked(&self, ok: bool) {
        *self
            .last_check
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(LastCheck { at: now_ms(), ok });
    }

    fn begin(&self) -> Result<(), IpcError> {
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err(IpcError {
                code: ErrorCode::Busy,
                message: "an update is already running".into(),
            });
        }
        Ok(())
    }

    fn publish<R: Runtime>(&self, app: &AppHandle<R>, status: UpdateStatus) {
        let mut current = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = status.clone();
        let _ = app.emit(events::UPDATE, status);
    }

    /// Ends a run. The slot is freed before the final status goes out, so a window reacting to
    /// it (Up to date → Install) is never refused as busy.
    fn finish<R: Runtime>(&self, app: &AppHandle<R>, status: UpdateStatus) {
        let mut current = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.busy.store(false, Ordering::Release);
        *current = status.clone();
        let _ = app.emit(events::UPDATE, status);
    }
}

async fn find<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<Update>, tauri_plugin_updater::Error> {
    // The plugin sets no timeouts of its own; a host that goes quiet would hang the check.
    app.updater_builder()
        .configure_client(|client| {
            client
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(30))
        })
        .build()?
        .check()
        .await
}

fn available(update: &Update) -> UpdateStatus {
    UpdateStatus::Available {
        version: update.version.clone(),
        current: update.current_version.clone(),
        notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
        date: update.date.map(|date| date.date().to_string()),
    }
}

/// Checks on its own: soon after start, then every few hours. An update found shows as the title
/// bar's badge. A failed check stays quiet, to be retried at the next interval, and a check or
/// install the user started is never interrupted.
pub(crate) fn check_in_background<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            quiet_check(&app).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn quiet_check<R: Runtime>(app: &AppHandle<R>) {
    let updater = app.state::<Updater>();
    if matches!(
        updater.status(),
        UpdateStatus::Available { .. }
            | UpdateStatus::Downloading { .. }
            | UpdateStatus::Ready { .. }
            | UpdateStatus::Installing { .. }
    ) || updater.begin().is_err()
    {
        return;
    }
    let found = find(app).await;
    updater.checked(found.is_ok());
    match found {
        Ok(Some(update)) => {
            info!(version = %update.version, "update available");
            let status = available(&update);
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Pending::Available(update));
            updater.finish(app, status);
        }
        Ok(None) => updater.finish(
            app,
            UpdateStatus::UpToDate {
                version: VERSION.into(),
                checked_at: now_ms(),
            },
        ),
        Err(error) => {
            warn!(error = %describe(&error), "background update check failed");
            updater.busy.store(false, Ordering::Release);
        }
    }
}

pub(crate) async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateStatus, IpcError> {
    let updater = app.state::<Updater>();
    updater.begin()?;
    updater
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    updater.publish(app, UpdateStatus::Checking);
    let found = find(app).await;
    updater.checked(found.is_ok());
    let status = match found {
        Ok(Some(update)) => {
            info!(version = %update.version, "update available");
            let status = available(&update);
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Pending::Available(update));
            status
        }
        Ok(None) => UpdateStatus::UpToDate {
            version: VERSION.into(),
            checked_at: now_ms(),
        },
        Err(error) => {
            warn!(error = %describe(&error), "update check failed");
            UpdateStatus::Failed {
                message: describe(&error),
            }
        }
    };
    updater.finish(app, status.clone());
    Ok(status)
}

/// First intent downloads and verifies an available update, stopping at [`UpdateStatus::Ready`].
/// A second explicit intent installs those same bytes and restarts. On Windows the installer takes
/// over and this process exits inside `install`.
pub(crate) async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), IpcError> {
    let updater = app.state::<Updater>();
    updater.begin()?;
    let pending = updater
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();

    let pending = match pending {
        Some(pending) => pending,
        None => match find(app).await {
            Ok(Some(update)) => Pending::Available(update),
            Ok(None) => {
                updater.finish(
                    app,
                    UpdateStatus::UpToDate {
                        version: VERSION.into(),
                        checked_at: now_ms(),
                    },
                );
                return Ok(());
            }
            Err(error) => {
                updater.finish(
                    app,
                    UpdateStatus::Failed {
                        message: describe(&error),
                    },
                );
                return Ok(());
            }
        },
    };

    let (update, bytes) = match pending {
        Pending::Ready { update, bytes } => (update, bytes),
        Pending::Available(update) => {
            let version = update.version.clone();
            let (mut received, mut published) = (0u64, 0u64);
            updater.publish(
                app,
                UpdateStatus::Downloading {
                    version: version.clone(),
                    received: 0,
                    total: None,
                },
            );
            let download = update
                .download(
                    |chunk, total| {
                        received += chunk as u64;
                        if received - published >= PROGRESS_STEP || Some(received) == total {
                            published = received;
                            updater.publish(
                                app,
                                UpdateStatus::Downloading {
                                    version: version.clone(),
                                    received,
                                    total,
                                },
                            );
                        }
                    },
                    || {},
                )
                .await;
            let bytes = match download {
                Ok(bytes) => bytes,
                Err(error) => {
                    updater.finish(
                        app,
                        UpdateStatus::Failed {
                            message: describe(&error),
                        },
                    );
                    return Ok(());
                }
            };
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Pending::Ready { update, bytes });
            updater.finish(app, UpdateStatus::Ready { version });
            return Ok(());
        }
    };

    let version = update.version.clone();
    updater.publish(
        app,
        UpdateStatus::Installing {
            version: version.clone(),
        },
    );
    info!(%version, "installing update");
    let installed = tauri::async_runtime::spawn_blocking(move || update.install(bytes)).await;
    match installed {
        Ok(Ok(())) => app.restart(),
        Ok(Err(error)) => updater.finish(
            app,
            UpdateStatus::Failed {
                message: describe(&error),
            },
        ),
        Err(error) => updater.finish(
            app,
            UpdateStatus::Failed {
                message: error.to_string(),
            },
        ),
    }
    Ok(())
}

/// NSIS current-user updates can run without UI; MSI and unknown bundle types stay passive so a
/// per-machine package can still show its UAC prompt.
#[cfg(any(windows, test))]
pub(crate) fn windows_install_mode(bundle: Option<BundleType>) -> &'static str {
    match bundle {
        Some(BundleType::Nsis) => "quiet",
        _ => "passive",
    }
}

/// The updater plugin deserializes its config once at startup, so the bundle-specific mode must be
/// written into the context before the builder runs.
#[cfg(any(windows, test))]
pub(crate) fn set_windows_install_mode(plugins: &mut PluginConfig, mode: &str) {
    let Some(updater) = plugins
        .0
        .get_mut("updater")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    let windows = updater
        .entry("windows")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !windows.is_object() {
        *windows = serde_json::Value::Object(serde_json::Map::new());
    }
    windows
        .as_object_mut()
        .expect("windows updater config is an object")
        .insert("installMode".into(), mode.into());
}

/// The whole cause chain: the plugin's own message is only its outermost line.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let line = cause.to_string();
        if !text.contains(&line) {
            text.push_str(": ");
            text.push_str(&line);
        }
        source = cause.source();
    }
    text
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fmt};

    use super::{describe, set_windows_install_mode, windows_install_mode};
    use tauri::utils::config::{BundleType, PluginConfig};

    #[derive(Debug)]
    struct Outer(Inner);
    #[derive(Debug)]
    struct Inner;

    impl fmt::Display for Outer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("error sending request")
        }
    }

    impl fmt::Display for Inner {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("connection refused")
        }
    }

    impl Error for Inner {}

    impl Error for Outer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn the_cause_chain_is_kept() {
        assert_eq!(
            describe(&Outer(Inner)),
            "error sending request: connection refused"
        );
    }
    #[test]
    fn nsis_updates_are_quiet_but_msi_and_unknown_packages_stay_passive() {
        assert_eq!(windows_install_mode(Some(BundleType::Nsis)), "quiet");
        assert_eq!(windows_install_mode(Some(BundleType::Msi)), "passive");
        assert_eq!(windows_install_mode(None), "passive");
    }

    #[test]
    fn the_install_mode_is_patched_before_the_plugin_reads_its_config() {
        let mut plugins = PluginConfig::default();
        plugins.0.insert(
            "updater".into(),
            serde_json::json!({
                "pubkey": "key",
                "endpoints": ["https://example.invalid/latest.json"],
                "windows": { "installMode": "passive" }
            }),
        );
        set_windows_install_mode(&mut plugins, "quiet");
        let config: tauri_plugin_updater::Config =
            serde_json::from_value(plugins.0["updater"].clone()).unwrap();
        assert_eq!(config.windows.unwrap().install_mode.to_string(), "quiet");
    }
}
