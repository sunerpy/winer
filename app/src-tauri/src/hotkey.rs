//! The global shortcut that summons the window while the game has the screen: the one combination
//! the settings name (`general.hotkey`), registered with the system for winer alone. A second one,
//! off by default, sends the callout (`automation.callout.hotkey`, at the end of this file).
//!
//! Registration runs on the main thread, where the shortcut plugin keeps its manager: called from
//! there, the plugin acts at once instead of waiting for the event loop, so nothing here waits on
//! the main thread while it holds a lock.

use std::{
    str::FromStr as _,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcut, Shortcut, ShortcutEvent, ShortcutState};
use tracing::{info, warn};
use winer_core::{
    Service,
    callout::Press,
    settings::Audience,
    view::{CalloutHotkeyStatus, HotkeyStatus, NoticeKind, Phase},
};

use crate::{elevation, events, game_chat, window};

/// The recorder lets the shortcut go while it listens; should the window never say it is done (it
/// was closed, or reloaded), the shortcut comes back after this long.
const SUSPEND_LIMIT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Hotkey {
    state: Mutex<State>,
    /// The shortcut put the window above the game; it stays there until it is hidden or the game
    /// ends. Apart from the lock: the shortcut's own handler reads it.
    pinned: AtomicBool,
    // Callout: the second shortcut.
    /// The id of the combination registered for the callout, 0 for none: the handler tells the two
    /// shortcuts apart by it, without the lock.
    callout: AtomicU32,
    /// A press of the callout's shortcut is being carried out; another one meanwhile does nothing.
    sending: AtomicBool,
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
    // Callout: the second shortcut, let go while suspended like the first.
    callout: CalloutSlot,
}

impl State {
    fn status(&self) -> HotkeyStatus {
        HotkeyStatus {
            shortcut: self.wanted.clone(),
            active: self.registered.is_some(),
            suspended: self.suspended,
            error: self.error.clone(),
            callout: self.callout.status(),
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
        // The callout lets go first and takes its combination last, so the window's shortcut can
        // take one the callout has just given up.
        release_callout(app, &mut state);
        sync(app, &mut state);
        take_callout(app, &mut state, &hotkey.callout);
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

/// The shortcut plugin's handler, for both of winer's combinations: the callout's is told apart by
/// the id registered for it, and every other press is the window's.
pub(crate) fn on_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    shortcut: &Shortcut,
    event: ShortcutEvent,
) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let callout = app
        .try_state::<Hotkey>()
        .map_or(0, |hotkey| hotkey.callout.load(Ordering::Acquire));
    if is_callout(callout, shortcut) {
        call_out(app);
    } else {
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

// ---- The callout's shortcut (`automation.callout.hotkey`) ----

/// The second combination, which sends the callout. It is let go and taken back with the window's,
/// and never holds the window's combination: the settings keep the two apart.
#[derive(Default)]
struct CalloutSlot {
    wanted: Option<String>,
    registered: Option<Shortcut>,
    error: Option<String>,
}

impl CalloutSlot {
    fn status(&self) -> CalloutHotkeyStatus {
        CalloutHotkeyStatus {
            shortcut: self.wanted.clone(),
            active: self.registered.is_some(),
            error: self.error.clone(),
        }
    }

    /// The combination to hold now: the wanted one, unless the recorder has every shortcut let go.
    fn target(&self, suspended: bool) -> Option<Result<Shortcut, String>> {
        self.wanted.as_deref().filter(|_| !suspended).map(parse)
    }
}

/// Holds the combination the settings name for the callout, `None` for none. Returns at once; the
/// outcome reaches the window as a [`HotkeyStatus`] event.
pub(crate) fn apply_callout<R: Runtime>(app: &AppHandle<R>, wanted: Option<String>) {
    drop(update(app, move |state| state.callout.wanted = wanted));
}

/// Lets go of what the callout holds, unless it is still the combination to hold. Main thread only.
fn release_callout<R: Runtime>(app: &AppHandle<R>, state: &mut State) {
    let target = state.callout.target(state.suspended);
    let slot = &mut state.callout;
    let keep =
        matches!((&target, slot.registered), (Some(Ok(wanted)), Some(held)) if *wanted == held);
    if !keep
        && let Some(held) = slot.registered.take()
        && let Some(manager) = app.try_state::<GlobalShortcut<R>>()
        && let Err(error) = manager.unregister(held)
    {
        warn!(%error, "the callout's old shortcut was not released");
    }
}

/// Registers the combination the callout wants, and records its id for the handler. Main thread
/// only.
fn take_callout<R: Runtime>(app: &AppHandle<R>, state: &mut State, id: &AtomicU32) {
    let target = state.callout.target(state.suspended);
    let slot = &mut state.callout;
    slot.error = None;
    match target {
        None => {}
        Some(Err(error)) => slot.error = Some(error),
        Some(Ok(shortcut)) if slot.registered == Some(shortcut) => {}
        Some(Ok(shortcut)) => match app.try_state::<GlobalShortcut<R>>() {
            None => slot.error = Some("the global shortcut plugin is not running".into()),
            Some(manager) => match manager.register(shortcut) {
                Ok(()) => {
                    info!(%shortcut, "callout shortcut registered");
                    slot.registered = Some(shortcut);
                }
                Err(error) => {
                    warn!(%error, %shortcut, "callout shortcut refused");
                    slot.error = Some(error.to_string());
                }
            },
        },
    }
    id.store(
        slot.registered.map_or(0, |shortcut| shortcut.id()),
        Ordering::Release,
    );
}

/// Whether `shortcut` is the callout's, by the id registered for it (0: none is).
fn is_callout(registered: u32, shortcut: &Shortcut) -> bool {
    registered != 0 && shortcut.id() == registered
}

/// A press of the callout's shortcut. What it does is the core's to say (`Service::callout_press`):
/// champ select's lines go to its chat over the client's API, as 发送到队伍 sends them; the game's
/// are typed into the game's chat here, off the main thread. What came of it goes to the activity
/// feed, since the window may be hidden behind the game. One press at a time: another one while a
/// callout is going out does nothing.
fn call_out<R: Runtime>(app: &AppHandle<R>) {
    let (Some(service), Some(hotkey)) = (app.try_state::<Service>(), app.try_state::<Hotkey>())
    else {
        return;
    };
    if hotkey.sending.swap(true, Ordering::AcqRel) {
        return;
    }
    let (service, sending) = (service.inner().clone(), Sending(app.clone()));
    match service.callout_press() {
        Press::ChampSelect => {
            tauri::async_runtime::spawn(async move {
                let _sending = sending;
                service.report(match service.send_callout(Some(Audience::Team)).await {
                    Ok(lines) => NoticeKind::CalledOut { lines },
                    Err(error) => NoticeKind::Failed {
                        action: "callout".into(),
                        message: error.to_string(),
                    },
                });
            });
        }
        Press::Game(lines) => {
            if !elevation::is_elevated() {
                warn!(
                    "winer runs without administrator rights; a game that has them ignores its keys"
                );
            }
            tauri::async_runtime::spawn_blocking(move || {
                let _sending = sending;
                service.report(game_chat::notice(game_chat::type_lines(&lines)));
            });
        }
        Press::Skip(reason) => {
            info!(?reason, "the callout's shortcut sent nothing");
            service.report(NoticeKind::CalloutSkipped { reason });
        }
    }
}

/// A press of the callout's shortcut under way: dropped however the press ends, it lets the next
/// one through.
struct Sending<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Drop for Sending<R> {
    fn drop(&mut self) {
        if let Some(hotkey) = self.0.try_state::<Hotkey>() {
            hotkey.sending.store(false, Ordering::Release);
        }
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
            parse("alt+Backquote"),
            "the default is Alt+`"
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

    // ---- The callout's shortcut ----

    #[test]
    fn a_press_is_the_callouts_only_by_the_id_registered_for_it() {
        let callout = parse("Ctrl+Shift+X").unwrap();
        let window = parse(DEFAULT_HOTKEY).unwrap();
        assert!(is_callout(callout.id(), &callout));
        assert!(
            !is_callout(callout.id(), &window),
            "the window's stays the window's"
        );
        assert!(
            !is_callout(0, &callout),
            "with none registered, every press is the window's"
        );
    }

    #[test]
    fn the_callouts_shortcut_holds_its_combination_unless_the_recorder_lets_go_of_all() {
        let slot = CalloutSlot {
            wanted: Some("Ctrl+Shift+X".into()),
            ..CalloutSlot::default()
        };
        assert_eq!(slot.target(false), Some(parse("Ctrl+Shift+X")));
        assert_eq!(
            slot.target(true),
            None,
            "let go while a combination is recorded"
        );
        assert_eq!(CalloutSlot::default().target(false), None);
        let nonsense = CalloutSlot {
            wanted: Some("Ctrl+Nonsense".into()),
            ..CalloutSlot::default()
        };
        assert!(matches!(nonsense.target(false), Some(Err(_))));
        assert_eq!(
            slot.status(),
            CalloutHotkeyStatus {
                shortcut: Some("Ctrl+Shift+X".into()),
                active: false,
                error: None
            }
        );
    }
}
