//! winer's shortcuts while the game has the screen. The system's shortcuts (`RegisterHotKey`, which
//! `hotkey` registers through the shortcut plugin) never reached winer in a game on the owner's PC
//! (2026-11-03: no press logged in three games, every press in champ select and the lobby was).
//! So while a game runs, a low-level keyboard hook (`WH_KEYBOARD_LL`, as ZuAnBot listens) watches
//! for the same combinations. Only with the game's window in front, and only an exact combination:
//! that press goes to winer instead of the game, as the system's shortcut would have taken it, and
//! every other key passes untouched. Nothing about a key is kept or logged.
//!
//! Which key a combination is, and what the hook does with each key event, is decided here without
//! touching the system and tested on every platform; `imp` installs the hook on Windows and does
//! nothing elsewhere.

// Outside Windows nothing installs the hook, so only the tests use what decides for it.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::Arc;

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// What a press is handed to: the tag its combination was given (`listen`).
pub(crate) type Press = Arc<dyn Fn(u8) + Send + Sync>;

/// The modifiers held with a key, either side of each, as `RegisterHotKey` tells them apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Mods {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) win: bool,
}

/// One combination as the keyboard reports it: its key's virtual-key code, and its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Combo {
    pub(crate) key: u16,
    pub(crate) mods: Mods,
}

impl Combo {
    /// `shortcut` as the hook sees it, with the virtual key the shortcut plugin registers it under;
    /// `None` for a key the settings never write.
    pub(crate) fn of(shortcut: &Shortcut) -> Option<Self> {
        Some(Self {
            key: virtual_key(shortcut.key)?,
            mods: Mods {
                ctrl: shortcut.mods.contains(Modifiers::CONTROL),
                alt: shortcut.mods.contains(Modifiers::ALT),
                shift: shortcut.mods.contains(Modifiers::SHIFT),
                win: shortcut.mods.intersects(Modifiers::SUPER | Modifiers::META),
            },
        })
    }
}

/// The virtual-key code of every key a combination in the settings may end in
/// (`settings::normalize_hotkey`), the same the shortcut plugin passes to `RegisterHotKey`.
fn virtual_key(code: Code) -> Option<u16> {
    let letters = [
        Code::KeyA,
        Code::KeyB,
        Code::KeyC,
        Code::KeyD,
        Code::KeyE,
        Code::KeyF,
        Code::KeyG,
        Code::KeyH,
        Code::KeyI,
        Code::KeyJ,
        Code::KeyK,
        Code::KeyL,
        Code::KeyM,
        Code::KeyN,
        Code::KeyO,
        Code::KeyP,
        Code::KeyQ,
        Code::KeyR,
        Code::KeyS,
        Code::KeyT,
        Code::KeyU,
        Code::KeyV,
        Code::KeyW,
        Code::KeyX,
        Code::KeyY,
        Code::KeyZ,
    ];
    let digits = [
        Code::Digit0,
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit5,
        Code::Digit6,
        Code::Digit7,
        Code::Digit8,
        Code::Digit9,
    ];
    let function = [
        Code::F1,
        Code::F2,
        Code::F3,
        Code::F4,
        Code::F5,
        Code::F6,
        Code::F7,
        Code::F8,
        Code::F9,
        Code::F10,
        Code::F11,
        Code::F12,
        Code::F13,
        Code::F14,
        Code::F15,
        Code::F16,
        Code::F17,
        Code::F18,
        Code::F19,
        Code::F20,
        Code::F21,
        Code::F22,
        Code::F23,
        Code::F24,
    ];
    let numpad = [
        Code::Numpad0,
        Code::Numpad1,
        Code::Numpad2,
        Code::Numpad3,
        Code::Numpad4,
        Code::Numpad5,
        Code::Numpad6,
        Code::Numpad7,
        Code::Numpad8,
        Code::Numpad9,
    ];
    // VK_A..VK_Z, VK_0..VK_9, VK_F1..VK_F24 and VK_NUMPAD0..VK_NUMPAD9 each run in order.
    let in_run = |run: &[Code], first: u16| {
        run.iter()
            .position(|&key| key == code)
            .map(|index| first + index as u16)
    };
    if let Some(key) = in_run(&letters, 0x41)
        .or_else(|| in_run(&digits, 0x30))
        .or_else(|| in_run(&function, 0x70))
        .or_else(|| in_run(&numpad, 0x60))
    {
        return Some(key);
    }
    Some(match code {
        Code::Space => 0x20,
        Code::PageUp => 0x21,
        Code::PageDown => 0x22,
        Code::End => 0x23,
        Code::Home => 0x24,
        Code::ArrowLeft => 0x25,
        Code::ArrowUp => 0x26,
        Code::ArrowRight => 0x27,
        Code::ArrowDown => 0x28,
        Code::PrintScreen => 0x2C,
        Code::Insert => 0x2D,
        Code::Delete => 0x2E,
        Code::Pause => 0x13,
        Code::ScrollLock => 0x91,
        Code::NumpadMultiply => 0x6A,
        Code::NumpadAdd => 0x6B,
        Code::NumpadSubtract => 0x6D,
        Code::NumpadDecimal => 0x6E,
        Code::NumpadDivide => 0x6F,
        Code::Semicolon => 0xBA,
        Code::Equal => 0xBB,
        Code::Comma => 0xBC,
        Code::Minus => 0xBD,
        Code::Period => 0xBE,
        Code::Slash => 0xBF,
        Code::Backquote => 0xC0,
        Code::BracketLeft => 0xDB,
        Code::Backslash => 0xDC,
        Code::BracketRight => 0xDD,
        Code::Quote => 0xDE,
        _ => return None,
    })
}

/// Shift, Ctrl, Alt and the Windows keys, as one or as either side: never the key a combination
/// ends in.
fn is_modifier(key: u16) -> bool {
    matches!(key, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// What the hook does with one key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Not winer's: on to the game, untouched.
    Pass,
    /// A combination went down with the game in front: the press of the one tagged so. The game
    /// never sees the key.
    Press(u8),
    /// The rest of a press already taken, its key's repeats and its release: kept from the game too.
    Swallow,
}

/// The combinations listened for, and the key of the press taken last.
#[derive(Debug, Default)]
pub(crate) struct Matcher {
    combos: Vec<(Combo, u8)>,
    /// Until it comes up, or another key goes down.
    held: Option<u16>,
}

impl Matcher {
    pub(crate) const fn new() -> Self {
        Self {
            combos: Vec::new(),
            held: None,
        }
    }

    /// Listens for `combos` from now on, each with its tag.
    pub(crate) fn set(&mut self, combos: Vec<(Combo, u8)>) {
        self.combos = combos;
    }

    /// `key` went down (`down`) or up, typed by the user, with `mods` held. `in_front` says whether
    /// the game's window is the foreground window; it is asked only when a combination matches.
    pub(crate) fn key(
        &mut self,
        key: u16,
        down: bool,
        mods: Mods,
        in_front: impl FnOnce() -> bool,
    ) -> Verdict {
        if !down {
            if self.held == Some(key) {
                self.held = None;
                return Verdict::Swallow;
            }
            return Verdict::Pass;
        }
        if self.held == Some(key) {
            // Held down: the keyboard repeats it, and the press was taken once.
            return Verdict::Swallow;
        }
        if is_modifier(key) {
            return Verdict::Pass;
        }
        self.held = None;
        match self
            .combos
            .iter()
            .find(|(combo, _)| combo.key == key && combo.mods == mods)
        {
            Some(&(_, tag)) if in_front() => {
                self.held = Some(key);
                Verdict::Press(tag)
            }
            _ => Verdict::Pass,
        }
    }
}

/// Listens for `combos` while the game runs, each press handed to `press` with its combination's
/// tag; `None`: listens for nothing, and the hook is taken out. Called with the shortcuts' state
/// locked, so one call at a time.
pub(crate) fn listen(wanted: Option<(Vec<(Combo, u8)>, Press)>) {
    imp::listen(wanted);
}

#[cfg(windows)]
mod imp {
    #![allow(unsafe_code)]

    use std::{
        io,
        ptr::{null, null_mut},
        sync::{Mutex, MutexGuard},
        thread,
    };

    use tracing::{info, warn};
    use windows_sys::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
            },
            WindowsAndMessaging::{
                CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
                PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
                UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN, WM_USER,
            },
        },
    };

    use super::{Combo, Matcher, Mods, Press, Verdict};
    use crate::game_chat;

    struct Hook {
        matcher: Matcher,
        press: Option<Press>,
        /// The hook's thread: `Some(0)` while it starts, its id once it runs.
        thread: Option<u32>,
    }

    /// Shared with the hook procedure, which has no other way to reach it.
    static HOOK: Mutex<Hook> = Mutex::new(Hook {
        matcher: Matcher::new(),
        press: None,
        thread: None,
    });

    fn lock() -> MutexGuard<'static, Hook> {
        HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn listen(wanted: Option<(Vec<(Combo, u8)>, Press)>) {
        let mut hook = lock();
        let Some((combos, press)) = wanted else {
            hook.matcher.set(Vec::new());
            // A thread still starting finds no `press` and installs nothing.
            hook.press = None;
            if let Some(thread) = hook.thread.take()
                && thread != 0
            {
                // SAFETY: a plain post; one to a thread that has already ended fails harmlessly.
                unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
            }
            return;
        };
        hook.matcher.set(combos);
        hook.press = Some(press);
        if hook.thread.is_some() {
            return;
        }
        hook.thread = Some(0);
        drop(hook);
        if let Err(error) = thread::Builder::new()
            .name("winer-game-keys".into())
            .spawn(run)
        {
            warn!(%error, "the game's keyboard listener did not start");
            lock().thread = None;
        }
    }

    /// The hook's thread: installs the hook, then serves it until told to quit.
    fn run() {
        // SAFETY: plain Win32 calls; `message` is written by the calls that take it, and the hook
        // procedure is a `HOOKPROC` that stays valid for the program's life.
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            // The thread's message queue, made before its id goes out, so a quit posted at once
            // is not lost.
            PeekMessageW(&mut message, null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
            let me = GetCurrentThreadId();
            {
                let mut hook = lock();
                // Taken out, or started again, while this thread started.
                if hook.press.is_none() || hook.thread != Some(0) {
                    return;
                }
                hook.thread = Some(me);
            }
            let installed =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), GetModuleHandleW(null()), 0);
            if installed.is_null() {
                let error = io::Error::last_os_error();
                warn!(%error, "the game's keyboard hook was refused");
            } else {
                info!("listening for winer's shortcuts in the game");
                // The hook procedure runs inside this wait; nothing but the quit is posted here.
                while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {}
                UnhookWindowsHookEx(installed);
                info!("stopped listening for winer's shortcuts in the game");
            }
            let mut hook = lock();
            if hook.thread == Some(me) {
                hook.thread = None;
            }
        }
    }

    /// The hook procedure: as short as can be, since the system drops a hook that keeps the
    /// keyboard waiting (`LowLevelHooksTimeout`). A press goes on to `press`, which only queues it.
    unsafe extern "system" fn on_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: for HC_ACTION, `lparam` points at the event's KBDLLHOOKSTRUCT.
            let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            // winer's own typing (`game_chat`) comes through here too, injected.
            if event.flags & LLKHF_INJECTED == 0 {
                let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                let (verdict, press) = {
                    let mut hook = lock();
                    let verdict = hook.matcher.key(
                        event.vkCode as u16,
                        down,
                        held(),
                        game_chat::game_in_front,
                    );
                    (verdict, hook.press.clone())
                };
                match verdict {
                    Verdict::Pass => {}
                    Verdict::Swallow => return 1,
                    Verdict::Press(tag) => {
                        if let Some(press) = press {
                            press(tag);
                        }
                        return 1;
                    }
                }
            }
        }
        // SAFETY: the event as it came, on to the next hook.
        unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
    }

    /// The modifiers down now. The key being reported is not counted in the keys' state yet, but
    /// every modifier pressed before it is.
    fn held() -> Mods {
        // SAFETY: any virtual key may be asked about; the top bit says it is down.
        let down = |key: u16| unsafe { GetAsyncKeyState(i32::from(key)) } < 0;
        Mods {
            ctrl: down(VK_CONTROL),
            alt: down(VK_MENU),
            shift: down(VK_SHIFT),
            win: down(VK_LWIN) || down(VK_RWIN),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Combo, Press};

    pub(super) fn listen(_: Option<(Vec<(Combo, u8)>, Press)>) {}
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use winer_core::settings::{HOTKEY_KEYS, normalize_hotkey};

    use super::*;

    fn combo(combination: &str) -> Combo {
        let shortcut = Shortcut::from_str(combination).unwrap();
        Combo::of(&shortcut).unwrap_or_else(|| panic!("{combination} has a virtual key"))
    }

    const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
        win: false,
    };

    #[test]
    fn every_key_the_settings_take_has_its_virtual_key() {
        let letters = ('A'..='Z').map(String::from);
        let digits = ('0'..='9').map(String::from);
        let function = (1..=24).map(|n| format!("F{n}"));
        let named = HOTKEY_KEYS.iter().map(|key| (*key).to_owned());
        let mut keys = std::collections::HashSet::new();
        for key in letters.chain(digits).chain(function).chain(named) {
            let combination = normalize_hotkey(&format!("Alt+{key}")).unwrap();
            let combo = combo(&combination);
            assert!(!is_modifier(combo.key), "{combination} ends in a key");
            assert!(keys.insert(combo.key), "{combination} has a key of its own");
        }
        assert_eq!(combo("Alt+5").key, 0x35);
        assert_eq!(combo("Alt+Backquote").key, 0xC0, "the default, Alt+`");
        assert_eq!(combo("Ctrl+Alt+F12").key, 0x7B);
        assert_eq!(combo("Alt+Num5").key, 0x65);
        assert_eq!(combo("Alt+PageDown").key, 0x22);
        assert_eq!(
            combo("Ctrl+Shift+Super+Q").mods,
            Mods {
                ctrl: true,
                alt: false,
                shift: true,
                win: true
            }
        );
    }

    #[test]
    fn only_the_exact_combination_with_the_game_in_front_is_taken() {
        let mut matcher = Matcher::new();
        matcher.set(vec![(combo("Alt+5"), 2), (combo("Alt+3"), 1)]);
        let shift_alt = Mods { shift: true, ..ALT };
        assert_eq!(
            matcher.key(0x35, true, Mods::default(), || true),
            Verdict::Pass
        );
        assert_eq!(
            matcher.key(0x35, false, Mods::default(), || true),
            Verdict::Pass
        );
        assert_eq!(
            matcher.key(0x35, true, shift_alt, || true),
            Verdict::Pass,
            "a modifier more is another combination"
        );
        assert_eq!(matcher.key(0x34, true, ALT, || true), Verdict::Pass);
        assert_eq!(
            matcher.key(0x35, true, ALT, || false),
            Verdict::Pass,
            "with anything else in front, the system's own shortcut takes it"
        );
        assert_eq!(matcher.key(0x35, false, ALT, || false), Verdict::Pass);
        assert_eq!(matcher.key(0x33, true, ALT, || true), Verdict::Press(1));
    }

    #[test]
    fn a_press_taken_keeps_its_repeats_and_its_release_from_the_game() {
        let mut matcher = Matcher::new();
        matcher.set(vec![(combo("Alt+5"), 2)]);
        let mut asked = 0;
        assert_eq!(
            matcher.key(0x35, true, ALT, || {
                asked += 1;
                true
            }),
            Verdict::Press(2)
        );
        assert_eq!(
            matcher.key(0x35, true, ALT, || panic!("a repeat is not a press")),
            Verdict::Swallow
        );
        assert_eq!(
            matcher.key(0xA4, false, Mods::default(), || true),
            Verdict::Pass,
            "Alt coming up first goes on"
        );
        assert_eq!(
            matcher.key(0x35, true, Mods::default(), || true),
            Verdict::Swallow,
            "repeats after Alt is let go, still the same press"
        );
        assert_eq!(
            matcher.key(0x35, false, Mods::default(), || true),
            Verdict::Swallow
        );
        assert_eq!(
            matcher.key(0x35, false, ALT, || true),
            Verdict::Pass,
            "only one release"
        );
        assert_eq!(
            matcher.key(0x35, true, ALT, || true),
            Verdict::Press(2),
            "a new press"
        );
        assert_eq!(asked, 1);
        // Another key down ends the press: a release lost on the way cannot hold the next one.
        assert_eq!(
            matcher.key(0x51, true, Mods::default(), || true),
            Verdict::Pass
        );
        assert_eq!(matcher.key(0x35, true, ALT, || true), Verdict::Press(2));
        assert_eq!(
            matcher.key(0xA0, true, ALT, || true),
            Verdict::Pass,
            "a modifier down does not end it"
        );
        assert_eq!(matcher.key(0x35, true, ALT, || true), Verdict::Swallow);
    }

    #[test]
    fn listening_for_nothing_takes_nothing() {
        let mut matcher = Matcher::new();
        assert_eq!(matcher.key(0x35, true, ALT, || true), Verdict::Pass);
        matcher.set(vec![(combo("Alt+5"), 2)]);
        matcher.set(Vec::new());
        assert_eq!(matcher.key(0x35, true, ALT, || true), Verdict::Pass);
        listen(None);
    }
}
