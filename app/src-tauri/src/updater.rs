//! Signed updates, driven from Rust. The window only renders [`UpdateStatus`]; it never touches
//! the updater plugin itself.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt as _};
use tracing::{info, warn};
use winer_core::view::{ErrorCode, IpcError, UpdateStatus};

use crate::{VERSION, events};

/// Download progress is published at most once per this many bytes, and always at the end.
const PROGRESS_STEP: u64 = 512 * 1024;

#[derive(Default)]
pub(crate) struct Updater {
    status: Mutex<UpdateStatus>,
    busy: AtomicBool,
    /// What the last check found, kept so Install does not ask the server twice.
    pending: Mutex<Option<Update>>,
}

impl Updater {
    pub(crate) fn status(&self) -> UpdateStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
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

pub(crate) async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateStatus, IpcError> {
    let updater = app.state::<Updater>();
    updater.begin()?;
    updater.publish(app, UpdateStatus::Checking);
    let status = match find(app).await {
        Ok(Some(update)) => {
            info!(version = %update.version, "update available");
            let status = available(&update);
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(update);
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

/// Downloads, verifies and installs, then restarts. On Windows the installer takes over and this
/// process exits inside `install`.
pub(crate) async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), IpcError> {
    let updater = app.state::<Updater>();
    updater.begin()?;
    let pending = updater
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    let update = match pending {
        Some(update) => update,
        None => match find(app).await {
            Ok(Some(update)) => update,
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

    use super::describe;

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
}
