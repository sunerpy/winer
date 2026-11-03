//! The global shortcut that summons the window while the game has the screen: the one combination
//! the settings name (`general.hotkey`), registered with the system for winer alone. Four more,
//! each off by default, send something to a chat (at the end of this file): the callout
//! (`automation.callout.hotkey`), a comeback to the team (`comeback.hotkey`) or to everyone
//! (`comeback.all_hotkey`), and the first quick phrase to the teammate asked to trade
//! (`comeback.phrase_hotkey`).
//!
//! Registration runs on the main thread, where the shortcut plugin keeps its manager: called from
//! there, the plugin acts at once instead of waiting for the event loop, so nothing here waits on
//! the main thread while it holds a lock. While a game runs, the same combinations are also heard
//! through a keyboard hook (`game_keys`): the system's shortcuts do not reach winer then.

use std::{
    str::FromStr as _,
    sync::{
        Arc, Mutex, MutexGuard,
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
    comeback::{ComebackPress, PhrasePress},
    settings::Audience,
    view::{CalloutHotkeyStatus, ComebackPlace, HotkeyStatus, NoticeKind, Phase},
};

use crate::{events, game_chat, game_keys, window};

/// The recorder lets the shortcut go while it listens; should the window never say it is done (it
/// was closed, or reloaded), the shortcut comes back after this long.
const SUSPEND_LIMIT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Hotkey {
    state: Mutex<State>,
    /// The shortcut put the window above the game; it stays there until it is hidden or the game
    /// ends. Apart from the lock: the shortcut's own handler reads it.
    pinned: AtomicBool,
    /// The id of the combination each of the other shortcuts holds, in [`Action::ALL`]'s order, 0
    /// for none: the handler tells the shortcuts apart by them, without the lock.
    ids: [AtomicU32; Action::ALL.len()],
    /// A press of the callout's, a comeback's or the quick phrase's shortcut is being carried out;
    /// another press of any of them meanwhile does nothing.
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
    /// The other shortcuts, in [`Action::ALL`]'s order, let go while suspended like the first.
    slots: [Slot; Action::ALL.len()],
}

impl State {
    fn status(&self) -> HotkeyStatus {
        let [callout, comeback, comeback_all, phrase] = &self.slots;
        HotkeyStatus {
            shortcut: self.wanted.clone(),
            active: self.registered.is_some(),
            suspended: self.suspended,
            error: self.error.clone(),
            callout: callout.status(),
            comeback: comeback.status(),
            comeback_all: comeback_all.status(),
            phrase: phrase.status(),
        }
    }

    /// Every combination held now, as the game's keyboard hook hears it, tagged as
    /// [`action_of_tag`] reads it back: the window's 0, the others their place in [`Action::ALL`]
    /// plus one.
    fn combos(&self) -> Vec<(game_keys::Combo, u8)> {
        std::iter::once((self.registered, 0))
            .chain(
                Action::ALL
                    .into_iter()
                    .map(|action| (self.slots[action as usize].registered, action as u8 + 1)),
            )
            .filter_map(|(shortcut, tag)| Some((game_keys::Combo::of(&shortcut?)?, tag)))
            .collect()
    }

    fn slot(&mut self, action: Action) -> &mut Slot {
        &mut self.slots[action as usize]
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
        // The other shortcuts let go first and take their combinations last, so the window's
        // shortcut, and each of theirs, can take one that another has just given up.
        for action in Action::ALL {
            release(app, &mut state, action);
        }
        sync(app, &mut state);
        for action in Action::ALL {
            take(app, &mut state, action, &hotkey.ids[action as usize]);
        }
        let in_game = app
            .try_state::<Service>()
            .is_some_and(|service| service.phase().in_game());
        listen_in_game(app, &state, in_game);
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

/// The shortcut plugin's handler, for every one of winer's combinations: the callout's, the
/// comebacks' and the quick phrase's are told apart by the ids registered for them, and every other
/// press is the window's.
pub(crate) fn on_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    shortcut: &Shortcut,
    event: ShortcutEvent,
) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let ids = app
        .try_state::<Hotkey>()
        .map(|hotkey| hotkey.ids.each_ref().map(|id| id.load(Ordering::Acquire)))
        .unwrap_or_default();
    act(app, action_of(&ids, shortcut));
}

/// What a press of `action`'s combination does; `None` is the window's. Main thread only.
fn act<R: Runtime>(app: &AppHandle<R>, action: Option<Action>) {
    match action {
        Some(Action::Callout) => call_out(app),
        Some(Action::Comeback) => come_back(app, false),
        Some(Action::ComebackAll) => come_back(app, true),
        Some(Action::Phrase) => send_phrase(app),
        None => toggle(app),
    }
}

/// While a game runs (`in_game`), hears every combination held through the game's keyboard hook
/// (`game_keys`), each press carried out on the main thread as the plugin's are; otherwise the hook
/// is taken out. With the state locked, so one change at a time.
fn listen_in_game<R: Runtime>(app: &AppHandle<R>, state: &State, in_game: bool) {
    let combos = state.combos();
    game_keys::listen((in_game && !combos.is_empty()).then(|| {
        let app = app.clone();
        let press: game_keys::Press = Arc::new(move |tag| {
            on_main(&app, move |app| {
                let action = action_of_tag(tag);
                info!(
                    shortcut = action.map_or("window", Action::name),
                    "a shortcut pressed in the game"
                );
                act(app, action);
            });
        });
        (combos, press)
    }));
}

/// The action a tag of [`State::combos`] names; `None` is the window's.
fn action_of_tag(tag: u8) -> Option<Action> {
    let index = usize::from(tag.checked_sub(1)?);
    Action::ALL.get(index).copied()
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

/// A game started: its keyboard hook hears the shortcuts. The game is over: the hook goes, and a
/// window pinned above the game comes down to the others.
pub(crate) fn on_phase<R: Runtime>(app: &AppHandle<R>, phase: Phase) {
    if let Some(hotkey) = app.try_state::<Hotkey>() {
        listen_in_game(app, &lock(&hotkey.state), phase.in_game());
    }
    if !phase.in_game() {
        unpin(app);
    }
}

// ---- The shortcuts that send to a chat: the callout's, the comebacks' and the quick phrase's ----

/// What a press of one of the other shortcuts does. Each holds a combination of its own, let go and
/// taken back with the window's, and never one another holds: the settings keep them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    /// Sends the callout (`automation.callout.hotkey`).
    Callout,
    /// Sends a comeback to the team: champ select's chat, or the game's team chat
    /// (`comeback.hotkey`).
    Comeback,
    /// Sends a comeback to everyone in the game; in champ select, to its chat
    /// (`comeback.all_hotkey`).
    ComebackAll,
    /// Sends the first quick phrase, in champ select, to the teammate the user has asked to trade
    /// champions with (`comeback.phrase_hotkey`).
    Phrase,
}

impl Action {
    /// Every one, in the order of the slots and their ids.
    const ALL: [Self; 4] = [
        Self::Callout,
        Self::Comeback,
        Self::ComebackAll,
        Self::Phrase,
    ];

    /// What the log calls its shortcut.
    fn name(self) -> &'static str {
        match self {
            Self::Callout => "callout",
            Self::Comeback => "comeback",
            Self::ComebackAll => "all-chat comeback",
            Self::Phrase => "quick phrase",
        }
    }
}

/// The combination one of the other shortcuts holds.
#[derive(Default)]
struct Slot {
    wanted: Option<String>,
    registered: Option<Shortcut>,
    error: Option<String>,
}

impl Slot {
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
    drop(update(app, move |state| {
        state.slot(Action::Callout).wanted = wanted;
    }));
}

/// Holds the combinations the settings name for the Comebacks page's shortcuts, to the team
/// (`team`), to everyone (`all`) and the quick phrase's (`phrase`), `None` for none. Returns at
/// once; the outcome reaches the window as a [`HotkeyStatus`] event.
pub(crate) fn apply_comebacks<R: Runtime>(
    app: &AppHandle<R>,
    team: Option<String>,
    all: Option<String>,
    phrase: Option<String>,
) {
    drop(update(app, move |state| {
        state.slot(Action::Comeback).wanted = team;
        state.slot(Action::ComebackAll).wanted = all;
        state.slot(Action::Phrase).wanted = phrase;
    }));
}

/// Lets go of what `action`'s slot holds, unless it is still the combination to hold. Main thread
/// only.
fn release<R: Runtime>(app: &AppHandle<R>, state: &mut State, action: Action) {
    let suspended = state.suspended;
    let slot = state.slot(action);
    let target = slot.target(suspended);
    let keep =
        matches!((&target, slot.registered), (Some(Ok(wanted)), Some(held)) if *wanted == held);
    if !keep
        && let Some(held) = slot.registered.take()
        && let Some(manager) = app.try_state::<GlobalShortcut<R>>()
        && let Err(error) = manager.unregister(held)
    {
        warn!(%error, "the {}'s old shortcut was not released", action.name());
    }
}

/// Registers the combination `action`'s slot wants, and records its id for the handler. Main
/// thread only.
fn take<R: Runtime>(app: &AppHandle<R>, state: &mut State, action: Action, id: &AtomicU32) {
    let suspended = state.suspended;
    let slot = state.slot(action);
    let target = slot.target(suspended);
    slot.error = None;
    match target {
        None => {}
        Some(Err(error)) => slot.error = Some(error),
        Some(Ok(shortcut)) if slot.registered == Some(shortcut) => {}
        Some(Ok(shortcut)) => match app.try_state::<GlobalShortcut<R>>() {
            None => slot.error = Some("the global shortcut plugin is not running".into()),
            Some(manager) => match manager.register(shortcut) {
                Ok(()) => {
                    info!(%shortcut, "{} shortcut registered", action.name());
                    slot.registered = Some(shortcut);
                }
                Err(error) => {
                    warn!(%error, %shortcut, "{} shortcut refused", action.name());
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

/// Which of the other shortcuts `shortcut` is, by the ids registered for them (`ids`, in
/// [`Action::ALL`]'s order; 0: that one holds none). `None`: it is the window's.
fn action_of(ids: &[u32; Action::ALL.len()], shortcut: &Shortcut) -> Option<Action> {
    Action::ALL
        .into_iter()
        .zip(ids)
        .find(|&(_, &id)| id != 0 && id == shortcut.id())
        .map(|(action, _)| action)
}

/// A press of the callout's shortcut. What it does is the core's to say (`Service::callout_press`):
/// champ select's lines go to its chat over the client's API, as 发送到队伍 sends them; the game's
/// are typed into the game's chat here, off the main thread. What came of it goes to the activity
/// feed, since the window may be hidden behind the game. One press at a time, of this shortcut or
/// a comeback's: another one while something is going out does nothing.
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
        // The game's rights are checked as the keys are about to go (`game_chat`).
        Press::Game(lines) => {
            tauri::async_runtime::spawn_blocking(move || {
                let _sending = sending;
                service.report(game_chat::notice(game_chat::type_lines(&lines, false)));
            });
        }
        Press::Skip(reason) => {
            info!(?reason, "the callout's shortcut sent nothing");
            service.report(NoticeKind::CalloutSkipped { reason });
        }
    }
}

/// A press of a comeback's shortcut, to the team or, with `all`, to everyone. Which line goes, and
/// where, is the core's to say (`Service::comeback_press`): in champ select it is posted in its
/// chat over the client's API; while the game runs it is typed into the game's chat here, with
/// `/all ` for everyone. What came of it goes to the activity feed, never the line itself (the log
/// keeps no line either). One press at a time, as for the callout.
fn come_back<R: Runtime>(app: &AppHandle<R>, all: bool) {
    let (Some(service), Some(hotkey)) = (app.try_state::<Service>(), app.try_state::<Hotkey>())
    else {
        return;
    };
    if hotkey.sending.swap(true, Ordering::AcqRel) {
        return;
    }
    let (service, sending) = (service.inner().clone(), Sending(app.clone()));
    // Off the main thread from the start: the first press with the library in use reads its file.
    tauri::async_runtime::spawn_blocking(move || match service.comeback_press(all) {
        ComebackPress::ChampSelect { line, game_id } => {
            tauri::async_runtime::spawn(async move {
                let _sending = sending;
                service.report(match service.send_comeback(line, game_id).await {
                    Ok(messages) => NoticeKind::ComebackSent {
                        place: ComebackPlace::ChampSelect,
                        messages,
                    },
                    Err(error) => NoticeKind::Failed {
                        action: "comeback".into(),
                        message: error.to_string(),
                    },
                });
            });
        }
        // The game's rights are checked as the keys are about to go (`game_chat`). A line that
        // reached the chat, whole or in part, is the one the next press neither repeats nor
        // follows within the spacing; one that sent nothing changes neither.
        ComebackPress::Game { line, all } => {
            let _sending = sending;
            let typed = game_chat::type_lines(std::slice::from_ref(&line), all);
            if game_chat::sent(typed) > 0 {
                service.comeback_went_out(&line);
            }
            service.report(game_chat::comeback_notice(typed, all));
        }
        ComebackPress::Skip { place, reason } => {
            info!(?place, ?reason, "a comeback's shortcut sent nothing");
            service.report(NoticeKind::ComebackSkipped { place, reason });
        }
    });
}

/// A press of quick phrase's shortcut. What it does is the core's to say (`Service::phrase_press`):
/// in champ select, the first quick phrase goes to the teammate the user has asked to trade with,
/// posted in its chat as the board's menu posts it (`Service::send_phrase`). What came of it goes
/// to the activity feed. One press at a time, as for the others.
fn send_phrase<R: Runtime>(app: &AppHandle<R>) {
    let (Some(service), Some(hotkey)) = (app.try_state::<Service>(), app.try_state::<Hotkey>())
    else {
        return;
    };
    if hotkey.sending.swap(true, Ordering::AcqRel) {
        return;
    }
    let (service, sending) = (service.inner().clone(), Sending(app.clone()));
    match service.phrase_press() {
        PhrasePress::Send { game_id, cell_id } => {
            tauri::async_runtime::spawn(async move {
                let _sending = sending;
                service.report(match service.send_phrase(0, game_id, cell_id).await {
                    Ok(_) => {
                        service.phrase_went_out();
                        NoticeKind::PhraseSent
                    }
                    Err(error) => NoticeKind::Failed {
                        action: "phrase".into(),
                        message: error.to_string(),
                    },
                });
            });
        }
        PhrasePress::Skip(reason) => {
            info!(?reason, "the quick phrase's shortcut sent nothing");
            service.report(NoticeKind::PhraseSkipped { reason });
        }
    }
}

/// A press of the callout's, a comeback's or the quick phrase's shortcut under way: dropped however
/// the press ends, it lets the next one through.
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

    // ---- The shortcuts that send to a chat ----

    #[test]
    fn each_press_is_told_apart_by_the_id_registered_for_its_combination() {
        let [window, callout, team, all, phrase] =
            [DEFAULT_HOTKEY, "Ctrl+Shift+X", "Alt+3", "Alt+4", "Alt+6"].map(|combination| {
                parse(combination).unwrap_or_else(|error| panic!("{combination}: {error}"))
            });
        let distinct: std::collections::HashSet<u32> = [&window, &callout, &team, &all, &phrase]
            .iter()
            .map(|shortcut| shortcut.id())
            .collect();
        assert_eq!(distinct.len(), 5, "five combinations, five ids");
        let ids = [callout.id(), team.id(), all.id(), phrase.id()];
        assert_eq!(action_of(&ids, &callout), Some(Action::Callout));
        assert_eq!(action_of(&ids, &team), Some(Action::Comeback));
        assert_eq!(action_of(&ids, &all), Some(Action::ComebackAll));
        assert_eq!(action_of(&ids, &phrase), Some(Action::Phrase));
        assert_eq!(
            action_of(&ids, &window),
            None,
            "the window's stays the window's"
        );
        assert_eq!(
            action_of(&[0; 4], &callout),
            None,
            "with none registered, every press is the window's"
        );
        assert_eq!(
            action_of(&[callout.id(), 0, all.id(), 0], &team),
            None,
            "a slot that holds nothing takes no press"
        );
    }

    #[test]
    fn the_game_hears_every_combination_held_and_tells_them_apart_by_their_tags() {
        let mut state = State::default();
        assert!(state.combos().is_empty(), "nothing held, nothing heard");
        state.registered = Some(parse(DEFAULT_HOTKEY).unwrap());
        state.slot(Action::Comeback).registered = Some(parse("Alt+5").unwrap());
        state.slot(Action::Phrase).registered = Some(parse("Alt+6").unwrap());
        // Wanted but refused: the system's shortcut does not hold it, so neither does the game's.
        state.slot(Action::Callout).wanted = Some("Alt+3".into());
        let combos = state.combos();
        let tags: Vec<(u16, u8)> = combos
            .iter()
            .map(|(combo, tag)| (combo.key, *tag))
            .collect();
        assert_eq!(tags, [(0xC0, 0), (0x35, 2), (0x36, 4)]);
        assert_eq!(action_of_tag(0), None, "the window's");
        for (tag, action) in (1..).zip(Action::ALL) {
            assert_eq!(action_of_tag(tag), Some(action));
        }
        assert_eq!(action_of_tag(5), None);
    }

    #[test]
    fn a_shortcut_holds_its_combination_unless_the_recorder_lets_go_of_all() {
        let slot = Slot {
            wanted: Some("Ctrl+Shift+X".into()),
            ..Slot::default()
        };
        assert_eq!(slot.target(false), Some(parse("Ctrl+Shift+X")));
        assert_eq!(
            slot.target(true),
            None,
            "let go while a combination is recorded"
        );
        assert_eq!(Slot::default().target(false), None);
        let nonsense = Slot {
            wanted: Some("Ctrl+Nonsense".into()),
            ..Slot::default()
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

    #[test]
    fn the_status_reports_each_shortcut_under_its_own_name() {
        let mut state = State::default();
        state.slot(Action::Callout).wanted = Some("Ctrl+Shift+X".into());
        state.slot(Action::Comeback).registered = Some(parse("Alt+3").unwrap());
        state.slot(Action::Comeback).wanted = Some("Alt+3".into());
        state.slot(Action::ComebackAll).error = Some("HotKey already registered".into());
        state.slot(Action::Phrase).wanted = Some("Alt+6".into());
        let status = state.status();
        assert_eq!(status.callout.shortcut.as_deref(), Some("Ctrl+Shift+X"));
        assert!(!status.callout.active);
        assert_eq!(
            status.comeback,
            CalloutHotkeyStatus {
                shortcut: Some("Alt+3".into()),
                active: true,
                error: None
            }
        );
        assert_eq!(
            status.comeback_all.error.as_deref(),
            Some("HotKey already registered")
        );
        assert_eq!(status.phrase.shortcut.as_deref(), Some("Alt+6"));
        assert!(!status.phrase.active);
        assert_eq!(status.shortcut, None, "the window's own is apart");
    }
}
