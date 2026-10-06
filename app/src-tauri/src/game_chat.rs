//! Typing the callout into the game's own chat, which has no API to post to: synthesized key
//! presses into the game's window (Windows `SendInput`). Enter opens the team chat, the text goes
//! in as Unicode characters (whatever the keyboard layout or input method), Enter sends it, one
//! message at a time. It is opt-in (`automation.callout.inGame`) and only ever types into the
//! game's window while that already is the foreground window: winer never brings it there, and
//! stops at the first key press it finds anything else in front.
//!
//! What is typed, and when, is planned here without touching the system and tested on every
//! platform; `imp` carries the plan out on Windows and does nothing elsewhere.

// The plan is carried out on Windows only; elsewhere it is still compiled for its tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::Duration;

use winer_core::view::{CalloutSkip, NoticeKind};

/// The game's executable: its window is the only one typed into.
const GAME_EXE: &str = "League of Legends.exe";

/// The longest message typed at once, in UTF-16 units. The game cuts a longer message off where
/// its chat's limit is, which was not measured (players report 150 to 230 characters); a line
/// longer than this goes out as two messages instead, well within it.
pub(crate) const MESSAGE_LIMIT: usize = 80;

/// Enter's virtual key and scan code. The scan code goes along: a game reading the keyboard by
/// scan code would otherwise see a key without one.
const VK_RETURN: u16 = 0x0D;
const ENTER_SCAN: u16 = 0x1C;
/// `KEYBDINPUT` flags.
const KEYEVENTF_KEYUP: u32 = 0x0002;
const KEYEVENTF_UNICODE: u32 = 0x0004;

/// One step of typing a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stroke {
    /// Enter, pressed and released: it opens the team chat, and sends what was typed into it.
    Enter,
    /// One character, typed as itself (`KEYEVENTF_UNICODE`) rather than as a key of the layout.
    Char(char),
    /// A wait before the next stroke.
    Pause(Pause),
}

/// The deliberate waits between strokes. Synthesized key presses arrive faster than any hand
/// types, and the game takes its input frame by frame: these give it the frames it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pause {
    /// After the Enter that opens the chat: the box opens on a later frame, and a character that
    /// arrived before it would reach the game as a key press (Q, W, E and R cast spells).
    Open,
    /// Between two characters, so a frame that takes several drops none.
    Char,
    /// Before the Enter that sends: the last characters are in the box by then.
    Send,
    /// Between two messages: the chat box has closed again before the next Enter opens it. An
    /// Enter while it was still open would close it, and the text after it would reach the game
    /// as key presses.
    Message,
}

impl Pause {
    pub(crate) fn duration(self) -> Duration {
        Duration::from_millis(match self {
            Self::Open => 150,
            Self::Char => 8,
            Self::Send => 80,
            Self::Message => 600,
        })
    }
}

/// The messages `lines` are typed as: control characters out (a typed carriage return is an
/// Enter), blank lines dropped, a line longer than [`MESSAGE_LIMIT`] cut in pieces.
pub(crate) fn messages(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.chars().filter(|c| !c.is_control()).collect::<String>())
        .flat_map(|line| chunks(&line, MESSAGE_LIMIT))
        .collect()
}

/// `line` in pieces of at most `limit` UTF-16 units, each cut after the last space or punctuation
/// mark that leaves it at least half full, else at the limit; never inside a character.
pub(crate) fn chunks(line: &str, limit: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = line.trim();
    while !rest.is_empty() {
        // `end` is where the limit falls, `cut` the last good break before it.
        let (mut units, mut end, mut cut) = (0, rest.len(), None);
        for (index, character) in rest.char_indices() {
            if units + character.len_utf16() > limit {
                end = index;
                break;
            }
            units += character.len_utf16();
            if breaks_after(character) && units * 2 >= limit {
                cut = Some(index + character.len_utf8());
            }
        }
        if end == rest.len() {
            pieces.push(rest.to_owned());
            break;
        }
        // A limit narrower than the first character still takes that character.
        let at = match cut.unwrap_or(end) {
            0 => rest.chars().next().map_or(rest.len(), char::len_utf8),
            at => at,
        };
        pieces.push(rest[..at].trim_end().to_owned());
        rest = rest[at..].trim_start();
    }
    pieces
}

/// A piece may end after this character: a space, or punctuation that closes a phrase.
fn breaks_after(character: char) -> bool {
    character.is_whitespace() || "，、；：。！？）」】,;:.!?)".contains(character)
}

/// Every stroke that types `message` into the team chat and sends it.
pub(crate) fn strokes(message: &str) -> Vec<Stroke> {
    let mut strokes = vec![Stroke::Enter, Stroke::Pause(Pause::Open)];
    for (index, character) in message.chars().enumerate() {
        if index > 0 {
            strokes.push(Stroke::Pause(Pause::Char));
        }
        strokes.push(Stroke::Char(character));
    }
    strokes.extend([Stroke::Pause(Pause::Send), Stroke::Enter]);
    strokes
}

/// One keyboard event as `SendInput` takes it: `KEYBDINPUT`'s virtual key, scan code and flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    pub(crate) vk: u16,
    pub(crate) scan: u16,
    pub(crate) flags: u32,
}

/// The events a stroke is made of: a key down, then up; for a character, a down and an up for each
/// of its UTF-16 units (two for one outside the Basic Multilingual Plane). A pause has none.
pub(crate) fn key_events(stroke: Stroke) -> Vec<KeyEvent> {
    match stroke {
        Stroke::Enter => vec![
            KeyEvent {
                vk: VK_RETURN,
                scan: ENTER_SCAN,
                flags: 0,
            },
            KeyEvent {
                vk: VK_RETURN,
                scan: ENTER_SCAN,
                flags: KEYEVENTF_KEYUP,
            },
        ],
        Stroke::Char(character) => character
            .encode_utf16(&mut [0; 2])
            .iter()
            .flat_map(|&unit| {
                [
                    KeyEvent {
                        vk: 0,
                        scan: unit,
                        flags: KEYEVENTF_UNICODE,
                    },
                    KeyEvent {
                        vk: 0,
                        scan: unit,
                        flags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    },
                ]
            })
            .collect(),
        Stroke::Pause(_) => Vec::new(),
    }
}

/// Whether `path`, a process's executable, is the game's.
pub(crate) fn is_game(path: &str) -> bool {
    path.rsplit(['\\', '/'])
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case(GAME_EXE))
}

/// What came of typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Typed {
    /// Every message went out.
    Sent(u32),
    /// Nothing was typed, for this reason.
    Nothing(CalloutSkip),
    /// The typing stopped part of the way, for `reason`; `sent` messages had gone out.
    Stopped { sent: u32, reason: CalloutSkip },
}

/// The activity feed's entry for `typed`.
pub(crate) fn notice(typed: Typed) -> NoticeKind {
    match typed {
        Typed::Sent(lines) => NoticeKind::TypedInGame { lines },
        Typed::Nothing(reason) => NoticeKind::CalloutSkipped { reason },
        Typed::Stopped { sent, reason } => NoticeKind::TypingStopped {
            lines: sent,
            reason,
        },
    }
}

/// Types `lines` into the game's team chat, if the game's window is the foreground window now.
/// Blocks for as long as the typing takes (about half a second a message): call it off the main
/// thread and off the async runtime.
pub(crate) fn type_lines(lines: &[String]) -> Typed {
    imp::type_messages(&messages(lines))
}

#[cfg(windows)]
mod imp {
    #![allow(unsafe_code)]

    use std::{
        thread,
        time::{Duration, Instant},
    };

    use tracing::info;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
                KEYEVENTF_UNICODE, SendInput, VK_RETURN,
            },
            WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        },
    };

    use super::{CalloutSkip, KeyEvent, Pause, Stroke, Typed, is_game, key_events, strokes};

    // The plan's own constants are the system's.
    const _: () = assert!(
        super::VK_RETURN == VK_RETURN
            && super::KEYEVENTF_KEYUP == KEYEVENTF_KEYUP
            && super::KEYEVENTF_UNICODE == KEYEVENTF_UNICODE
    );

    /// The shortcut fires while its keys are still down: typing waits this long at most for them to
    /// come up, checking this often.
    const RELEASE_WAIT: Duration = Duration::from_millis(1500);
    const RELEASE_POLL: Duration = Duration::from_millis(15);

    pub(super) fn type_messages(messages: &[String]) -> Typed {
        let Some(game) = game_in_front() else {
            return Typed::Nothing(CalloutSkip::NotInFront);
        };
        if !keys_released() {
            return Typed::Nothing(CalloutSkip::KeysHeld);
        }
        info!(
            messages = messages.len(),
            "typing the callout into the game's chat"
        );
        let (mut sent, mut pressed) = (0, false);
        for (index, message) in messages.iter().enumerate() {
            if index > 0 {
                thread::sleep(Pause::Message.duration());
            }
            for stroke in strokes(message) {
                if let Stroke::Pause(pause) = stroke {
                    thread::sleep(pause.duration());
                    continue;
                }
                // Checked before every key: once anything else is in front, nothing more is typed.
                // SAFETY: no arguments; a null window is compared like any other.
                let reason = if unsafe { GetForegroundWindow() } != game {
                    CalloutSkip::NotInFront
                } else if send(&key_events(stroke)) {
                    pressed = true;
                    continue;
                } else {
                    CalloutSkip::Blocked
                };
                info!(sent, ?reason, "typing the callout stopped");
                return if pressed {
                    Typed::Stopped { sent, reason }
                } else {
                    Typed::Nothing(reason)
                };
            }
            sent += 1;
        }
        Typed::Sent(sent)
    }

    /// The game's window, when it is the foreground window.
    fn game_in_front() -> Option<HWND> {
        // SAFETY: plain Win32 calls on values checked as they come; the process handle opened
        // here is closed here, and `path` is as long as `size` says.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == 0 {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut path = [0u16; 1024];
            let mut size = path.len() as u32;
            let named = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                path.as_mut_ptr(),
                &mut size,
            ) != 0;
            CloseHandle(process);
            if !named {
                return None;
            }
            let path = String::from_utf16_lossy(&path[..(size as usize).min(path.len())]);
            is_game(&path).then_some(window)
        }
    }

    /// Whether every key of the keyboard is up, waiting [`RELEASE_WAIT`] at most: Shift still held
    /// with the first Enter would open the chat to everyone, not the team, and the shortcut's own
    /// key held down would repeat into the line.
    fn keys_released() -> bool {
        let deadline = Instant::now() + RELEASE_WAIT;
        loop {
            // Every virtual key from Backspace up; below it are the mouse buttons.
            // SAFETY: any virtual key may be asked about; the top bit says it is down.
            let held = (0x08..=0xFE).any(|key| unsafe { GetAsyncKeyState(key) } < 0);
            if !held {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(RELEASE_POLL);
        }
    }

    /// Sends `events` as one batch, so nothing gets between a key's down and up. Whether the system
    /// took every one of them.
    fn send(events: &[KeyEvent]) -> bool {
        let inputs: Vec<INPUT> = events
            .iter()
            .map(|event| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: event.vk,
                        wScan: event.scan,
                        dwFlags: event.flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            })
            .collect();
        // SAFETY: `inputs` holds `inputs.len()` INPUTs, each the size passed.
        let taken = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
        taken as usize == inputs.len()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{CalloutSkip, Typed};

    pub(super) fn type_messages(_: &[String]) -> Typed {
        Typed::Nothing(CalloutSkip::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_enter_the_characters_a_pause_apart_and_enter() {
        use Pause::{Char, Open, Send};
        assert_eq!(
            strokes("小心 a"),
            [
                Stroke::Enter,
                Stroke::Pause(Open),
                Stroke::Char('小'),
                Stroke::Pause(Char),
                Stroke::Char('心'),
                Stroke::Pause(Char),
                Stroke::Char(' '),
                Stroke::Pause(Char),
                Stroke::Char('a'),
                Stroke::Pause(Send),
                Stroke::Enter,
            ]
        );
        assert!(
            Pause::Open.duration() > Pause::Char.duration()
                && Pause::Message.duration() > Pause::Open.duration(),
            "the chat box needs longer to open and close than a character to arrive"
        );
    }

    #[test]
    fn enter_is_a_key_with_its_scan_code_and_a_character_is_typed_as_itself() {
        assert_eq!(
            key_events(Stroke::Enter),
            [
                KeyEvent {
                    vk: 0x0D,
                    scan: 0x1C,
                    flags: 0
                },
                KeyEvent {
                    vk: 0x0D,
                    scan: 0x1C,
                    flags: KEYEVENTF_KEYUP
                },
            ]
        );
        assert_eq!(
            key_events(Stroke::Char('战')),
            [
                KeyEvent {
                    vk: 0,
                    scan: 0x6218,
                    flags: KEYEVENTF_UNICODE
                },
                KeyEvent {
                    vk: 0,
                    scan: 0x6218,
                    flags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                },
            ],
            "no key of the layout, so neither the input method nor Shift changes it"
        );
        let astral: Vec<u16> = key_events(Stroke::Char('😀'))
            .iter()
            .map(|event| event.scan)
            .collect();
        assert_eq!(
            astral,
            [0xD83D, 0xD83D, 0xDE00, 0xDE00],
            "a surrogate pair, each unit down and up"
        );
        assert!(key_events(Stroke::Pause(Pause::Send)).is_empty());
    }

    #[test]
    fn a_long_line_is_cut_at_a_break_into_messages_within_the_limit() {
        assert_eq!(
            chunks("小心 亚索", 80),
            ["小心 亚索"],
            "a short line stays whole"
        );
        assert_eq!(
            chunks("小心 亚索 某某：峡谷通天代，近20场胜率65%", 12),
            ["小心 亚索 某某：", "峡谷通天代，", "近20场胜率65%"],
            "cut after the marks that close a phrase"
        );
        assert_eq!(
            chunks("Watch Yasuo (Someone): Rift Demigod", 16),
            ["Watch Yasuo", "(Someone): Rift", "Demigod"],
            "at a space, the space dropped"
        );
        assert_eq!(
            chunks("一二三四五六七八九十", 4),
            ["一二三四", "五六七八", "九十"],
            "no break at all: cut at the limit"
        );
        assert_eq!(
            chunks("a，bcdefgh", 6),
            ["a，bcde", "fgh"],
            "a break too early would leave a piece mostly empty"
        );
        assert_eq!(
            chunks("ab😀", 3),
            ["ab", "😀"],
            "never inside a surrogate pair"
        );
        assert_eq!(
            chunks("😀", 1),
            ["😀"],
            "a character wider than the limit still goes"
        );
        assert!(chunks("   ", 10).is_empty());
        for piece in chunks(&"峡谷通天代，近20场胜率65%".repeat(12), MESSAGE_LIMIT) {
            assert!(piece.encode_utf16().count() <= MESSAGE_LIMIT, "{piece}");
        }
    }

    #[test]
    fn the_messages_drop_blank_lines_and_control_characters() {
        let lines = [
            "【敌方·红色方】winer 战绩鉴定".to_owned(),
            "  ".to_owned(),
            "对面\r 盖伦\n".to_owned(),
        ];
        assert_eq!(
            messages(&lines),
            ["【敌方·红色方】winer 战绩鉴定", "对面 盖伦"],
            "a typed carriage return would be an Enter of its own"
        );
        let long = format!("小心：{}", "近20场胜率65%，".repeat(10));
        assert!(
            messages(&[long]).len() > 1,
            "longer than a message: more than one"
        );
    }

    #[test]
    fn only_the_games_executable_counts_as_the_game() {
        assert!(is_game(
            r"D:\software\lol\英雄联盟(26)\Game\League of Legends.exe"
        ));
        assert!(is_game(
            "C:/Riot Games/League of Legends/Game/league of legends.EXE"
        ));
        assert!(!is_game(
            r"D:\software\lol\英雄联盟(26)\LeagueClient\LeagueClientUx.exe"
        ));
        assert!(!is_game(r"C:\Windows\explorer.exe"));
        assert!(!is_game(""));
    }

    #[test]
    fn what_came_of_typing_is_put_in_the_activity_feed() {
        assert_eq!(notice(Typed::Sent(3)), NoticeKind::TypedInGame { lines: 3 });
        assert_eq!(
            notice(Typed::Nothing(CalloutSkip::NotInFront)),
            NoticeKind::CalloutSkipped {
                reason: CalloutSkip::NotInFront
            }
        );
        assert_eq!(
            notice(Typed::Stopped {
                sent: 1,
                reason: CalloutSkip::NotInFront
            }),
            NoticeKind::TypingStopped {
                lines: 1,
                reason: CalloutSkip::NotInFront
            }
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn nothing_is_typed_off_windows() {
        assert_eq!(
            type_lines(&["小心".to_owned()]),
            Typed::Nothing(CalloutSkip::Unsupported)
        );
    }
}
