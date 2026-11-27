//! Signed updates, driven from Rust. The window only renders [`UpdateStatus`]; it never touches
//! the updater plugin itself.
//!
//! With `general.autoUpdate` on (the default) winer looks on its own: [`FIRST_CHECK`] after start,
//! then every [`CHECK_EVERY`], and at once when the switch is turned on. A look downloads and
//! verifies what it finds and stops at [`UpdateStatus::Ready`]; the title bar then offers the
//! restart, and only the user's click installs. A check by hand that finds an update downloads it
//! the same way while the switch is on. Off, winer asks the update server only when the user checks.

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
use tokio::sync::Notify;
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
    /// `general.autoUpdate` as last read: winer looks for updates on its own and downloads them.
    auto: AtomicBool,
    /// Wakes the looks on their own when the switch is turned on.
    wake: Notify,
}

/// One check for updates as the self-check reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LastCheck {
    /// Epoch milliseconds.
    pub at: i64,
    pub ok: bool,
}

impl Updater {
    /// With the switch as the settings have it at start.
    pub(crate) fn new(auto: bool) -> Self {
        Self {
            auto: AtomicBool::new(auto),
            ..Self::default()
        }
    }

    /// The settings changed: keep the switch, and look at once when it was just turned on.
    pub(crate) fn follow(&self, on: bool) {
        let was = self.auto.swap(on, Ordering::AcqRel);
        if on && !was {
            self.wake.notify_one();
        }
    }

    /// The looks on their own: `look` runs [`FIRST_CHECK`] after start and [`CHECK_EVERY`] after
    /// each one while the switch is on, and at once when it is turned on; never while it is off.
    async fn looks<F, Fut>(&self, mut look: F)
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut wait = FIRST_CHECK;
        loop {
            // Either the time is up or the switch was just turned on.
            let _ = tokio::time::timeout(wait, self.wake.notified()).await;
            if self.auto.load(Ordering::Acquire) {
                look().await;
            }
            wait = CHECK_EVERY;
        }
    }

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

/// The looks on their own for as long as winer runs ([`Updater::looks`]).
pub(crate) fn check_in_background<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        app.state::<Updater>().looks(|| look(&app)).await;
    });
}

/// The settings changed: follow `general.autoUpdate` ([`Updater::follow`]).
pub(crate) fn follow<R: Runtime>(app: &AppHandle<R>, on: bool) {
    if let Some(updater) = app.try_state::<Updater>() {
        updater.follow(on);
    }
}

/// One look on its own: the manifest is asked unless an update is known already, and what it finds
/// is downloaded and verified, ending at [`UpdateStatus::Ready`]. A look never installs, and never
/// interrupts a check or an install the user started. A failed check stays quiet; a failed
/// download leaves the update found. The next look tries again.
async fn look<R: Runtime>(app: &AppHandle<R>) {
    let updater = app.state::<Updater>();
    if matches!(
        updater.status(),
        UpdateStatus::Downloading { .. }
            | UpdateStatus::Ready { .. }
            | UpdateStatus::Installing { .. }
    ) || updater.begin().is_err()
    {
        return;
    }
    let known = updater
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    let update = match known {
        Some(Pending::Available(update)) => update,
        // Downloaded already: the restart is the user's.
        Some(ready @ Pending::Ready { .. }) => {
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ready);
            updater.busy.store(false, Ordering::Release);
            return;
        }
        None => {
            info!("checking for updates on its own");
            let found = find(app).await;
            updater.checked(found.is_ok());
            match found {
                Ok(Some(update)) => {
                    info!(version = %update.version, "update available");
                    updater.publish(app, available(&update));
                    update
                }
                Ok(None) => {
                    updater.finish(
                        app,
                        UpdateStatus::UpToDate {
                            version: VERSION.into(),
                            checked_at: now_ms(),
                        },
                    );
                    return;
                }
                Err(error) => {
                    warn!(error = %describe(&error), "background update check failed");
                    updater.busy.store(false, Ordering::Release);
                    return;
                }
            }
        }
    };
    let version = update.version.clone();
    match download(app, &updater, &update).await {
        Ok(bytes) => {
            info!(%version, "update downloaded and verified in the background");
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Pending::Ready { update, bytes });
            updater.finish(app, UpdateStatus::Ready { version });
        }
        Err(error) => {
            warn!(%version, error = %describe(&error), "background update download failed");
            let status = available(&update);
            *updater
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Pending::Available(update));
            updater.finish(app, status);
        }
    }
}

/// Downloads `update` and verifies its signature, publishing the progress on the way.
async fn download<R: Runtime>(
    app: &AppHandle<R>,
    updater: &Updater,
    update: &Update,
) -> Result<Vec<u8>, tauri_plugin_updater::Error> {
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
    update
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
        .await
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
    // Switched on, an update found by hand downloads as one found on its own does.
    if matches!(status, UpdateStatus::Available { .. }) && updater.auto.load(Ordering::Acquire) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { look(&app).await });
    }
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
            let bytes = match download(app, &updater, &update).await {
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

    use std::{cell::RefCell, time::Duration};

    use super::{
        CHECK_EVERY, FIRST_CHECK, Updater, describe, set_windows_install_mode, windows_install_mode,
    };
    use tauri::utils::config::{BundleType, PluginConfig};
    use tokio::time::{Instant, sleep};

    const HOUR: Duration = Duration::from_secs(60 * 60);

    /// Runs the updater's looks on its own for `span` while `drive` moves the switch, and returns
    /// when each look ran, from the start.
    async fn looks_within(
        updater: &Updater,
        span: Duration,
        drive: impl Future<Output = ()>,
    ) -> Vec<Duration> {
        let start = Instant::now();
        let seen = RefCell::new(Vec::new());
        let looks = updater.looks(|| {
            seen.borrow_mut().push(start.elapsed());
            std::future::ready(())
        });
        let until = async {
            drive.await;
            sleep(span.saturating_sub(start.elapsed())).await;
        };
        tokio::select! {
            () = looks => unreachable!("the looks never end"),
            () = until => {}
        }
        seen.into_inner()
    }

    #[tokio::test(start_paused = true)]
    async fn switched_on_it_looks_soon_after_start_then_every_few_hours() {
        let updater = Updater::new(true);
        let looks = looks_within(&updater, 13 * HOUR, async {}).await;
        assert_eq!(
            looks,
            [
                FIRST_CHECK,
                FIRST_CHECK + CHECK_EVERY,
                FIRST_CHECK + 2 * CHECK_EVERY
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn switched_off_it_never_looks_on_its_own() {
        let updater = Updater::new(false);
        assert_eq!(
            looks_within(&updater, 13 * HOUR, async {}).await,
            [Duration::ZERO; 0]
        );
        // Settings that change something else leave it off.
        let updater = Updater::new(false);
        let looks = looks_within(&updater, 13 * HOUR, async {
            sleep(HOUR).await;
            updater.follow(false);
        })
        .await;
        assert_eq!(looks, [Duration::ZERO; 0]);
    }

    #[tokio::test(start_paused = true)]
    async fn turned_on_it_looks_at_once_and_turned_off_it_stops() {
        let updater = Updater::new(false);
        let looks = looks_within(&updater, 10 * HOUR, async {
            sleep(HOUR).await;
            updater.follow(true);
            // Saved again with the switch still on: no second look.
            sleep(HOUR).await;
            updater.follow(true);
        })
        .await;
        assert_eq!(looks, [HOUR, HOUR + CHECK_EVERY]);

        let updater = Updater::new(true);
        let looks = looks_within(&updater, 13 * HOUR, async {
            sleep(HOUR).await;
            updater.follow(false);
        })
        .await;
        assert_eq!(looks, [FIRST_CHECK], "none after it went off");
    }

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
