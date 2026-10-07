//! The self-check, put together: the core's checks of the client and the internet
//! (`Service::diagnose`), then what the shell holds itself: the loader and the plugin, the
//! shortcuts, and how the last check for updates went. Settings › About runs it on demand; the
//! connection handler runs it when the client comes back on a new version (`events`).

use tauri::{AppHandle, Manager as _, Runtime};
use winer_core::{
    Service,
    bridge::Bridge,
    plugin::PluginStatus,
    view::{Check, CheckId, CheckReason, CheckStatus, Connection, DiagnosticsReport, HotkeyStatus},
};

use crate::{
    hotkey, plugin_host,
    updater::{LastCheck, Updater},
};

pub(crate) async fn run<R: Runtime>(
    app: &AppHandle<R>,
    service: &Service,
    bridge: &Bridge,
) -> DiagnosticsReport {
    let mut checks = service.diagnose().await;
    let plugin = {
        let (app, service, bridge) = (app.clone(), service.clone(), bridge.clone());
        // It reads the client's folder and the registry.
        tauri::async_runtime::spawn_blocking(move || {
            plugin_host::status(&service, &bridge, &app.state::<plugin_host::Host>())
        })
        .await
        .ok()
    };
    let connected = matches!(service.snapshot().connection, Connection::Connected { .. });
    let auto = service.settings().plugin.auto;
    checks.push(match plugin {
        Some(status) => plugin_check(&status, auto, connected),
        None => check(
            CheckId::Plugin,
            CheckStatus::Unknown,
            CheckReason::LoaderInactive,
            None,
        ),
    });
    checks.push(hotkey_check(&hotkey::status(app)));
    checks.push(updater_check(app.state::<Updater>().last_check()));
    let client_version = checks
        .iter()
        .find(|check| check.id == CheckId::Client && check.status == CheckStatus::Ok)
        .and_then(|check| check.detail.clone());
    DiagnosticsReport {
        at: now_ms(),
        client_version,
        checks,
    }
}

fn check(id: CheckId, status: CheckStatus, reason: CheckReason, detail: Option<String>) -> Check {
    Check {
        id,
        status,
        reason,
        detail,
        failure: None,
        took_ms: 0,
    }
}

/// The loader and the plugin, worst first. Another program launching the client comes before
/// everything: turning the features off removes winer's own link, never that program's.
fn plugin_check(status: &PluginStatus, auto: bool, connected: bool) -> Check {
    let (state, reason, detail) = if let Some(program) = &status.foreign_activation {
        (
            CheckStatus::Warn,
            CheckReason::ForeignLoader,
            Some(program.clone()),
        )
    } else if !connected {
        // The link is read from the client's folder, which only a connected client names.
        (CheckStatus::Skipped, CheckReason::NotConnected, None)
    } else if !auto && status.installed_version.is_none() {
        (CheckStatus::Skipped, CheckReason::PluginOff, None)
    } else if !status.active {
        (CheckStatus::Fail, CheckReason::LoaderInactive, None)
    } else if !status.current {
        (CheckStatus::Warn, CheckReason::PluginStale, None)
    } else if status.connected == 0 {
        (CheckStatus::Warn, CheckReason::BridgeIdle, None)
    } else {
        (CheckStatus::Ok, CheckReason::Fine, None)
    };
    check(CheckId::Plugin, state, reason, detail)
}

/// The window's shortcut and the callout's: refused when the system refused either.
fn hotkey_check(status: &HotkeyStatus) -> Check {
    if status.shortcut.is_none() && status.callout.shortcut.is_none() {
        return check(
            CheckId::Hotkey,
            CheckStatus::Skipped,
            CheckReason::NoHotkey,
            None,
        );
    }
    match status
        .error
        .clone()
        .or_else(|| status.callout.error.clone())
    {
        Some(error) => check(
            CheckId::Hotkey,
            CheckStatus::Fail,
            CheckReason::HotkeyRefused,
            Some(error),
        ),
        None => check(CheckId::Hotkey, CheckStatus::Ok, CheckReason::Fine, None),
    }
}

/// How the last check for updates went; this asks the server nothing.
fn updater_check(last: Option<LastCheck>) -> Check {
    match last {
        None => check(
            CheckId::Updater,
            CheckStatus::Unknown,
            CheckReason::NeverChecked,
            None,
        ),
        Some(LastCheck { ok: false, .. }) => check(
            CheckId::Updater,
            CheckStatus::Warn,
            CheckReason::CheckFailed,
            None,
        ),
        Some(_) => check(CheckId::Updater, CheckStatus::Ok, CheckReason::Fine, None),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use winer_core::view::CalloutHotkeyStatus;

    use super::*;

    fn plugin(change: impl FnOnce(&mut PluginStatus)) -> PluginStatus {
        let mut status = PluginStatus {
            loader_dir: Some("pengu".into()),
            active: true,
            managed: true,
            bundled_loader: "1.1.6".into(),
            occupied: false,
            setup_error: None,
            needs_elevation: false,
            installed_version: Some("0.0.8".into()),
            bundled_version: "0.0.8".into(),
            current: true,
            connected: 2,
            foreign_activation: None,
        };
        change(&mut status);
        status
    }

    fn outcome(check: &Check) -> (CheckStatus, CheckReason) {
        (check.status, check.reason)
    }

    #[test]
    fn the_plugin_check_reports_the_worst_first() {
        assert_eq!(
            outcome(&plugin_check(&plugin(|_| {}), true, true)),
            (CheckStatus::Ok, CheckReason::Fine)
        );
        // Turned off, nothing installed, yet another program launches the client: still a warning.
        let foreign = plugin(|status| {
            status.foreign_activation = Some("Pengu Loader.exe".into());
            status.active = false;
            status.installed_version = None;
        });
        let shown = plugin_check(&foreign, false, true);
        assert_eq!(
            outcome(&shown),
            (CheckStatus::Warn, CheckReason::ForeignLoader)
        );
        assert_eq!(shown.detail.as_deref(), Some("Pengu Loader.exe"));
        assert_eq!(
            outcome(&plugin_check(&foreign, true, false)),
            (CheckStatus::Warn, CheckReason::ForeignLoader),
            "the registry needs no client"
        );

        let off = plugin(|status| {
            status.active = false;
            status.installed_version = None;
        });
        assert_eq!(
            outcome(&plugin_check(&off, false, true)),
            (CheckStatus::Skipped, CheckReason::PluginOff)
        );
        assert_eq!(
            outcome(&plugin_check(&off, true, true)),
            (CheckStatus::Fail, CheckReason::LoaderInactive)
        );
        assert_eq!(
            outcome(&plugin_check(&off, true, false)),
            (CheckStatus::Skipped, CheckReason::NotConnected)
        );
        assert_eq!(
            outcome(&plugin_check(
                &plugin(|status| status.current = false),
                true,
                true
            )),
            (CheckStatus::Warn, CheckReason::PluginStale)
        );
        assert_eq!(
            outcome(&plugin_check(
                &plugin(|status| status.connected = 0),
                true,
                true
            )),
            (CheckStatus::Warn, CheckReason::BridgeIdle)
        );
    }

    #[test]
    fn a_refused_shortcut_fails_with_the_systems_words() {
        let none = HotkeyStatus::default();
        assert_eq!(
            outcome(&hotkey_check(&none)),
            (CheckStatus::Skipped, CheckReason::NoHotkey)
        );
        let held = HotkeyStatus {
            shortcut: Some("Alt+Backquote".into()),
            active: true,
            ..HotkeyStatus::default()
        };
        assert_eq!(
            outcome(&hotkey_check(&held)),
            (CheckStatus::Ok, CheckReason::Fine)
        );
        let refused = HotkeyStatus {
            callout: CalloutHotkeyStatus {
                shortcut: Some("F8".into()),
                active: false,
                error: Some("HotKey already registered".into()),
            },
            ..held
        };
        let shown = hotkey_check(&refused);
        assert_eq!(
            outcome(&shown),
            (CheckStatus::Fail, CheckReason::HotkeyRefused)
        );
        assert_eq!(shown.detail.as_deref(), Some("HotKey already registered"));
    }

    /// A quiet check that fails after one that worked is what the self-check reports, though the
    /// window's update line stays as the working one left it.
    #[test]
    fn the_last_check_for_updates_counts_the_quiet_ones() {
        let updater = Updater::default();
        assert_eq!(
            outcome(&updater_check(updater.last_check())),
            (CheckStatus::Unknown, CheckReason::NeverChecked)
        );
        updater.checked(true);
        assert_eq!(
            outcome(&updater_check(updater.last_check())),
            (CheckStatus::Ok, CheckReason::Fine)
        );
        updater.checked(false);
        assert_eq!(
            outcome(&updater_check(updater.last_check())),
            (CheckStatus::Warn, CheckReason::CheckFailed)
        );
    }
}
