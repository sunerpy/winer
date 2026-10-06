//! The global shortcut that summons the window while the game has the screen: the one combination
//! the settings name (`general.hotkey`), registered with the system for winer alone.
//!
//! Registration runs on the main thread, where the shortcut plugin keeps its manager: called from
//! there, the plugin acts at once instead of waiting for the event loop, so nothing here waits on
//! the main thread while it holds a lock.

use std::{
    str::FromStr as _,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcut, Shortcut, ShortcutEvent, ShortcutState};
use tracing::{info, warn};
use winer_core::{
    Service,
    view::{HotkeyStatus, Phase},
};

use crate::{events, window};

/// The recorder lets the shortcut go while it listens; should the window never say it is done (it
/// was closed, or reloaded), the shortcut comes back after this long.
const SUSPEND_LIMIT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Hotkey {
    state: Mutex<State>,
    /// The shortcut put the window above the game; it stays there until it is hidden or the game
    /// ends. Apart from the lock: the shortcut's own handler reads it.
    pinned: AtomicBool,
}

#[derive(Default)]
struct State {
    /// The combination the settings ask for.
    wanted: Option<String>,
    /// What the system holds for winer now.
    registered: Option<Shortcut>,
    suspended: bool,
    /// Counts suspensions, so a late safety timer cannot end a newer one.
    suspension: u64,
    error: Option<String>,
}

impl State {
    fn status(&self) -> HotkeyStatus {
        HotkeyStatus {
            shortcut: self.wanted.clone(),
            active: self.registered.is_some(),
            suspended: self.suspended,
            error: self.error.clone(),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A combination in the settings' spelling as the shortcut plugin reads it. Every spelling
/// `normalize_hotkey` writes parses (a test holds the two together).
fn parse(combination: &str) -> Result<Shortcut, String> {
    Shortcut::from_str(combination).map_err(|error| error.to_string())
}

/// Brings what the system holds in line with `state`: the wanted combination, or nothing while
/// suspended. Main thread only.
fn sync<R: Runtime>(app: &AppHandle<R>, state: &mut State) {
    let Some(manager) = app.try_state::<GlobalShortcut<R>>() else {
        state.error = Some("the global shortcut plugin is not running".into());
        return;
    };
    let target = match (&state.wanted, state.suspended) {
        (Some(wanted), false) => Some(parse(wanted)),
        _ => None,
    };
    let keep =
        matches!((&target, state.registered), (Some(Ok(wanted)), Some(held)) if *wanted == held);
    if !keep
        && let Some(held) = state.registered.take()
        && let Err(error) = manager.unregister(held)
    {
        warn!(%error, "the old shortcut was not released");
    }
    state.error = None;
    match target {
        None => {}
        Some(Err(error)) => state.error = Some(error),
        Some(Ok(_)) if keep => {}
        Some(Ok(shortcut)) => match manager.register(shortcut) {
            Ok(()) => {
                info!(%shortcut, "global shortcut registered");
                state.registered = Some(shortcut);
            }
            // Most often another program already holds the combination.
            Err(error) => {
                warn!(%error, %shortcut, "global shortcut refused");
                state.error = Some(error.to_string());
            }
        },
    }
}

/// Runs `work` on the main thread: at once when called there, otherwise queued without waiting.
fn on_main<R: Runtime>(app: &AppHandle<R>, work: impl FnOnce(&AppHandle<R>) + Send + 'static) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || work(&handle)) {
        warn!(%error, "the main thread did not take the shortcut's work");
    }
}

/// Changes the state on the main thread, re-syncs and announces the outcome when it moved.
fn update<R: Runtime>(
    app: &AppHandle<R>,
    change: impl FnOnce(&mut State) + Send + 'static,
) -> tokio::sync::oneshot::Receiver<HotkeyStatus> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    on_main(app, move |app| {
        let Some(hotkey) = app.try_state::<Hotkey>() else {
            return;
        };
        let mut state = lock(&hotkey.state);
        let before = state.status();
        change(&mut state);
        sync(app, &mut state);
        let status = state.status();
        drop(state);
        if status != before {
            let _ = app.emit(events::HOTKEY, &status);
        }
        let _ = sender.send(status);
    });
    receiver
}

/// Holds the combination the settings name, `None` for none. Returns at once; the outcome reaches
/// the window as a [`HotkeyStatus`] event.
pub(crate) fn apply<R: Runtime>(app: &AppHandle<R>, wanted: Option<String>) {
    drop(update(app, move |state| state.wanted = wanted));
}

pub(crate) fn status<R: Runtime>(app: &AppHandle<R>) -> HotkeyStatus {
    app.try_state::<Hotkey>()
        .map(|hotkey| lock(&hotkey.state).status())
        .unwrap_or_default()
}

/// Lets the shortcut go while the settings record a new one, and takes it back after.
pub(crate) async fn suspend<R: Runtime>(app: &AppHandle<R>, suspended: bool) -> HotkeyStatus {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let changed = update(app, move |state| {
        if suspended {
            state.suspension += 1;
        }
        state.suspended = suspended;
        let _ = sender.send(state.suspension);
    });
    if suspended && let Ok(suspension) = receiver.await {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SUSPEND_LIMIT).await;
            drop(update(&app, move |state| {
                if state.suspended && state.suspension == suspension {
                    state.suspended = false;
                }
            }));
        });
    }
    match changed.await {
        Ok(status) => status,
        Err(_) => status(app),
    }
}

/// What a press does with the window as it is now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toggle {
    Hide,
    /// Up, focused, and above everything when `pin` (a game is running).
    Show {
        pin: bool,
    },
}

/// A window in front (shown, not minimized, focused or pinned above the game) goes away; any
/// other comes up, above the game while one runs.
pub(crate) fn decide(
    visible: bool,
    minimized: bool,
    focused: bool,
    pinned: bool,
    in_game: bool,
) -> Toggle {
    if visible && !minimized && (focused || pinned) {
        Toggle::Hide
    } else {
        Toggle::Show { pin: in_game }
    }
}

/// The shortcut plugin's handler: only winer's one combination is registered, so every press is it.
pub(crate) fn on_shortcut<R: Runtime>(app: &AppHandle<R>, _: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        toggle(app);
    }
}

fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let Some(main) = app.get_webview_window(window::MAIN) else {
        return;
    };
    let pinned = app
        .try_state::<Hotkey>()
        .is_some_and(|hotkey| hotkey.pinned.load(Ordering::Relaxed));
    let in_game = app
        .try_state::<Service>()
        .is_some_and(|service| service.phase().in_game());
    match decide(
        main.is_visible().unwrap_or(false),
        main.is_minimized().unwrap_or(false),
        main.is_focused().unwrap_or(false),
        pinned,
        in_game,
    ) {
        Toggle::Hide => {
            unpin(app);
            let _ = main.hide();
        }
        Toggle::Show { pin } => {
            window::show(app);
            if pin {
                let _ = main.set_always_on_top(true);
                if let Some(hotkey) = app.try_state::<Hotkey>() {
                    hotkey.pinned.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Takes the window off the top, if the shortcut put it there.
pub(crate) fn unpin<R: Runtime>(app: &AppHandle<R>) {
    let Some(hotkey) = app.try_state::<Hotkey>() else {
        return;
    };
    if hotkey.pinned.swap(false, Ordering::Relaxed)
        && let Some(main) = app.get_webview_window(window::MAIN)
    {
        let _ = main.set_always_on_top(false);
    }
}

/// The game is over: a window pinned above it comes down to the others.
pub(crate) fn on_phase<R: Runtime>(app: &AppHandle<R>, phase: Phase) {
    if !phase.in_game() {
        unpin(app);
    }
}

#[cfg(test)]
mod tests {
    use winer_core::settings::{DEFAULT_HOTKEY, HOTKEY_KEYS, normalize_hotkey};

    use super::*;

    #[test]
    fn every_combination_the_settings_write_is_one_the_plugin_reads() {
        let letters = ('A'..='Z').map(String::from);
        let digits = ('0'..='9').map(String::from);
        let function = (1..=24).map(|n| format!("F{n}"));
        let named = HOTKEY_KEYS.iter().map(|key| (*key).to_owned());
        for key in letters.chain(digits).chain(function).chain(named) {
            let combination = normalize_hotkey(&format!("Ctrl+Alt+Shift+Win+{key}"))
                .unwrap_or_else(|| panic!("{key} is a key the settings take"));
            assert!(parse(&combination).is_ok(), "{combination} parses");
        }
        assert_eq!(
            parse(DEFAULT_HOTKEY),
            parse("control+shift+KeyW"),
            "the default is Ctrl+Shift+W"
        );
        assert!(parse("Ctrl+Nonsense").is_err());
    }

    #[test]
    fn a_press_hides_a_window_in_front_and_brings_up_any_other() {
        // visible, minimized, focused, pinned, in game
        assert_eq!(decide(true, false, true, false, false), Toggle::Hide);
        assert_eq!(
            decide(true, false, false, true, true),
            Toggle::Hide,
            "pinned above the game counts as in front, even with the game focused"
        );
        assert_eq!(
            decide(true, false, false, false, true),
            Toggle::Show { pin: true },
            "behind the game: brought up and pinned"
        );
        assert_eq!(
            decide(false, false, false, false, false),
            Toggle::Show { pin: false }
        );
        assert_eq!(
            decide(true, true, true, false, true),
            Toggle::Show { pin: true },
            "a minimized window comes back"
        );
    }
}
