//! The Comebacks page (反击): which line a press of its two shortcuts sends and where, and the
//! library a user may download (在线语录); and the Phrases page (快捷语): the quick phrases a teammate
//! is sent from champ select's board, by their own shortcut or by a trade the user asks for (交换). Pure: the service keeps what
//! went out last and the downloaded file (`service/comeback.rs`); the shell types into the game.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{
    callout,
    model::{ChampSelectSession, TRADE_SENT},
    settings::{ComebackSettings, Language, LibraryLevel},
    sqlite::{self, Value},
    view::{CalloutSkip, ComebackLibrary, ComebackLibraryLines, ComebackPlace, Phase, PhraseSkip},
};

/// The least time between two comebacks that go out: presses one after another flood the chat,
/// and a flood is what gets an account reported.
pub const SPACING: Duration = Duration::from_secs(3);

/// What a press of a comeback's shortcut does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComebackPress {
    /// Champ select `game_id`: this line goes to its chat, and to no later champ select's.
    ChampSelect { line: String, game_id: i64 },
    /// The game runs and in-game sending is on: the shell types this line into the game's chat, to
    /// everyone with `all`.
    Game { line: String, all: bool },
    /// Nothing is sent.
    Skip {
        place: ComebackPlace,
        reason: CalloutSkip,
    },
}

/// What went out last, and when it finished going out: the next press neither repeats it nor
/// follows it within [`SPACING`]. Only a line that reached the chat, whole or in part, counts; a
/// press that sent nothing leaves it as it was. A quick phrase sent by its shortcut starts the
/// spacing too, and is no line to avoid.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub last: Option<String>,
    pub sent_at: Option<Instant>,
}

/// Where a press of the team's shortcut (`all` false) or the all-chat one goes in `phase`.
pub fn place(phase: Phase, all: bool) -> ComebackPlace {
    match (phase, all) {
        (Phase::ChampSelect, _) => ComebackPlace::ChampSelect,
        (_, true) => ComebackPlace::All,
        (_, false) => ComebackPlace::Team,
    }
}

/// What a press does in `phase` under `rule`: in champ select (`champ_select`, its game id, once
/// its view is in) a line of `pool` goes to its chat; while the game runs (`InProgress`: not its
/// loading screen) the shell types one, if in-game sending is on. The line is [`pick`]ed with
/// `random`; nothing goes out within [`SPACING`] of the last line that did (`memory`, at `now`).
#[allow(clippy::too_many_arguments)]
pub fn press(
    phase: Phase,
    champ_select: Option<i64>,
    rule: &ComebackSettings,
    all: bool,
    pool: &[&str],
    memory: &Memory,
    now: Instant,
    random: u64,
) -> ComebackPress {
    let skip = |reason| ComebackPress::Skip {
        place: place(phase, all),
        reason,
    };
    match (phase, champ_select) {
        (Phase::ChampSelect, Some(_)) => {}
        (Phase::InProgress, _) if !rule.in_game => return skip(CalloutSkip::InGameOff),
        (Phase::InProgress, _) => {}
        _ => return skip(CalloutSkip::NotNow),
    }
    let Some(index) = pick(pool, memory.last.as_deref(), random) else {
        return skip(CalloutSkip::NoLines);
    };
    if memory
        .sent_at
        .is_some_and(|at| now.saturating_duration_since(at) < SPACING)
    {
        return skip(CalloutSkip::TooSoon);
    }
    let line = pool[index].to_owned();
    match champ_select.filter(|_| phase == Phase::ChampSelect) {
        Some(game_id) => ComebackPress::ChampSelect { line, game_id },
        None => ComebackPress::Game { line, all },
    }
}

/// Which line of `pool` to send: any, chosen by `random`, but never one equal to `last` while the
/// pool holds another. `None` for an empty pool.
pub fn pick(pool: &[&str], last: Option<&str>, random: u64) -> Option<usize> {
    let fresh: Vec<usize> = (0..pool.len())
        .filter(|&index| Some(pool[index]) != last)
        .collect();
    let choices: Vec<usize> = if fresh.is_empty() {
        (0..pool.len()).collect()
    } else {
        fresh
    };
    let count = u64::try_from(choices.len())
        .ok()
        .filter(|&count| count > 0)?;
    choices.get(usize::try_from(random % count).ok()?).copied()
}

/// The lines a press picks from: the user's own, then, with the library switched on and
/// downloaded, its lines of the chosen level.
pub fn pool<'a>(rule: &'a ComebackSettings, library: Option<&'a Library>) -> Vec<&'a str> {
    let own = rule.lines.iter().map(String::as_str);
    let downloaded = library
        .filter(|_| rule.library.enabled)
        .map(|library| library.lines(rule.library.level))
        .into_iter()
        .flatten();
    own.chain(downloaded).collect()
}

// ---- Quick phrases (选人快捷语) ----

/// The quick phrases the Live page offers: the user's, or the language's own while there are none.
pub fn phrases(rule: &ComebackSettings, language: Language) -> Vec<String> {
    if rule.phrases.is_empty() {
        default_phrases(language)
            .iter()
            .map(|&phrase| phrase.to_owned())
            .collect()
    } else {
        rule.phrases.clone()
    }
}

/// The language's quick phrases: the owner's own words, with the teammate's seat and champion.
pub fn default_phrases(language: Language) -> &'static [&'static str] {
    match language {
        Language::ZhCn => &["{seat}大佬，小弟想玩下{champion}，给您磕一个了"],
        Language::En => &["{seat}, may I play {champion}? Thank you!"],
    }
}

/// A quick phrase for the teammate in `seat` (`callout::seat_label`), on `champion` (blank while
/// none is shown), called `name` (blank when hidden): a blank value takes its brackets along, as
/// in the callout.
pub fn phrase(template: &str, seat: &str, champion: &str, name: &str) -> String {
    callout::fill(
        template,
        [
            ("{seat}", seat.to_owned()),
            ("{champion}", champion.to_owned()),
            ("{name}", name.to_owned()),
        ],
    )
}

/// What a press of the quick phrase's shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhrasePress {
    /// The first quick phrase goes to champ select `game_id`'s chat, for the teammate in `cell_id`.
    Send { game_id: i64, cell_id: i64 },
    /// Nothing is sent.
    Skip(PhraseSkip),
}

/// What a press does in `phase`, with champ select's session as the client last sent it: the first
/// quick phrase is for the teammate the user has asked to trade champions with (交换), the one
/// they would pick from the board; nothing goes out within [`SPACING`] of the last line or phrase
/// that did (`memory`, at `now`).
pub fn phrase_press(
    phase: Phase,
    session: Option<&ChampSelectSession>,
    memory: &Memory,
    now: Instant,
) -> PhrasePress {
    let Some(session) = session.filter(|_| phase == Phase::ChampSelect) else {
        return PhrasePress::Skip(PhraseSkip::NotChampSelect);
    };
    let Some(cell_id) = session.trade_asked() else {
        return PhrasePress::Skip(PhraseSkip::NoTrade);
    };
    if memory
        .sent_at
        .is_some_and(|at| now.saturating_duration_since(at) < SPACING)
    {
        return PhrasePress::Skip(PhraseSkip::TooSoon);
    }
    PhrasePress::Send {
        game_id: session.game_id,
        cell_id,
    }
}

// ---- Trades (交换): what the user's own requests send by themselves (`settings::TradeRule`) ----

/// The thank-you a trade that went through sends while the user has written none.
pub fn default_thanks(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "谢谢{seat}大佬！",
        Language::En => "Thanks, {seat}!",
    }
}

/// The thank-you template the rule holds, or the language's own.
pub fn thanks(rule: &ComebackSettings, language: Language) -> String {
    if rule.trade.thanks.is_empty() {
        default_thanks(language).to_owned()
    } else {
        rule.trade.thanks.clone()
    }
}

/// What a trade the user asked for did, as one champ select's sessions show it (`trade_events`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeEvent {
    /// The user asked the teammate in `cell_id` for their champion, `theirs`: the first ask of that
    /// teammate this champ select.
    Asked { cell_id: i64, theirs: i64 },
    /// A trade the user asked of the teammate in `cell_id` went through: the user now holds
    /// `theirs`. Once per teammate per champ select.
    Done { cell_id: i64, theirs: i64 },
}

/// The request being followed: the teammate's cell, and both champions as they were when it was
/// made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ask {
    cell_id: i64,
    mine: i64,
    theirs: i64,
}

/// What [`trade_events`] keeps from one session of a champ select to the next.
#[derive(Clone, Debug, Default)]
pub struct TradeWatch {
    game_id: Option<i64>,
    /// Each cell's trade state in the last session, so a cell turning `SENT` is seen.
    states: HashMap<i64, String>,
    /// The cells asked, and the cells thanked: a message each, once per champ select. Kept apart
    /// from `pending`, so an ask made again after a cancel is still followed to its end.
    asked: HashSet<i64>,
    thanked: HashSet<i64>,
    /// The user's request now; the latest replaces any other.
    pending: Option<Ask>,
}

/// What the trades of `session` did since the last session `watch` saw. A new request is a cell
/// whose trade reads `SENT` and did not before; it went through once the user holds the champion
/// that teammate had and they hold the user's, whatever the trades read then (the client rebuilds
/// them after a swap). It is still waiting while the cell reads `SENT` or `ACCEPTED`, or has no
/// trade at all, and neither champion moved; any other state, or another champion, ends it with no
/// event. A session of another champ select starts the watch afresh.
pub fn trade_events(watch: &mut TradeWatch, session: &ChampSelectSession) -> Vec<TradeEvent> {
    if watch.game_id != Some(session.game_id) {
        *watch = TradeWatch {
            game_id: Some(session.game_id),
            ..TradeWatch::default()
        };
    }
    let champion = |cell_id: i64| {
        session
            .my_team
            .iter()
            .find(|player| player.cell_id == cell_id)
            .map(|player| player.champion_id)
    };
    let mine = champion(session.local_player_cell_id);
    let mut events = Vec::new();
    if let Some(ask) = watch.pending {
        let theirs = champion(ask.cell_id);
        if mine == Some(ask.theirs) && theirs == Some(ask.mine) {
            watch.pending = None;
            if watch.thanked.insert(ask.cell_id) {
                events.push(TradeEvent::Done {
                    cell_id: ask.cell_id,
                    theirs: ask.theirs,
                });
            }
        } else {
            let unmoved = mine == Some(ask.mine) && theirs == Some(ask.theirs);
            let waiting = session
                .trades
                .iter()
                .find(|trade| trade.cell_id == ask.cell_id)
                .is_none_or(|trade| {
                    trade.state.eq_ignore_ascii_case(TRADE_SENT)
                        || trade.state.eq_ignore_ascii_case(TRADE_ACCEPTED)
                });
            if !(unmoved && waiting) {
                watch.pending = None;
            }
        }
    }
    for trade in &session.trades {
        let was_sent = watch
            .states
            .get(&trade.cell_id)
            .is_some_and(|state| state.eq_ignore_ascii_case(TRADE_SENT));
        if !trade.state.eq_ignore_ascii_case(TRADE_SENT) || was_sent {
            continue;
        }
        let (Some(mine), Some(theirs)) = (mine, champion(trade.cell_id)) else {
            continue;
        };
        watch.pending = Some(Ask {
            cell_id: trade.cell_id,
            mine,
            theirs,
        });
        if watch.asked.insert(trade.cell_id) {
            events.push(TradeEvent::Asked {
                cell_id: trade.cell_id,
                theirs,
            });
        }
    }
    watch.states = session
        .trades
        .iter()
        .map(|trade| (trade.cell_id, trade.state.clone()))
        .collect();
    events
}

/// The state of a trade the teammate accepted, which the client may report before the swap.
const TRADE_ACCEPTED: &str = "ACCEPTED";

// ---- Lines as a message holds them ----

/// `text` as one chat message holds it: every control character (a newline among them) and every
/// run of white space one space, trimmed.
pub(crate) fn clean_line(text: &str) -> String {
    text.split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `lines` cleaned ([`clean_line`]) and cut to `length` characters, blanks dropped, at most `limit`.
pub(crate) fn clean_lines(lines: &[String], limit: usize, length: usize) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            clean_line(line)
                .chars()
                .take(length)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .filter(|line| !line.is_empty())
        .take(limit)
        .collect()
}

// ---- The downloaded library (在线语录) ----

/// The library's repository: the open 祖安语录 / 骂人宝典 (Apache-2.0), which ZuAnBot's README names
/// as its inspiration. winer ships none of it; the user downloads it on the Comebacks page.
pub const LIBRARY_REPOSITORY: &str = "cndiandian/zuanbot.com";
/// The commit that added the file, which no later commit changed (read 2026-11-02).
pub const LIBRARY_COMMIT: &str = "a79264d56181395b14fa8492e35ef54a286fc808";
/// The file in it: a SQLite database whose table `main` holds the lines.
pub const LIBRARY_PATH: &str = "db/data.db";
/// The file's size and SHA-256 at that commit: any other bytes are refused.
pub const LIBRARY_SIZE: usize = 970_752;
pub const LIBRARY_SHA256: &str = "ee2a22eeee994ee51d6acfc30e88e61ab0f61eae4a328a8c80fdbd60d5171ce2";
/// The longest line the library keeps, as the user's own: a longer one is dropped, not cut.
pub const LIBRARY_LINE_LENGTH: usize = ComebackSettings::LINE_LENGTH;
/// The most lines either set keeps once the user edits it: the pinned file has under a thousand a
/// set, and this bounds what the window sends and the file holds.
pub const LIBRARY_LINE_LIMIT: usize = 5000;

/// Where the file is fetched from, in order: three of jsDelivr's hosts, which serve the
/// repository at that commit and are usually reachable in China, then GitHub itself.
pub fn library_urls() -> Vec<String> {
    let mirror = |host: &str| {
        format!("https://{host}/gh/{LIBRARY_REPOSITORY}@{LIBRARY_COMMIT}/{LIBRARY_PATH}")
    };
    vec![
        mirror("cdn.jsdelivr.net"),
        mirror("fastly.jsdelivr.net"),
        mirror("gcore.jsdelivr.net"),
        format!(
            "https://raw.githubusercontent.com/{LIBRARY_REPOSITORY}/{LIBRARY_COMMIT}/{LIBRARY_PATH}"
        ),
    ]
}

/// How long one mirror may take, from connecting to the last byte.
pub const FETCH_LIMIT: Duration = Duration::from_secs(30);

/// The library from the first of `urls` that serves exactly `size` bytes whose SHA-256 is
/// `sha256`, each mirror within [`FETCH_LIMIT`]: its lines read and cleaned ([`library_from`]).
/// The error names what each mirror did.
pub async fn download(
    client: &reqwest::Client,
    urls: &[String],
    size: usize,
    sha256: &str,
    fetched_at: i64,
) -> Result<Library, String> {
    let mut failures = Vec::new();
    for url in urls {
        let host = url.split('/').nth(2).unwrap_or(url.as_str()).to_owned();
        match fetch(client, url, size).await {
            Ok(bytes) if digest(&bytes) == sha256 => {
                return library_from(&bytes, fetched_at);
            }
            Ok(_) => failures.push(format!("{host}: not the file winer knows")),
            Err(error) => failures.push(format!("{host}: {error}")),
        }
        tracing::warn!(host, "a mirror of the library failed");
    }
    Err(format!(
        "the library could not be downloaded ({})",
        failures.join("; ")
    ))
}

/// `url`'s body, refused past `size` bytes.
async fn fetch(client: &reqwest::Client, url: &str, size: usize) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .timeout(FETCH_LIMIT)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let mut bytes = Vec::with_capacity(size);
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > size {
            return Err("more bytes than the file winer knows".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// The lowercase hex SHA-256 of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The library in the database `file`: table `main`'s `text` by its `level`, `min` 口吐芬芳 and
/// `max` 火力全开, each line cleaned ([`clean_line`]); blank, repeated and longer than
/// [`LIBRARY_LINE_LENGTH`] characters dropped, as are rows of any other level.
pub fn library_from(file: &[u8], fetched_at: i64) -> Result<Library, String> {
    let table = sqlite::table(file, "main")
        .map_err(|error| format!("the library's file does not read: {error}"))?;
    let (text, level) = table
        .column("text")
        .zip(table.column("level"))
        .ok_or("the library's table has no text and level")?;
    let (mut mild, mut strong, mut seen) = (Vec::new(), Vec::new(), HashSet::new());
    for row in &table.rows {
        let (Some(Value::Text(line)), Some(Value::Text(level))) = (row.get(text), row.get(level))
        else {
            continue;
        };
        let set = match level.as_str() {
            "min" => &mut mild,
            "max" => &mut strong,
            _ => continue,
        };
        let line = clean_line(line);
        if line.is_empty()
            || line.chars().count() > LIBRARY_LINE_LENGTH
            || !seen.insert(line.clone())
        {
            continue;
        }
        set.push(line);
    }
    Ok(Library {
        source: format!("{LIBRARY_REPOSITORY}@{LIBRARY_COMMIT}"),
        sha256: digest(file),
        fetched_at,
        edited: false,
        mild,
        strong,
    })
}

/// A set of the library as the user edited it: each line cleaned ([`clean_line`]) and cut to
/// [`LIBRARY_LINE_LENGTH`] characters, blanks and repeats within the set dropped (the first kept),
/// at most [`LIBRARY_LINE_LIMIT`].
pub fn edited_lines(lines: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    clean_lines(lines, usize::MAX, LIBRARY_LINE_LENGTH)
        .into_iter()
        .filter(|line| seen.insert(line.clone()))
        .take(LIBRARY_LINE_LIMIT)
        .collect()
}

/// The library as kept beside the settings (`comeback-library.json`): its lines cleaned, by set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// `cndiandian/zuanbot.com@<commit>`.
    pub source: String,
    /// The SHA-256 of the file the lines were read from.
    pub sha256: String,
    /// When it was downloaded, in milliseconds since the epoch.
    pub fetched_at: i64,
    /// The user changed its lines since the download (`edited_lines`): removing it loses that.
    #[serde(default)]
    pub edited: bool,
    /// 口吐芬芳: the rows whose `level` is `min`.
    pub mild: Vec<String>,
    /// 火力全开: the rows whose `level` is `max`.
    pub strong: Vec<String>,
}

impl Library {
    /// Its lines of `level`.
    pub fn lines(&self, level: LibraryLevel) -> impl Iterator<Item = &str> {
        let (mild, strong) = match level {
            LibraryLevel::Mild => (true, false),
            LibraryLevel::Strong => (false, true),
            LibraryLevel::Both => (true, true),
        };
        let none: &[String] = &[];
        let mild = if mild { self.mild.as_slice() } else { none };
        let strong = if strong { self.strong.as_slice() } else { none };
        mild.iter().chain(strong).map(String::as_str)
    }

    /// What the Comebacks page shows of it.
    pub fn status(&self) -> ComebackLibrary {
        ComebackLibrary {
            downloaded: true,
            mild: u32::try_from(self.mild.len()).unwrap_or(u32::MAX),
            strong: u32::try_from(self.strong.len()).unwrap_or(u32::MAX),
            fetched_at: Some(self.fetched_at),
            edited: self.edited,
        }
    }

    /// Its lines by set, as the Comebacks page lists them for editing.
    pub fn sets(&self) -> ComebackLibraryLines {
        ComebackLibraryLines {
            mild: self.mild.clone(),
            strong: self.strong.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_never_sends_the_last_line_again_while_there_is_another() {
        let pool = ["a", "b", "c"];
        for random in 0..30 {
            let index = pick(&pool, Some("b"), random).unwrap();
            assert_ne!(pool[index], "b");
        }
        let seen: std::collections::HashSet<usize> = (0..30)
            .filter_map(|random| pick(&pool, Some("b"), random))
            .collect();
        assert_eq!(seen.len(), 2, "every other line can come");
        assert_eq!(pick(&["only"], Some("only"), 7), Some(0), "one line, again");
        assert_eq!(pick(&["x", "x"], Some("x"), 3).map(|i| i < 2), Some(true));
        assert_eq!(pick(&[], None, 1), None);
    }

    fn rule(lines: &[&str], in_game: bool) -> ComebackSettings {
        ComebackSettings {
            lines: lines.iter().map(|&line| line.to_owned()).collect(),
            in_game,
            ..ComebackSettings::default()
        }
    }

    /// A champ select of game 9 as the client sends it: the user in cell 7, the team's champions by
    /// cell, and the trades by cell and state.
    fn trading(game_id: i64, team: &[(i64, i64)], trades: &[(i64, &str)]) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "gameId": game_id,
            "localPlayerCellId": 7,
            "myTeam": team
                .iter()
                .map(|(cell, champion)| serde_json::json!({ "cellId": cell, "championId": champion }))
                .collect::<Vec<_>>(),
            "trades": trades
                .iter()
                .enumerate()
                .map(|(id, (cell, state))| serde_json::json!({
                    "id": id + 40, "cellId": cell, "state": state
                }))
                .collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    /// The user (cell 7) on Talon (32), the teammate in cell 5 on Annie (1) and in cell 6 on Ahri (103).
    const TEAM: [(i64, i64); 3] = [(5, 1), (6, 103), (7, 32)];
    /// The same after the user and cell 5 swapped.
    const SWAPPED: [(i64, i64); 3] = [(5, 32), (6, 103), (7, 1)];

    #[test]
    fn a_trade_asked_sends_once_and_thanks_once_its_champions_are_swapped() {
        let mut watch = TradeWatch::default();
        let events = |watch: &mut TradeWatch, team: &[(i64, i64)], trades: &[(i64, &str)]| {
            trade_events(watch, &trading(9, team, trades))
        };
        assert_eq!(
            events(&mut watch, &TEAM, &[(5, "AVAILABLE"), (6, "INVALID")]),
            []
        );
        assert_eq!(
            events(&mut watch, &TEAM, &[(5, "SENT"), (6, "INVALID")]),
            [TradeEvent::Asked {
                cell_id: 5,
                theirs: 1
            }]
        );
        assert_eq!(
            events(&mut watch, &TEAM, &[(5, "SENT"), (6, "INVALID")]),
            [],
            "still the same ask"
        );
        assert_eq!(
            events(&mut watch, &TEAM, &[(5, "ACCEPTED"), (6, "INVALID")]),
            [],
            "accepted, not swapped yet: still followed"
        );
        assert_eq!(
            events(&mut watch, &SWAPPED, &[(5, "AVAILABLE"), (6, "AVAILABLE")]),
            [TradeEvent::Done {
                cell_id: 5,
                theirs: 1
            }],
            "the trades are rebuilt after the swap"
        );
        assert_eq!(events(&mut watch, &SWAPPED, &[(5, "AVAILABLE")]), []);
    }

    #[test]
    fn an_ask_made_again_after_a_cancel_is_not_sent_twice_but_still_thanked() {
        let mut watch = TradeWatch::default();
        let mut events = |team: &[(i64, i64)], trades: &[(i64, &str)]| {
            trade_events(&mut watch, &trading(9, team, trades))
        };
        assert_eq!(events(&TEAM, &[(5, "SENT")]).len(), 1);
        assert_eq!(events(&TEAM, &[(5, "CANCELLED")]), []);
        assert_eq!(events(&TEAM, &[(5, "AVAILABLE")]), []);
        assert_eq!(
            events(&TEAM, &[(5, "SENT")]),
            [],
            "that teammate was asked already"
        );
        assert_eq!(
            events(&TEAM, &[]),
            [],
            "no trade in the list while nothing moved: still followed"
        );
        assert_eq!(
            events(&SWAPPED, &[]),
            [TradeEvent::Done {
                cell_id: 5,
                theirs: 1
            }]
        );
    }

    #[test]
    fn nothing_is_thanked_but_the_users_own_trade_that_went_through() {
        // Declined, then the two swap all the same: the ask had ended.
        let mut watch = TradeWatch::default();
        trade_events(&mut watch, &trading(9, &TEAM, &[(5, "SENT")]));
        assert_eq!(
            trade_events(&mut watch, &trading(9, &TEAM, &[(5, "DECLINED")])),
            []
        );
        assert_eq!(trade_events(&mut watch, &trading(9, &SWAPPED, &[])), []);
        // The teammate took another champion: the ask ended, and its swap never comes.
        let mut watch = TradeWatch::default();
        trade_events(&mut watch, &trading(9, &TEAM, &[(5, "SENT")]));
        let other = [(5, 22), (6, 103), (7, 32)];
        assert_eq!(
            trade_events(&mut watch, &trading(9, &other, &[(5, "SENT")])),
            []
        );
        assert_eq!(trade_events(&mut watch, &trading(9, &SWAPPED, &[])), []);
        // Two other teammates trading each other, a request to the user, a cell never asked.
        let mut watch = TradeWatch::default();
        assert_eq!(
            trade_events(&mut watch, &trading(9, &TEAM, &[(5, "BUSY"), (6, "BUSY")])),
            []
        );
        assert_eq!(
            trade_events(&mut watch, &trading(9, &TEAM, &[(5, "RECEIVED")])),
            []
        );
        assert_eq!(trade_events(&mut watch, &trading(9, &SWAPPED, &[])), []);
    }

    #[test]
    fn another_champ_select_starts_the_watch_afresh() {
        let mut watch = TradeWatch::default();
        assert_eq!(
            trade_events(&mut watch, &trading(9, &TEAM, &[(5, "SENT")])).len(),
            1
        );
        assert_eq!(
            trade_events(&mut watch, &trading(10, &TEAM, &[(5, "SENT")])),
            [TradeEvent::Asked {
                cell_id: 5,
                theirs: 1
            }],
            "game 10 is another champ select"
        );
        assert_eq!(
            trade_events(&mut watch, &trading(9, &SWAPPED, &[])),
            [],
            "nor does game 9's swap count after it"
        );
    }

    /// A recorder's sample: `(cell, champion)` for the team, and the trades as the client listed
    /// them, `(id, cell, state)`.
    type Sample = ([(i64, i64); 5], &'static [(i64, i64, &'static str)]);

    /// One sample of the read-only recorder on the owner's PC: the local cell, `cell:champion` for
    /// the team, and the trades as the client listed them (`id`, `cell`, `state`).
    fn recorded(
        game_id: i64,
        local: i64,
        team: [(i64, i64); 5],
        trades: &[(i64, i64, &str)],
    ) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "gameId": game_id,
            "localPlayerCellId": local,
            "benchEnabled": true,
            "myTeam": team
                .iter()
                .map(|(cell, champion)| serde_json::json!({ "cellId": cell, "championId": champion }))
                .collect::<Vec<_>>(),
            "trades": trades
                .iter()
                .map(|(id, cell, state)| serde_json::json!({ "id": id, "cellId": cell, "state": state }))
                .collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    #[test]
    fn the_champ_selects_recorded_on_the_owners_pc_ask_and_thank_exactly_once() {
        // ARAM, 2026-11-06 22:46 CST: the user in cell 3, on champion 6, asks cell 4, on 202.
        let game = 601027531658;
        let samples: [Sample; 6] = [
            (
                [(0, 39), (1, 50), (2, 10), (3, 6), (4, 202)],
                &[
                    (44, 2, "AVAILABLE"),
                    (46, 1, "AVAILABLE"),
                    (48, 0, "AVAILABLE"),
                    (57, 4, "AVAILABLE"),
                ],
            ),
            (
                [(0, 39), (1, 50), (2, 10), (3, 6), (4, 202)],
                &[
                    (44, 2, "INVALID"),
                    (46, 1, "INVALID"),
                    (48, 0, "INVALID"),
                    (57, 4, "SENT"),
                ],
            ),
            (
                [(0, 39), (1, 50), (2, 10), (3, 202), (4, 6)],
                &[
                    (64, 0, "AVAILABLE"),
                    (71, 4, "AVAILABLE"),
                    (60, 2, "AVAILABLE"),
                    (62, 1, "AVAILABLE"),
                ],
            ),
            // Cell 4 took a bench champion, then cell 2 did.
            (
                [(0, 39), (1, 50), (2, 10), (3, 202), (4, 99)],
                &[
                    (64, 0, "AVAILABLE"),
                    (79, 4, "AVAILABLE"),
                    (60, 2, "AVAILABLE"),
                    (62, 1, "AVAILABLE"),
                ],
            ),
            (
                [(0, 39), (1, 50), (2, 221), (3, 202), (4, 99)],
                &[
                    (64, 0, "AVAILABLE"),
                    (79, 4, "AVAILABLE"),
                    (86, 2, "AVAILABLE"),
                    (62, 1, "AVAILABLE"),
                ],
            ),
            ([(0, 39), (1, 50), (2, 221), (3, 202), (4, 99)], &[]),
        ];
        let mut watch = TradeWatch::default();
        let events: Vec<_> = samples
            .iter()
            .map(|(team, trades)| trade_events(&mut watch, &recorded(game, 3, *team, trades)))
            .collect();
        assert_eq!(
            events,
            [
                vec![],
                vec![TradeEvent::Asked {
                    cell_id: 4,
                    theirs: 202
                }],
                vec![TradeEvent::Done {
                    cell_id: 4,
                    theirs: 202
                }],
                vec![],
                vec![],
                vec![],
            ]
        );

        // ARAM, 2026-11-06 21:57 CST: the user in cell 2 swaps from the bench (85 for 16) while cells
        // 1 and 4 trade each other (BUSY on both, then 85 and 17 change places).
        let game = 601027440794;
        let samples: [Sample; 4] = [
            (
                [(0, 91), (1, 85), (2, 16), (3, 64), (4, 17)],
                &[
                    (77, 0, "INVALID"),
                    (78, 4, "AVAILABLE"),
                    (80, 3, "AVAILABLE"),
                    (85, 1, "AVAILABLE"),
                ],
            ),
            (
                [(0, 91), (1, 85), (2, 16), (3, 64), (4, 17)],
                &[
                    (77, 0, "INVALID"),
                    (78, 4, "BUSY"),
                    (85, 1, "BUSY"),
                    (92, 3, "AVAILABLE"),
                ],
            ),
            (
                [(0, 518), (1, 17), (2, 16), (3, 64), (4, 85)],
                &[
                    (102, 4, "AVAILABLE"),
                    (107, 1, "AVAILABLE"),
                    (119, 0, "INVALID"),
                    (92, 3, "AVAILABLE"),
                ],
            ),
            ([(0, 518), (1, 17), (2, 16), (3, 64), (4, 83)], &[]),
        ];
        let mut watch = TradeWatch::default();
        for (team, trades) in samples {
            assert_eq!(
                trade_events(&mut watch, &recorded(game, 2, team, trades)),
                [],
                "nobody asked the user, and the user asked nobody"
            );
        }
    }

    #[test]
    fn the_thank_you_is_the_users_or_the_languages() {
        let mut rule = ComebackSettings::default();
        assert_eq!(thanks(&rule, Language::ZhCn), "谢谢{seat}大佬！");
        assert_eq!(thanks(&rule, Language::En), "Thanks, {seat}!");
        rule.trade.thanks = "{seat}，{champion}我拿走了，谢谢".into();
        assert_eq!(
            thanks(&rule, Language::ZhCn),
            "{seat}，{champion}我拿走了，谢谢"
        );
    }

    #[test]
    fn the_quick_phrases_shortcut_names_the_teammate_asked_to_trade() {
        let session = |trades: &[(i64, &str)]| -> ChampSelectSession {
            serde_json::from_value(serde_json::json!({
                "gameId": 601024537150_i64,
                "localPlayerCellId": 7,
                "trades": trades
                    .iter()
                    .enumerate()
                    .map(|(id, (cell, state))| serde_json::json!({
                        "id": id + 20, "cellId": cell, "state": state
                    }))
                    .collect::<Vec<_>>(),
            }))
            .unwrap()
        };
        let now = Instant::now();
        let memory = Memory::default();
        let asked = session(&[
            (6, "AVAILABLE"),
            (9, "INVALID"),
            (5, "SENT"),
            (8, "AVAILABLE"),
        ]);
        assert_eq!(
            phrase_press(Phase::ChampSelect, Some(&asked), &memory, now),
            PhrasePress::Send {
                game_id: 601024537150,
                cell_id: 5
            }
        );
        let none = session(&[(6, "AVAILABLE"), (9, "INVALID")]);
        assert_eq!(
            phrase_press(Phase::ChampSelect, Some(&none), &memory, now),
            PhrasePress::Skip(PhraseSkip::NoTrade),
            "no ask out, or answered"
        );
        assert_eq!(
            phrase_press(Phase::ChampSelect, None, &memory, now),
            PhrasePress::Skip(PhraseSkip::NotChampSelect),
            "its session is not in yet"
        );
        assert_eq!(
            phrase_press(Phase::InProgress, Some(&asked), &memory, now),
            PhrasePress::Skip(PhraseSkip::NotChampSelect)
        );
        let recent = Memory {
            last: None,
            sent_at: Some(now),
        };
        assert_eq!(
            phrase_press(Phase::ChampSelect, Some(&asked), &recent, now + SPACING / 2),
            PhrasePress::Skip(PhraseSkip::TooSoon)
        );
        assert!(matches!(
            phrase_press(Phase::ChampSelect, Some(&asked), &recent, now + SPACING),
            PhrasePress::Send { .. }
        ));
    }

    #[test]
    fn a_press_goes_to_champ_select_or_the_game_or_says_why_not() {
        let now = Instant::now();
        let memory = Memory::default();
        let on = rule(&["hi"], true);
        let pool: Vec<&str> = pool(&on, None);
        assert_eq!(
            press(
                Phase::ChampSelect,
                Some(7),
                &on,
                true,
                &pool,
                &memory,
                now,
                0
            ),
            ComebackPress::ChampSelect {
                line: "hi".into(),
                game_id: 7
            }
        );
        assert_eq!(
            press(Phase::ChampSelect, None, &on, true, &pool, &memory, now, 0),
            ComebackPress::Skip {
                place: ComebackPlace::ChampSelect,
                reason: CalloutSkip::NotNow
            },
            "champ select's view is not in yet"
        );
        assert_eq!(
            press(Phase::InProgress, None, &on, true, &pool, &memory, now, 0),
            ComebackPress::Game {
                line: "hi".into(),
                all: true
            }
        );
        let off = rule(&["hi"], false);
        assert_eq!(
            press(Phase::InProgress, None, &off, false, &pool, &memory, now, 0),
            ComebackPress::Skip {
                place: ComebackPlace::Team,
                reason: CalloutSkip::InGameOff
            }
        );
        assert_eq!(
            press(Phase::Lobby, None, &on, true, &pool, &memory, now, 0),
            ComebackPress::Skip {
                place: ComebackPlace::All,
                reason: CalloutSkip::NotNow
            }
        );
        assert_eq!(
            press(
                Phase::ChampSelect,
                Some(7),
                &on,
                false,
                &[],
                &memory,
                now,
                0
            ),
            ComebackPress::Skip {
                place: ComebackPlace::ChampSelect,
                reason: CalloutSkip::NoLines
            }
        );
        let recent = Memory {
            last: Some("hi".into()),
            sent_at: Some(now),
        };
        assert_eq!(
            press(
                Phase::ChampSelect,
                Some(7),
                &on,
                false,
                &pool,
                &recent,
                now + Duration::from_secs(2),
                0
            ),
            ComebackPress::Skip {
                place: ComebackPlace::ChampSelect,
                reason: CalloutSkip::TooSoon
            }
        );
        assert_eq!(
            press(
                Phase::ChampSelect,
                Some(7),
                &on,
                false,
                &pool,
                &recent,
                now + SPACING,
                0
            ),
            ComebackPress::ChampSelect {
                line: "hi".into(),
                game_id: 7
            },
            "three seconds after the last one finished, the one line again"
        );
    }

    #[test]
    fn the_library_joins_the_pool_only_switched_on_and_by_its_level() {
        let library = Library {
            mild: vec!["m".into()],
            strong: vec!["s".into()],
            ..Library::default()
        };
        let mut rule = rule(&["own"], false);
        assert_eq!(pool(&rule, Some(&library)), ["own"], "off by default");
        rule.library.enabled = true;
        assert_eq!(pool(&rule, Some(&library)), ["own", "m"]);
        rule.library.level = LibraryLevel::Strong;
        assert_eq!(pool(&rule, Some(&library)), ["own", "s"]);
        rule.library.level = LibraryLevel::Both;
        assert_eq!(pool(&rule, Some(&library)), ["own", "m", "s"]);
        assert_eq!(
            pool(&rule, None),
            ["own"],
            "switched on, nothing downloaded"
        );
        assert_eq!(library.status().mild, 1);
        assert_eq!(library.status().strong, 1);
    }

    #[test]
    fn an_edited_set_is_cleaned_cut_and_kept_once_a_line() {
        let long = "长".repeat(LIBRARY_LINE_LENGTH + 5);
        let lines = [
            "  泉水\n见 ".to_owned(),
            String::new(),
            "泉水 见".to_owned(),
            long,
            "下次一定".to_owned(),
        ];
        let kept = edited_lines(&lines);
        assert_eq!(
            kept[0], "泉水 见",
            "whitespace and controls fold to one space"
        );
        assert_eq!(kept.len(), 3, "the blank and the repeat go: {kept:?}");
        assert_eq!(
            kept[1].chars().count(),
            LIBRARY_LINE_LENGTH,
            "cut, not dropped"
        );
        assert_eq!(kept[2], "下次一定");
        let many: Vec<String> = (0..LIBRARY_LINE_LIMIT + 3).map(|n| n.to_string()).collect();
        assert_eq!(edited_lines(&many).len(), LIBRARY_LINE_LIMIT);
    }

    #[test]
    fn a_library_file_from_before_edits_reads_as_unedited() {
        let file = r#"{"source":"s","sha256":"h","fetchedAt":1,"mild":["m"],"strong":[]}"#;
        let library: Library = serde_json::from_str(file).unwrap();
        assert!(!library.edited);
        assert!(!library.status().edited);
        assert_eq!(library.sets().mild, ["m"]);
    }

    #[test]
    fn a_quick_phrase_names_the_seat_and_the_champion_and_drops_a_hidden_name() {
        let [zh] = default_phrases(Language::ZhCn) else {
            panic!("one Chinese phrase")
        };
        assert_eq!(
            phrase(zh, "5L", "龙王", ""),
            "5L大佬，小弟想玩下龙王，给您磕一个了"
        );
        assert_eq!(
            phrase("{seat}【{name}】换{champion}吗", "2L", "亚索", ""),
            "2L换亚索吗",
            "a hidden name takes its brackets along"
        );
        assert_eq!(
            phrase("{seat}【{name}】换{champion}吗", "2L", "亚索", "Ann"),
            "2L【Ann】换亚索吗"
        );
        let [en] = default_phrases(Language::En) else {
            panic!("one English phrase")
        };
        assert_eq!(
            phrase(en, "P5", "Aurelion Sol", ""),
            "P5, may I play Aurelion Sol? Thank you!"
        );
        let mut rule = ComebackSettings::default();
        assert_eq!(phrases(&rule, Language::En), [*en]);
        rule.phrases = vec!["mine".into()];
        assert_eq!(phrases(&rule, Language::ZhCn), ["mine"]);
    }

    #[test]
    fn a_line_becomes_one_message_without_control_characters_or_runs_of_space() {
        assert_eq!(clean_line("  你好\r\n世界\t !\u{0}  "), "你好 世界 !");
        let lines = vec![
            " a ".to_owned(),
            "\n".to_owned(),
            "b".repeat(250),
            "c".to_owned(),
        ];
        assert_eq!(
            clean_lines(&lines, 2, 200),
            ["a".to_owned(), "b".repeat(200)],
            "blanks dropped, cut to the length, at most the limit"
        );
    }

    /// The fixture `sqlite.rs` reads: harmless words shaped as the library is.
    const FIXTURE: &[u8] = include_bytes!("testdata/comeback-library.db");

    #[test]
    fn the_librarys_lines_are_cleaned_and_sorted_by_level() {
        let library = library_from(FIXTURE, 7).unwrap();
        assert_eq!(library.fetched_at, 7);
        assert_eq!(library.sha256, digest(FIXTURE));
        assert!(library.source.ends_with(LIBRARY_COMMIT));
        assert_eq!(&library.mild[..2], ["line 1", "line 3"]);
        assert_eq!(&library.strong[..2], ["line 2", "line 4"]);
        assert!(
            library.mild.contains(&"两行 文字".to_owned()),
            "newlines and tabs become spaces"
        );
        assert_eq!(
            library
                .mild
                .iter()
                .chain(&library.strong)
                .filter(|line| *line == "dup")
                .count(),
            1,
            "a repeated line once"
        );
        let every: Vec<&String> = library.mild.iter().chain(&library.strong).collect();
        assert!(
            every
                .iter()
                .all(|line| !line.is_empty() && line.chars().count() <= 200)
        );
        assert!(
            !every.iter().any(|line| *line == "other"),
            "another level is dropped"
        );
        // 60 + 60 numbered lines, the cleaned one, one `dup` and 13 tails.
        assert_eq!(library.mild.len(), 60 + 1 + 1);
        assert_eq!(library.strong.len(), 60 + 13);
        assert!(library_from(b"not a database", 0).is_err());
    }

    #[test]
    fn the_library_is_fetched_from_the_pinned_commit_on_every_mirror() {
        let urls = library_urls();
        assert_eq!(urls.len(), 4);
        for url in &urls {
            assert!(url.starts_with("https://"), "{url}");
            assert!(url.contains(LIBRARY_COMMIT), "{url}");
            assert!(url.ends_with("/db/data.db"), "{url}");
        }
        assert!(urls[3].starts_with("https://raw.githubusercontent.com/"));
    }
}
