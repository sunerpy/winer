//! The Comebacks page (反击): which line a press of its two shortcuts sends and where, the quick
//! phrases a teammate is sent from champ select's board, and the library a user may download (在线
//! 语录). Pure: the service keeps what went out last and the downloaded file
//! (`service/comeback.rs`); the shell types into the game.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{
    callout,
    settings::{ComebackSettings, Language, LibraryLevel},
    sqlite::{self, Value},
    view::{CalloutSkip, ComebackLibrary, ComebackPlace, Phase},
};

/// The least time between two comebacks that go out: presses one after another flood the chat,
/// and a flood is what gets an account reported.
pub const SPACING: Duration = Duration::from_secs(3);

/// What a press of a comeback's shortcut does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComebackPress {
    /// Champ select: this line goes to its chat.
    ChampSelect(String),
    /// The game runs and in-game sending is on: the shell types this line into the game's chat, to
    /// everyone with `all`.
    Game { line: String, all: bool },
    /// Nothing is sent.
    Skip {
        place: ComebackPlace,
        reason: CalloutSkip,
    },
}

/// What went out last: the next press neither repeats it nor follows it within [`SPACING`].
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

/// What a press does in `phase` under `rule`: in champ select a line of `pool` goes to its chat;
/// while the game runs (`InProgress`: not its loading screen) the shell types one, if in-game
/// sending is on. The line is [`pick`]ed with `random`; nothing goes out within [`SPACING`] of the
/// last one (`memory`, at `now`).
pub fn press(
    phase: Phase,
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
    match phase {
        Phase::ChampSelect => {}
        Phase::InProgress if !rule.in_game => return skip(CalloutSkip::InGameOff),
        Phase::InProgress => {}
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
    if phase == Phase::ChampSelect {
        ComebackPress::ChampSelect(line)
    } else {
        ComebackPress::Game { line, all }
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
        mild,
        strong,
    })
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

    #[test]
    fn a_press_goes_to_champ_select_or_the_game_or_says_why_not() {
        let now = Instant::now();
        let memory = Memory::default();
        let on = rule(&["hi"], true);
        let pool: Vec<&str> = pool(&on, None);
        assert_eq!(
            press(Phase::ChampSelect, &on, true, &pool, &memory, now, 0),
            ComebackPress::ChampSelect("hi".into())
        );
        assert_eq!(
            press(Phase::InProgress, &on, true, &pool, &memory, now, 0),
            ComebackPress::Game {
                line: "hi".into(),
                all: true
            }
        );
        let off = rule(&["hi"], false);
        assert_eq!(
            press(Phase::InProgress, &off, false, &pool, &memory, now, 0),
            ComebackPress::Skip {
                place: ComebackPlace::Team,
                reason: CalloutSkip::InGameOff
            }
        );
        assert_eq!(
            press(Phase::Lobby, &on, true, &pool, &memory, now, 0),
            ComebackPress::Skip {
                place: ComebackPlace::All,
                reason: CalloutSkip::NotNow
            }
        );
        assert_eq!(
            press(Phase::ChampSelect, &on, false, &[], &memory, now, 0),
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
                &on,
                false,
                &pool,
                &recent,
                now + SPACING,
                0
            ),
            ComebackPress::ChampSelect("hi".into()),
            "three seconds on, the one line again"
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
