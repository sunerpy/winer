//! User settings: the schema the window edits, and the file that keeps it.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    callout,
    view::{Position, Tier},
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub appearance: Appearance,
    pub general: General,
    pub automation: Automation,
    pub plugin: PluginSettings,
    pub profile: ProfileSettings,
    /// The build panel and where its numbers come from (`builds`).
    pub builds: BuildSettings,
    /// History: what the history lists show.
    pub history: HistorySettings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    pub theme: Theme,
    pub accent: Accent,
    pub density: Density,
    /// Base font size in px, 12–16.
    pub font_size: u8,
    pub reduce_motion: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Hextech,
            accent: Accent::Default,
            density: Density::Comfortable,
            font_size: 13,
            reduce_motion: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Light or dark, following the OS.
    System,
    Light,
    Dark,
    Graphite,
    /// Navy and gold, after the client itself.
    #[default]
    Hextech,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    /// The theme's own accent.
    #[default]
    Default,
    Gold,
    Blue,
    Teal,
    Green,
    Orange,
    Pink,
    Purple,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    /// Closing the window keeps winer in the tray.
    pub close_to_tray: bool,
    pub language: Language,
    /// Fetch what Hextech ARAM's augments do from ARAM.GG; without it they show name and icon only.
    pub augment_details: bool,
    /// The roast titles beside a grade (`rating::FormTitle` and the scoreboard's own).
    pub titles: bool,
    // Social.
    /// The global shortcut that shows and hides the window, in [`normalize_hotkey`]'s form; `None`
    /// turns it off. A file without the field gets the default; `null` keeps it off.
    pub hotkey: Option<String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: Language::ZhCn,
            augment_details: true,
            titles: true,
            hotkey: Some(DEFAULT_HOTKEY.to_owned()),
        }
    }
}

// ---- Social: the hotkey's combinations ----

/// Not bound by the game, the client or Windows by default. The first choice, Ctrl+Shift+W, was
/// already held by another program on the QA host (2026-10-06), as was Ctrl+Shift+Q; this one was
/// free there.
pub const DEFAULT_HOTKEY: &str = "Alt+Backquote";

/// The keys a combination can end in besides letters, digits and F1–F24, by the names
/// [`normalize_hotkey`] writes. Each is one the shell's shortcut parser accepts.
pub const HOTKEY_KEYS: &[&str] = &[
    "Space",
    "Insert",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "Backquote",
    "Minus",
    "Equal",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Semicolon",
    "Quote",
    "Comma",
    "Period",
    "Slash",
    "Num0",
    "Num1",
    "Num2",
    "Num3",
    "Num4",
    "Num5",
    "Num6",
    "Num7",
    "Num8",
    "Num9",
    "NumAdd",
    "NumSubtract",
    "NumMultiply",
    "NumDivide",
    "NumDecimal",
    "Pause",
    "ScrollLock",
    "PrintScreen",
];

/// A combination in one spelling, `Ctrl+Alt+Shift+Super+Key` (the modifiers that are there, in that
/// order), or `None` when it is not one. At least one of Ctrl, Alt and Super is required: a key
/// alone, or with Shift alone, is typed in chat and in the game. Accepts the browser's key codes
/// (`KeyW`, `Digit1`, `ArrowUp`, `Numpad1`) and `Win`, `Meta`, `Cmd` for Super.
pub fn normalize_hotkey(text: &str) -> Option<String> {
    let (mut ctrl, mut alt, mut shift, mut super_) = (false, false, false, false);
    let mut key = None;
    for token in text.split('+').map(str::trim) {
        if token.is_empty() || key.is_some() {
            return None;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "super" | "win" | "meta" | "cmd" | "command" => super_ = true,
            _ => key = Some(hotkey_key(token)?),
        }
    }
    let key = key?;
    if !(ctrl || alt || super_) {
        return None;
    }
    let mut parts: Vec<&str> = [
        (ctrl, "Ctrl"),
        (alt, "Alt"),
        (shift, "Shift"),
        (super_, "Super"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    parts.push(&key);
    Some(parts.join("+"))
}

/// One key's canonical name, from ours or the browser's (`KeyW`, `Digit1`, `ArrowUp`, `Numpad1`).
fn hotkey_key(token: &str) -> Option<String> {
    let upper = token.to_ascii_uppercase();
    let bare = upper
        .strip_prefix("KEY")
        .or_else(|| upper.strip_prefix("DIGIT"))
        .filter(|rest| rest.len() == 1)
        .unwrap_or(&upper);
    if bare.len() == 1 && bare.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(bare.to_owned());
    }
    if let Some(number) = bare.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&number)
        && !bare.starts_with("F0")
    {
        return Some(format!("F{number}"));
    }
    let named = if let Some(rest) = bare.strip_prefix("ARROW") {
        rest.to_owned()
    } else if let Some(rest) = bare.strip_prefix("NUMPAD") {
        format!("NUM{rest}")
    } else {
        bare.to_owned()
    };
    HOTKEY_KEYS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(&named))
        .map(|name| (*name).to_owned())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Automation {
    pub accept: AcceptRule,
    pub pick: PickRule,
    pub ban: BanRule,
    /// Return to the lobby after the post-game screen.
    pub play_again: bool,
    pub callout: CalloutRule,
    pub bench: BenchRule,
    /// The kinds of game each rule acts in; a switched-on rule does nothing elsewhere.
    pub scopes: Scopes,
    /// Runes and summoner spells, remembered per champion and mode and set up again (`loadout`).
    pub loadout: LoadoutRule,
    /// Experimental: write winer's item set for a champion once it is locked in (`loadout`).
    pub item_sets: bool,
}

impl Automation {
    /// Whether to go back to the lobby after a game of `mode`.
    pub fn plays_again(&self, mode: Option<Mode>) -> bool {
        self.play_again && self.scopes.covers(Scoped::PlayAgain, mode)
    }

    /// Whether runes and spells are remembered and set up again in a game of `mode`.
    pub fn restores_loadout(&self, mode: Option<Mode>) -> bool {
        self.loadout.enabled && self.scopes.covers(Scoped::Loadout, mode)
    }

    /// Whether winer's item set is written by itself in a game of `mode`.
    pub fn writes_item_sets(&self, mode: Option<Mode>) -> bool {
        self.item_sets && self.scopes.covers(Scoped::ItemSets, mode)
    }
}

/// A kind of game, as the automation tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Ranked Summoner's Rift.
    Ranked,
    /// Every other Summoner's Rift queue: draft, blind, quickplay, swiftplay.
    Normal,
    /// ARAM.
    Aram,
    /// Hextech ARAM.
    Hextech,
    /// Arena.
    Arena,
    /// The rest: URF, One for All, rotating modes, tutorials.
    Other,
}

impl Mode {
    pub const ALL: [Self; 6] = [
        Self::Ranked,
        Self::Normal,
        Self::Aram,
        Self::Hextech,
        Self::Arena,
        Self::Other,
    ];

    /// The kind of a queue of `game_mode` (`CLASSIC`, `ARAM`, `KIWI`, …), ranked or not.
    pub fn of(game_mode: &str, ranked: bool) -> Self {
        match game_mode.to_ascii_uppercase().as_str() {
            "CLASSIC" | "SWIFTPLAY" if ranked => Self::Ranked,
            "CLASSIC" | "SWIFTPLAY" => Self::Normal,
            "ARAM" => Self::Aram,
            "KIWI" => Self::Hextech,
            "CHERRY" => Self::Arena,
            _ => Self::Other,
        }
    }
}

/// One of the rules that can be limited to some kinds of game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scoped {
    Accept,
    Pick,
    Ban,
    Callout,
    Bench,
    PlayAgain,
    Loadout,
    ItemSets,
}

impl Scoped {
    /// The kinds of game the rule can act in at all: nobody picks or bans in ARAM, and only ARAM
    /// has a bench. Arena has no rune page and hands everyone the same two spells (the client's
    /// spell list offers `CHERRY` two, both fixed), so there is no loadout to set up there.
    pub fn applicable(self) -> &'static [Mode] {
        const PICKED: [Mode; 4] = [Mode::Ranked, Mode::Normal, Mode::Arena, Mode::Other];
        const LOADOUT: [Mode; 5] = [
            Mode::Ranked,
            Mode::Normal,
            Mode::Aram,
            Mode::Hextech,
            Mode::Other,
        ];
        const ITEM_SETS: [Mode; 5] = [
            Mode::Ranked,
            Mode::Normal,
            Mode::Aram,
            Mode::Hextech,
            Mode::Arena,
        ];
        match self {
            Self::Pick | Self::Ban => &PICKED,
            Self::Bench => &[Mode::Aram, Mode::Hextech],
            Self::Accept | Self::Callout | Self::PlayAgain => &Mode::ALL,
            Self::Loadout => &LOADOUT,
            Self::ItemSets => &ITEM_SETS,
        }
    }
}

/// Where each rule acts, by kind of game. Every rule starts out everywhere it can act.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Scopes {
    pub accept: Vec<Mode>,
    pub pick: Vec<Mode>,
    pub ban: Vec<Mode>,
    /// Where the callout goes out by itself; sending it by hand works everywhere.
    pub callout: Vec<Mode>,
    pub bench: Vec<Mode>,
    pub play_again: Vec<Mode>,
    pub loadout: Vec<Mode>,
    pub item_sets: Vec<Mode>,
}

impl Default for Scopes {
    fn default() -> Self {
        let all = |rule: Scoped| rule.applicable().to_vec();
        Self {
            accept: all(Scoped::Accept),
            pick: all(Scoped::Pick),
            ban: all(Scoped::Ban),
            callout: all(Scoped::Callout),
            bench: all(Scoped::Bench),
            play_again: all(Scoped::PlayAgain),
            loadout: all(Scoped::Loadout),
            item_sets: all(Scoped::ItemSets),
        }
    }
}

impl Scopes {
    fn modes(&self, rule: Scoped) -> &[Mode] {
        match rule {
            Scoped::Accept => &self.accept,
            Scoped::Pick => &self.pick,
            Scoped::Ban => &self.ban,
            Scoped::Callout => &self.callout,
            Scoped::Bench => &self.bench,
            Scoped::PlayAgain => &self.play_again,
            Scoped::Loadout => &self.loadout,
            Scoped::ItemSets => &self.item_sets,
        }
    }

    /// Whether `rule` acts in a game of `mode`. A game whose kind is not known counts only for a
    /// rule left on everywhere it can act.
    pub fn covers(&self, rule: Scoped, mode: Option<Mode>) -> bool {
        let modes = self.modes(rule);
        match mode {
            Some(mode) => modes.contains(&mode),
            None => rule.applicable().iter().all(|mode| modes.contains(mode)),
        }
    }

    /// Each list in canonical order, without a mode its rule cannot act in.
    fn normalize(&mut self) {
        for (rule, modes) in [
            (Scoped::Accept, &mut self.accept),
            (Scoped::Pick, &mut self.pick),
            (Scoped::Ban, &mut self.ban),
            (Scoped::Callout, &mut self.callout),
            (Scoped::Bench, &mut self.bench),
            (Scoped::PlayAgain, &mut self.play_again),
            (Scoped::Loadout, &mut self.loadout),
            (Scoped::ItemSets, &mut self.item_sets),
        ] {
            let chosen = std::mem::take(modes);
            *modes = rule
                .applicable()
                .iter()
                .copied()
                .filter(|mode| chosen.contains(mode))
                .collect();
        }
    }
}

/// Rank the team by recent form and say so in champ-select chat, one line per player.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct CalloutRule {
    /// Send once per champ select, as soon as every teammate's stats are in.
    pub auto: bool,
    pub audience: Audience,
    pub include_self: bool,
    /// Sent before the players' lines, as written; empty sends none.
    pub header: String,
    /// One line per player, with `{standing}`, `{seat}` (the place in champ select's list: `1L`,
    /// `P1`), `{name}`, `{champion}`, `{games}`, `{winRate}`, `{kda}`, `{score}`, `{title}` and
    /// `{quip}`; a value left blank (a hidden name) takes the brackets around it with it. Empty
    /// means the language's default (`callout::template`).
    pub template: String,
    /// How the team is split, and what the tiers are called.
    pub tiers: TierSet,
    /// The user's own tier names, best first, for `TierSet::Custom`: two to five, blanks skipped.
    pub custom_tiers: Vec<String>,
    // ---- The callout's shortcut, and the game's own chat (`callout::press`) ----
    /// The global shortcut that sends the callout, in [`normalize_hotkey`]'s form: in champ select
    /// the team's lines go to its chat, as 发送到队伍 sends them; while the game runs, with
    /// [`Self::in_game`] on, the lines [`Self::game_teams`] chooses are typed into the game's chat.
    /// `None`, the default, holds no combination, and the window's own combination is never taken.
    pub hotkey: Option<String>,
    /// While the game runs, the shortcut types the in-game lines into the game's team chat with
    /// synthesized key presses: the game's chat has no API. Off by default, since third-party input
    /// into the game may break its terms.
    pub in_game: bool,
    /// The line about the enemy to watch, with the placeholders of `template`; empty means the
    /// language's default (`callout::watch_template`).
    pub watch_template: String,
    /// The line about the enemy to go after; empty means `callout::target_template`.
    pub target_template: String,
    // ---- The callout in the game: the team's own lines, and whose lines are typed ----
    /// The line about each teammate in the game, with the placeholders of `template`; empty means
    /// the language's default (`callout::ally_template`), which names the champion.
    pub ally_template: String,
    /// Whose lines a press of the shortcut types in the game.
    pub game_teams: GameTeams,
    // ---- How each player's line reads ----
    /// The default line of each player (`callout::template`) and of each teammate in the game
    /// (`callout::ally_template`): one short line to compare, or emoji, title and quip as well.
    pub style: CalloutStyle,
}

impl Default for CalloutRule {
    fn default() -> Self {
        Self {
            auto: false,
            audience: Audience::Team,
            include_self: true,
            header: String::new(),
            template: String::new(),
            tiers: TierSet::default(),
            custom_tiers: Vec::new(),
            hotkey: None,
            in_game: false,
            watch_template: String::new(),
            target_template: String::new(),
            ally_template: String::new(),
            game_teams: GameTeams::default(),
            style: CalloutStyle::default(),
        }
    }
}

/// How players are graded, best first; the names per language live in `callout.rs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TierSet {
    /// 峡谷五档: a team of five ranked against itself, one tier each, from 峡谷通天代 to 纯正牛马.
    #[default]
    RiftFive,
    /// 峡谷八档: eight grades, S+ to F, by fixed score bands (`rating::FORM_GRADES`), the same for
    /// everyone.
    Grades,
    /// 马系宇宙: seven horses, the team ranked against itself.
    HorseUniverse,
    /// 上等马 / 中等马 / 下等马.
    Horses,
    /// 独角马 / 上等马 / 中等马 / 下等马 / 纯牛马.
    HorsesFive,
    /// 峡谷之王 / 大腿 / 正常发挥 / 混子 / 提款机.
    Rift,
    /// `CalloutRule::custom_tiers`.
    Custom,
}

impl TierSet {
    /// A score maps to its grade on its own; every other set ranks a team against itself.
    pub fn absolute(self) -> bool {
        self == Self::Grades
    }
}

impl CalloutRule {
    pub const MAX_TIERS: usize = 5;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    /// The team's champ-select chat.
    #[default]
    Team,
    /// Shown in this client only.
    Me,
}

/// How the callout writes each player when the user has not written their own line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CalloutStyle {
    /// One short line a player, the same fields in the same order: seat, tier, win rate, KDA,
    /// form and name, so the lines compare at a glance.
    Compact,
    /// The tier's emoji in champ select, then the tier, the player, their numbers, the title and
    /// the tier's quip.
    #[default]
    Rich,
}

/// Whose lines the callout's shortcut types into the game's chat (`callout::typed`), at most
/// `callout::GAME_LINE_LIMIT` a press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GameTeams {
    /// The enemy to watch and the one to go after: what one speaks up in a game for.
    #[default]
    Enemies,
    /// Every rated teammate, as in champ select, by champion.
    Allies,
    /// The enemy lines, then the team's.
    Both,
}

/// ARAM: take a champion off the shared bench as soon as one higher on the wishlist appears.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct BenchRule {
    pub enabled: bool,
    /// Best first.
    pub champions: Vec<i64>,
}

impl BenchRule {
    pub const LIMIT: usize = 20;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct AcceptRule {
    pub enabled: bool,
    /// Wait this long before accepting, so a match found by accident can still be declined.
    pub delay_ms: u32,
}

impl Default for AcceptRule {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: 1500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct PickRule {
    pub enabled: bool,
    /// Lock the pick in; otherwise only hover it and leave the lock to the user.
    pub lock_in: bool,
    /// Show the first choice as an intent during the planning phase.
    pub declare_intent: bool,
    pub champions: ChampionPool,
    /// Sent to a lane the player did not ask for (补位), pick from that lane's own list only, never
    /// from `any`: a list for any lane was chosen for the lanes the player plays.
    pub skip_when_filled: bool,
}

impl Default for PickRule {
    fn default() -> Self {
        Self {
            enabled: false,
            lock_in: true,
            declare_intent: true,
            champions: ChampionPool::default(),
            skip_when_filled: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct BanRule {
    pub enabled: bool,
    pub champions: ChampionPool,
}

/// Champions in order of preference, per assigned position, with `any` as the fallback.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampionPool {
    pub any: Vec<i64>,
    pub top: Vec<i64>,
    pub jungle: Vec<i64>,
    pub middle: Vec<i64>,
    pub bottom: Vec<i64>,
    pub utility: Vec<i64>,
}

impl ChampionPool {
    pub const LIMIT: usize = 10;

    pub fn list(&self, position: Position) -> &[i64] {
        match position {
            Position::Top => &self.top,
            Position::Jungle => &self.jungle,
            Position::Middle => &self.middle,
            Position::Bottom => &self.bottom,
            Position::Utility => &self.utility,
        }
    }

    /// The position's own list, then `any`, without repeats.
    /// The position's own list alone, without `any`: for a player sent to a lane they did not ask
    /// for.
    pub fn own_candidates(&self, position: Option<Position>) -> Vec<i64> {
        position
            .map(|position| self.list(position).to_vec())
            .unwrap_or_default()
    }

    pub fn candidates(&self, position: Option<Position>) -> Vec<i64> {
        let own = position
            .map(|position| self.list(position))
            .unwrap_or_default();
        let mut seen = Vec::new();
        for &id in own.iter().chain(&self.any) {
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
        seen
    }

    fn normalize(&mut self) {
        for list in [
            &mut self.any,
            &mut self.top,
            &mut self.jungle,
            &mut self.middle,
            &mut self.bottom,
            &mut self.utility,
        ] {
            let mut kept = Vec::with_capacity(list.len());
            for &id in list.iter() {
                if id > 0 && !kept.contains(&id) && kept.len() < Self::LIMIT {
                    kept.push(id);
                }
            }
            *list = kept;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginSettings {
    /// Whenever a client connects: link the loader winer ships unless one is already linked,
    /// install the plugin, and keep both current. Off, winer only keeps an installed plugin current.
    pub auto: bool,
    /// The teammate panel in champ select.
    pub team_panel: bool,
    /// Hide the esports pop-ups, and put a short note in place of the home page's news and events
    /// hub; the note brings the hub back until the client restarts.
    pub hide_promotions: bool,
    /// In the client's own champ select, a click on an ARAM bench champion swaps at once: the
    /// plugin lifts the cooldown and winer carries the swap out.
    pub bench_no_cooldown: bool,
    /// Pengu Loader's directory, when it cannot be found from the client.
    pub loader_dir: Option<String>,
    // Social.
    /// In the client's friends list: the mode and running time of a friend's game, and one colour
    /// for the friends playing together.
    pub friend_status: bool,
    /// In the client's lobby: each member's recent form above their banner, and a click that opens
    /// their history in winer.
    pub lobby_panel: bool,
    // The history panel in the client.
    /// A click on a player in the client's lobby or champ select shows their latest games in a
    /// panel over the client page; off, the click opens their history in winer's window.
    pub history_in_client: bool,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            auto: true,
            team_panel: true,
            hide_promotions: false,
            bench_no_cooldown: true,
            loader_dir: None,
            friend_status: true,
            lobby_panel: true,
            // The history panel in the client.
            history_in_client: true,
        }
    }
}

/// What friends see of the player that winer keeps for them: the rank in the friends list and the
/// status put back after the client resets it. Both act on the client by themselves, so both start
/// off. Neither is scoped by mode: the chat presence is the same in and out of every game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileSettings {
    pub rank_disguise: RankDisguise,
    pub presence: PresenceRule,
}

/// The rank friends see in the friends list and on the hover card instead of the real one. Only the
/// chat presence changes: the real rank, matchmaking and the client's own profile do not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct RankDisguise {
    pub enabled: bool,
    pub queue: DisguiseQueue,
    pub tier: Tier,
    /// Not shown from Master up, which have no divisions.
    pub division: Division,
}

impl Default for RankDisguise {
    fn default() -> Self {
        Self {
            enabled: false,
            queue: DisguiseQueue::Solo,
            tier: Tier::Diamond,
            division: Division::One,
        }
    }
}

// Runes, spells, builds and item sets (`loadout`, `builds`).

/// In champ select, set up the runes and summoner spells last played on the champion in this kind
/// of game, once the champion is locked in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct LoadoutRule {
    pub enabled: bool,
    /// With nothing remembered for the champion, use the client's own recommended page.
    pub recommended: bool,
}

impl Default for LoadoutRule {
    fn default() -> Self {
        Self {
            enabled: false,
            recommended: true,
        }
    }
}

/// The queue a disguised rank claims to be from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DisguiseQueue {
    /// Ranked solo/duo, `RANKED_SOLO_5x5`.
    #[default]
    Solo,
    /// Ranked flex, `RANKED_FLEX_SR`.
    Flex,
}

/// A division within a tier, as the client writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Division {
    #[default]
    #[serde(rename = "I")]
    One,
    #[serde(rename = "II")]
    Two,
    #[serde(rename = "III")]
    Three,
    #[serde(rename = "IV")]
    Four,
}

/// The chat status winer puts back when the client resets it: on connecting to a client and after
/// each game (`profile::Keeper`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct PresenceRule {
    pub remember: bool,
    /// `chat`, `away`, `mobile` or `offline`: the states the client takes from winer.
    pub availability: String,
    /// Put back as well when set; `None` leaves the client's own.
    pub status_message: Option<String>,
    /// While the mobile state is chosen and no message of the user's own is set, the status
    /// message says 手机在线: the Tencent client names that state 在线分组, while friends read a
    /// status message as written, in quotation marks (`docs/platform-notes.md`). Another state
    /// takes it away again. Off by default.
    pub mobile_message: bool,
}

impl Default for PresenceRule {
    fn default() -> Self {
        Self {
            remember: false,
            availability: "chat".into(),
            status_message: None,
            mobile_message: false,
        }
    }
}

/// The build panel: what players take on a champion, from public statistics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildSettings {
    /// Off, the panel is hidden and nothing is fetched.
    pub enabled: bool,
    /// Where Summoner's Rift numbers come from.
    pub rift_source: RiftSource,
    /// Where ARAM's numbers come from, or nowhere.
    pub aram_source: ModeSource,
    /// Where Arena's numbers come from, or nowhere.
    pub arena_source: ModeSource,
    /// Hextech ARAM asks ARAM.GG when Tencent has no numbers; off, Tencent's are the only ones.
    pub hextech_fallback: bool,
    /// Champ select on the Rift shows champions worth considering (`recommend`); shown only.
    pub recommend: bool,
}

impl Default for BuildSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            rift_source: RiftSource::Tencent,
            aram_source: ModeSource::OpGg,
            arena_source: ModeSource::OpGg,
            hextech_fallback: true,
            recommend: true,
        }
    }
}

impl PresenceRule {
    /// The availabilities winer sets. `dnd` is the client's own during a game and a request for it
    /// is ignored (`docs/platform-notes.md`).
    pub const AVAILABILITIES: [&str; 4] = ["chat", "away", "mobile", "offline"];
    /// The client's own limit on a status message is longer; the window's field stops here.
    pub const MESSAGE_LIMIT: usize = 120;
    /// What the status message says for the mobile state under [`Self::mobile_message`], in the
    /// Tencent client's language whatever the window's: it is there for that client's friends.
    pub const MOBILE_MESSAGE: &str = "手机在线";

    /// The status message this rule puts back over `current`, the client's: the user's own when one
    /// is kept; for the mobile state with its message on and no own message, the mobile message,
    /// though only where the client shows none while nothing is kept (`None` leaves the client's
    /// own); otherwise as kept.
    pub fn kept_message(&self, current: &str) -> Option<String> {
        let own = self
            .status_message
            .as_deref()
            .is_some_and(|message| !message.trim().is_empty());
        if !own && self.mobile_message && self.availability == "mobile" {
            let mobile = Self::MOBILE_MESSAGE.to_owned();
            return match self.status_message {
                Some(_) => Some(mobile),
                None => current.trim().is_empty().then_some(mobile),
            };
        }
        self.status_message.clone()
    }
}

// ---- History: the history lists ----

/// What the history lists show. Form leaves custom games out whatever these say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct HistorySettings {
    /// Custom games stay out of the lists. On by default: practice and lobbies among friends are
    /// not the games a history is opened for, and they push those down the first page.
    pub hide_custom_games: bool,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            hide_custom_games: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RiftSource {
    /// The Tencent shards' own statistics (腾讯 101), the user's own server.
    #[default]
    Tencent,
    /// OP.GG's global statistics.
    OpGg,
}

/// Where a mode with one public source (ARAM, Arena) gets its numbers: that source, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ModeSource {
    /// OP.GG's global statistics.
    #[default]
    OpGg,
    /// Nothing is fetched for the mode.
    Off,
}

impl Settings {
    /// Clamps every value into range. Applied to anything read from disk or sent by the window.
    pub fn normalized(mut self) -> Self {
        let appearance = &mut self.appearance;
        appearance.font_size = appearance.font_size.clamp(12, 16);
        let automation = &mut self.automation;
        automation.accept.delay_ms = automation.accept.delay_ms.min(10_000);
        automation.pick.champions.normalize();
        automation.ban.champions.normalize();
        let callout = &mut automation.callout;
        callout.header = clip(&callout.header, 200);
        callout.template = clip(&callout.template, 200);
        callout.custom_tiers.truncate(CalloutRule::MAX_TIERS);
        for name in &mut callout.custom_tiers {
            *name = clip(name, 16);
        }
        let mut wishlist = Vec::with_capacity(automation.bench.champions.len());
        for &id in &automation.bench.champions {
            if id > 0 && !wishlist.contains(&id) && wishlist.len() < BenchRule::LIMIT {
                wishlist.push(id);
            }
        }
        automation.bench.champions = wishlist;
        automation.scopes.normalize();
        if let Some(dir) = &self.plugin.loader_dir {
            let dir = dir.trim();
            self.plugin.loader_dir = (!dir.is_empty()).then(|| dir.to_owned());
        }
        let presence = &mut self.profile.presence;
        if !PresenceRule::AVAILABILITIES.contains(&presence.availability.as_str()) {
            presence.availability = PresenceRule::default().availability;
        }
        if let Some(message) = &presence.status_message {
            presence.status_message = Some(clip(message, PresenceRule::MESSAGE_LIMIT));
        }
        // While the mobile message is on, that message is winer's to put up and take down, never one
        // the user keeps: kept, it would come back after the state had moved on.
        if presence.mobile_message
            && presence.status_message.as_deref() == Some(PresenceRule::MOBILE_MESSAGE)
        {
            presence.status_message = None;
        }
        // A combination that is not one turns the shortcut off rather than registering nonsense.
        self.general.hotkey = self.general.hotkey.as_deref().and_then(normalize_hotkey);
        // The callout's shortcut is spelled the same way, and never takes the window's combination:
        // the system holds one combination for one shortcut, and the window's was there first.
        let callout = &mut self.automation.callout;
        callout.hotkey = callout
            .hotkey
            .as_deref()
            .and_then(normalize_hotkey)
            .filter(|combination| self.general.hotkey.as_ref() != Some(combination));
        callout.watch_template = clip(&callout.watch_template, 200);
        callout.target_template = clip(&callout.target_template, 200);
        callout.ally_template = clip(&callout.ally_template, 200);
        self
    }

    /// Brings a file an older winer wrote up to date. Up to 0.0.2 the default callout line named
    /// the champion, up to 0.0.3 it called the form score 评分 and ran the name into the numbers,
    /// the in-game lines first named the champion and the player, and up to 0.0.4 the Chinese
    /// lines left names bare, for the chat's filter to read together with the tier as one word; a
    /// template saved as exactly one of those texts would have kept it for good: it becomes the
    /// current default of the same style and language. A template the user changed stays as
    /// written.
    fn migrated(mut self) -> Self {
        let language = self.general.language;
        let callout = &mut self.automation.callout;
        let style = callout.style;
        // The lines from before styles were the rich style's (`callout::former_default`).
        migrate(
            &mut callout.template,
            callout::former_default,
            |(style, language)| callout::template(style, language),
            (style, language),
        );
        migrate(
            &mut callout.ally_template,
            callout::former_ally_default,
            |(style, language)| callout::ally_template(style, language),
            (style, language),
        );
        migrate(
            &mut callout.watch_template,
            callout::former_watch_default,
            callout::watch_template,
            language,
        );
        migrate(
            &mut callout.target_template,
            callout::former_target_default,
            callout::target_template,
            language,
        );
        self
    }
}

/// `template` replaced, when `former` finds it a former default line, by the current default of
/// the same kind (a language, or a style and a language): blank, the default itself, where that is
/// the one the callout would use `now`; written out where it is not (the other language's, or the
/// other style's), so the line reads as it did.
fn migrate<K: Copy + PartialEq>(
    template: &mut String,
    former: fn(&str) -> Option<K>,
    current: fn(K) -> &'static str,
    now: K,
) {
    if let Some(written) = former(template) {
        *template = if written == now {
            String::new()
        } else {
            current(written).to_owned()
        };
    }
}

/// Trimmed and cut to `limit` characters (not bytes: names are usually CJK).
fn clip(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

/// The settings file. Writes are atomic: a crash leaves either the old file or the new one.
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Reads `path`, falling back to defaults when it is missing. A file that does not parse is
    /// moved aside rather than overwritten, so a bad edit is never silently lost.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let settings = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|error| {
                let aside = path.with_extension("json.invalid");
                tracing::warn!(%error, aside = %aside.display(), "settings file is invalid, using defaults");
                let _ = fs::rename(&path, aside);
                Settings::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                tracing::warn!(%error, "settings file is unreadable, using defaults");
                Settings::default()
            }
        };
        Self {
            path,
            current: Mutex::new(settings.normalized().migrated()),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().expect("settings lock").clone()
    }

    pub fn set(&self, settings: Settings) -> io::Result<Settings> {
        let settings = settings.normalized();
        let mut current = self.current.lock().expect("settings lock");
        write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(&settings).expect("settings serialize"),
        )?;
        *current = settings.clone();
        Ok(settings)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Temp file, `sync_all`, then `rename`. On Windows `rename` replaces an open destination where
/// `MoveFileExW` was measured failing (`docs/platform-notes.md`).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let result = (|| {
        let mut file = fs::File::create(&temporary)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_gives_defaults_and_a_saved_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let store = SettingsStore::open(&path);
        assert_eq!(store.get(), Settings::default());

        let mut changed = store.get();
        changed.automation.accept.enabled = true;
        changed.appearance.theme = Theme::Light;
        store.set(changed.clone()).unwrap();
        assert_eq!(SettingsStore::open(&path).get(), changed);
    }

    #[test]
    fn an_invalid_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(SettingsStore::open(&path).get(), Settings::default());
        assert!(path.with_extension("json.invalid").exists());
    }

    /// 0.0.2's default lines, as its window could have saved them: they named the champion.
    const ZH_0_0_2: &str =
        "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}";
    const EN_0_0_2: &str = "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}";
    /// 0.0.3's: the seat, but the form score called 评分 and the name run into the numbers.
    const ZH_0_0_3: &str =
        "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}";
    const EN_0_0_3: &str = "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}";

    #[test]
    fn a_template_saved_as_the_former_default_becomes_the_new_one_when_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let loaded = |language: &str, template: &str| {
            let file = serde_json::json!({
                "general": { "language": language },
                "automation": { "callout": { "template": template, "header": "冲" } },
            });
            fs::write(&path, file.to_string()).unwrap();
            SettingsStore::open(&path).get().automation.callout
        };

        let callout = loaded("zh-CN", ZH_0_0_2);
        assert_eq!(callout.template, "", "the default, which names the seat");
        assert_eq!(callout.header, "冲", "nothing else moves");
        assert_eq!(loaded("en", EN_0_0_2).template, "");
        assert_eq!(loaded("zh-CN", &format!("  {ZH_0_0_2} ")).template, "");
        assert_eq!(
            loaded("zh-CN", EN_0_0_2).template,
            callout::template(CalloutStyle::Rich, Language::En),
            "an English line under the Chinese window stays English"
        );
        assert_eq!(loaded("zh-CN", ZH_0_0_3).template, "", "0.0.3's line too");
        assert_eq!(loaded("en", EN_0_0_3).template, "");
        assert_eq!(
            loaded("en", ZH_0_0_3).template,
            callout::template(CalloutStyle::Rich, Language::ZhCn)
        );

        for own in [
            "{standing}：{champion} {name}",
            "{standing}：{champion} {name} 近{games}场胜率{winRate}",
            "{name} 玩 {champion}",
        ] {
            assert_eq!(loaded("zh-CN", own).template, own, "changed by the user");
        }
    }

    /// The in-game lines' first defaults, as a window could have saved them: the champion and the
    /// player's name.
    const ZH_WATCH: &str =
        "小心 {champion} {name}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}";
    const EN_WATCH: &str =
        "Watch {champion} ({name}): {standing}, {winRate} in {games} games, KDA {kda}{title}";
    const ZH_TARGET: &str =
        "对面 {champion} {name}：{standing}，近{games}场胜率{winRate}，可以多抓";
    const EN_TARGET: &str = "Go after {champion} ({name}): {standing}, {winRate} in {games} games";

    #[test]
    fn an_in_game_line_saved_as_its_former_default_becomes_the_new_one_when_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let loaded = |language: &str, watch: &str, target: &str| {
            let file = serde_json::json!({
                "general": { "language": language },
                "automation": { "callout": {
                    "watchTemplate": watch, "targetTemplate": target, "allyTemplate": "{champion}", "inGame": true
                } },
            });
            fs::write(&path, file.to_string()).unwrap();
            SettingsStore::open(&path).get().automation.callout
        };

        let callout = loaded("zh-CN", ZH_WATCH, &format!(" {ZH_TARGET}  "));
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            ("", ""),
            "the defaults, which name the champion alone"
        );
        assert!(
            callout.in_game && callout.ally_template == "{champion}",
            "nothing else moves"
        );
        let callout = loaded("en", EN_WATCH, EN_TARGET);
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            ("", "")
        );
        let callout = loaded("zh-CN", EN_WATCH, EN_TARGET);
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            (
                callout::watch_template(Language::En),
                callout::target_template(Language::En)
            ),
            "English lines under the Chinese window stay English"
        );
        let callout = loaded("zh-CN", ZH_TARGET, ZH_WATCH);
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            (ZH_TARGET, ZH_WATCH),
            "a line moved to the other's place was the user's doing"
        );
        let own = "小心 {champion} {name}";
        assert_eq!(loaded("zh-CN", own, "").watch_template, own);
    }

    /// 0.0.4's default lines, as its window could have saved them: the Chinese ones put names and
    /// champions after a space or a colon, and the English compact one ran the champion into the
    /// tier.
    const ZH_COMPACT_0_0_4: &str =
        "{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜{name}";
    const ZH_RICH_0_0_4: &str = "{emoji}{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}";
    const ZH_ALLY_COMPACT_0_0_4: &str =
        "{standing} {champion}｜胜率{winRate}｜KDA {kda}｜战力{score}";
    const ZH_ALLY_RICH_0_0_4: &str =
        "{standing}：{champion}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}";
    const EN_ALLY_COMPACT_0_0_4: &str =
        "{standing} {champion} | {winRate} | KDA {kda} | form {score}";
    const ZH_WATCH_0_0_4: &str =
        "小心 {champion}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}";
    const ZH_TARGET_0_0_4: &str = "对面 {champion}：{standing}，近{games}场胜率{winRate}，可以多抓";

    #[test]
    fn a_line_saved_as_a_0_0_4_default_becomes_the_same_styles_new_one_when_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let loaded = |language: &str, style: &str, mut callout: serde_json::Value| {
            callout["style"] = style.into();
            let file = serde_json::json!({
                "general": { "language": language },
                "automation": { "callout": callout },
            });
            fs::write(&path, file.to_string()).unwrap();
            SettingsStore::open(&path).get().automation.callout
        };
        let lines = |template: &str, ally: &str| serde_json::json!({ "template": template, "allyTemplate": ally, "header": "冲" });
        let (compact, rich) = (CalloutStyle::Compact, CalloutStyle::Rich);

        // Saved in the style and language the window uses: the default itself, now in brackets.
        let callout = loaded(
            "zh-CN",
            "compact",
            lines(ZH_COMPACT_0_0_4, ZH_ALLY_COMPACT_0_0_4),
        );
        assert_eq!(
            (callout.template.as_str(), callout.ally_template.as_str()),
            ("", "")
        );
        assert_eq!(
            (callout.style, callout.header.as_str()),
            (compact, "冲"),
            "nothing else moves"
        );
        let callout = loaded("zh-CN", "rich", lines(ZH_RICH_0_0_4, ZH_ALLY_RICH_0_0_4));
        assert_eq!(
            (callout.template.as_str(), callout.ally_template.as_str()),
            ("", "")
        );
        let callout = loaded("en", "compact", lines("", EN_ALLY_COMPACT_0_0_4));
        assert_eq!(callout.ally_template, "");

        // The other style's line, or the other language's, was the user's choice of line: it is
        // written out, in brackets, so the callout reads as it did.
        let callout = loaded(
            "zh-CN",
            "rich",
            lines(ZH_COMPACT_0_0_4, ZH_ALLY_COMPACT_0_0_4),
        );
        assert_eq!(
            (callout.template.as_str(), callout.ally_template.as_str()),
            (
                callout::template(compact, Language::ZhCn),
                callout::ally_template(compact, Language::ZhCn)
            )
        );
        let callout = loaded("zh-CN", "compact", lines(ZH_RICH_0_0_4, ""));
        assert_eq!(callout.template, callout::template(rich, Language::ZhCn));
        let callout = loaded("en", "rich", lines(ZH_RICH_0_0_4, ZH_ALLY_RICH_0_0_4));
        assert_eq!(
            (callout.template.as_str(), callout.ally_template.as_str()),
            (
                callout::template(rich, Language::ZhCn),
                callout::ally_template(rich, Language::ZhCn)
            ),
            "Chinese lines under the English window stay Chinese"
        );
        let callout = loaded("zh-CN", "compact", lines("", EN_ALLY_COMPACT_0_0_4));
        assert_eq!(
            callout.ally_template,
            callout::ally_template(compact, Language::En)
        );
        // A line from before styles was the rich one: under the compact style it stays rich.
        let callout = loaded("zh-CN", "compact", lines(ZH_0_0_3, ""));
        assert_eq!(callout.template, callout::template(rich, Language::ZhCn));

        // The enemy lines, which have no style.
        let callout = loaded(
            "zh-CN",
            "compact",
            serde_json::json!({ "watchTemplate": ZH_WATCH_0_0_4, "targetTemplate": ZH_TARGET_0_0_4 }),
        );
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            ("", "")
        );
        let callout = loaded(
            "en",
            "rich",
            serde_json::json!({ "watchTemplate": ZH_WATCH_0_0_4, "targetTemplate": ZH_TARGET_0_0_4 }),
        );
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            (
                callout::watch_template(Language::ZhCn),
                callout::target_template(Language::ZhCn)
            )
        );
        assert!(
            callout.watch_template.contains("小心【{champion}】"),
            "{}",
            callout.watch_template
        );

        // A line the user changed, or put in the other line's place, stays as written.
        for own in [
            "{seat} {standing}｜胜率{winRate}｜{name}",
            "{standing} {champion}",
            ZH_WATCH_0_0_4,
        ] {
            let callout = loaded("zh-CN", "compact", lines(own, own));
            assert_eq!(
                (callout.template.as_str(), callout.ally_template.as_str()),
                (own, own)
            );
        }
    }

    #[test]
    fn a_file_from_before_the_teams_own_lines_in_game_types_the_enemy_lines() {
        let old: Settings = serde_json::from_str(
            r#"{"automation":{"callout":{"inGame":true,"watchTemplate":"注意 {champion}"}}}"#,
        )
        .unwrap();
        let callout = old.normalized().automation.callout;
        assert_eq!(callout.game_teams, GameTeams::Enemies);
        assert_eq!(callout.ally_template, "", "the default, by champion");
        assert!(callout.in_game && callout.watch_template == "注意 {champion}");

        let both: Settings =
            serde_json::from_str(r#"{"automation":{"callout":{"gameTeams":"both"}}}"#).unwrap();
        assert_eq!(both.automation.callout.game_teams, GameTeams::Both);
        assert_eq!(
            serde_json::to_value(GameTeams::Allies).unwrap(),
            serde_json::json!("allies")
        );
        let mut long = Settings::default();
        long.automation.callout.ally_template = format!(" {} ", "队".repeat(300));
        assert_eq!(
            long.normalized()
                .automation
                .callout
                .ally_template
                .chars()
                .count(),
            200
        );
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let settings: Settings =
            serde_json::from_str(r#"{"automation":{"accept":{"enabled":true}},"future":1}"#)
                .unwrap();
        assert!(settings.automation.accept.enabled);
        assert_eq!(settings.automation.accept.delay_ms, 1500);
    }

    #[test]
    fn normalizing_clamps_and_deduplicates() {
        let mut settings = Settings::default();
        settings.appearance.font_size = 40;
        settings.automation.accept.delay_ms = 60_000;
        settings.automation.pick.champions.any = vec![1, 1, 0, -3, 2];
        settings.automation.ban.champions.top = (1..30).collect();
        settings.plugin.loader_dir = Some("   ".into());
        settings.automation.callout.template = format!("  {}  ", "马".repeat(300));
        settings.automation.callout.custom_tiers = vec![
            "  独角马 ".into(),
            String::new(),
            "x".repeat(40),
            "a".into(),
            "b".into(),
            "c".into(),
        ];
        settings.automation.bench.champions = vec![3, 3, 0, 5];
        let settings = settings.normalized();
        assert_eq!(settings.appearance.font_size, 16);
        assert_eq!(settings.automation.accept.delay_ms, 10_000);
        assert_eq!(settings.automation.pick.champions.any, vec![1, 2]);
        assert_eq!(
            settings.automation.ban.champions.top.len(),
            ChampionPool::LIMIT
        );
        assert_eq!(settings.plugin.loader_dir, None);
        assert_eq!(settings.automation.callout.template.chars().count(), 200);
        let tiers = &settings.automation.callout.custom_tiers;
        assert_eq!(
            (tiers.len(), tiers[0].as_str(), tiers[2].len()),
            (5, "独角马", 16)
        );
        assert_eq!(settings.automation.bench.champions, vec![3, 5]);
    }

    #[test]
    fn queues_are_told_apart_by_mode_and_ranking() {
        assert_eq!(Mode::of("CLASSIC", true), Mode::Ranked);
        assert_eq!(Mode::of("CLASSIC", false), Mode::Normal);
        assert_eq!(Mode::of("SWIFTPLAY", false), Mode::Normal);
        assert_eq!(Mode::of("aram", false), Mode::Aram);
        assert_eq!(Mode::of("KIWI", false), Mode::Hextech);
        assert_eq!(Mode::of("CHERRY", false), Mode::Arena);
        assert_eq!(Mode::of("URF", false), Mode::Other);
        assert_eq!(Mode::of("", false), Mode::Other);
    }

    #[test]
    fn a_rule_acts_only_in_the_modes_it_is_scoped_to() {
        let scopes = Scopes::default();
        assert!(scopes.covers(Scoped::Pick, Some(Mode::Ranked)));
        assert!(
            !scopes.covers(Scoped::Pick, Some(Mode::Aram)),
            "nobody picks in ARAM"
        );
        assert!(scopes.covers(Scoped::Bench, Some(Mode::Hextech)));
        assert!(!scopes.covers(Scoped::Bench, Some(Mode::Normal)));
        assert!(
            scopes.covers(Scoped::Accept, None),
            "on everywhere, so also where unknown"
        );

        let narrowed = Scopes {
            accept: vec![Mode::Ranked],
            ..Scopes::default()
        };
        assert!(narrowed.covers(Scoped::Accept, Some(Mode::Ranked)));
        assert!(!narrowed.covers(Scoped::Accept, Some(Mode::Hextech)));
        assert!(
            !narrowed.covers(Scoped::Accept, None),
            "unknown is not ranked"
        );
    }

    #[test]
    fn scopes_drop_modes_a_rule_cannot_act_in_and_keep_their_order() {
        let mut settings = Settings::default();
        settings.automation.scopes.pick =
            vec![Mode::Aram, Mode::Normal, Mode::Ranked, Mode::Normal];
        settings.automation.scopes.bench = vec![Mode::Ranked, Mode::Hextech];
        let scopes = settings.normalized().automation.scopes;
        assert_eq!(scopes.pick, vec![Mode::Ranked, Mode::Normal]);
        assert_eq!(scopes.bench, vec![Mode::Hextech]);
        // A file from before scopes existed acts everywhere, as it did.
        let old: Settings =
            serde_json::from_str(r#"{"automation":{"pick":{"enabled":true}}}"#).unwrap();
        assert_eq!(old.automation.scopes, Scopes::default());
    }

    #[test]
    fn what_winer_keeps_in_the_presence_starts_off_and_stays_in_range() {
        let settings = Settings::default();
        assert!(!settings.profile.rank_disguise.enabled);
        assert!(!settings.profile.presence.remember);
        // A file from before the profile tools has them off.
        let old: Settings = serde_json::from_str(r#"{"general":{"titles":false}}"#).unwrap();
        assert_eq!(old.profile, ProfileSettings::default());

        let mut settings = Settings::default();
        settings.profile.presence = PresenceRule {
            remember: true,
            availability: "dnd".into(),
            status_message: Some(format!("  {}  ", "签".repeat(200))),
            mobile_message: false,
        };
        let presence = settings.normalized().profile.presence;
        assert_eq!(presence.availability, "chat", "the client sets dnd itself");
        assert_eq!(
            presence
                .status_message
                .map(|message| message.chars().count()),
            Some(PresenceRule::MESSAGE_LIMIT)
        );
        let mut mobile = Settings::default();
        mobile.profile.presence.availability = "mobile".into();
        assert_eq!(mobile.normalized().profile.presence.availability, "mobile");
    }

    #[test]
    fn the_mobile_message_starts_off_and_is_never_kept_as_the_users_own() {
        assert!(!Settings::default().profile.presence.mobile_message);
        // A file from before the switch has it off.
        let old: Settings = serde_json::from_str(
            r#"{"profile":{"presence":{"remember":true,"availability":"mobile","statusMessage":null}}}"#,
        )
        .unwrap();
        assert!(!old.profile.presence.mobile_message);

        let mut settings = Settings::default();
        settings.profile.presence = PresenceRule {
            remember: true,
            availability: "mobile".into(),
            status_message: Some(PresenceRule::MOBILE_MESSAGE.into()),
            mobile_message: true,
        };
        assert_eq!(
            settings
                .clone()
                .normalized()
                .profile
                .presence
                .status_message,
            None,
            "the message on screen when remembering was winer's"
        );
        settings.profile.presence.mobile_message = false;
        assert_eq!(
            settings
                .normalized()
                .profile
                .presence
                .status_message
                .as_deref(),
            Some(PresenceRule::MOBILE_MESSAGE),
            "with the switch off it is the user's"
        );
    }

    #[test]
    fn the_kept_message_is_the_users_own_before_the_mobile_states() {
        let rule = |status_message: Option<&str>, mobile_message: bool| PresenceRule {
            remember: true,
            availability: "mobile".into(),
            status_message: status_message.map(str::to_owned),
            mobile_message,
        };
        let mobile = Some(PresenceRule::MOBILE_MESSAGE.to_owned());
        assert_eq!(
            rule(Some("下班了"), true).kept_message(""),
            Some("下班了".into())
        );
        assert_eq!(rule(None, true).kept_message(""), mobile);
        assert_eq!(
            rule(None, true).kept_message("今晚上分"),
            None,
            "nothing kept: the client's own stays"
        );
        assert_eq!(rule(Some(""), true).kept_message("今晚上分"), mobile);
        assert_eq!(
            rule(Some(""), false).kept_message("今晚上分"),
            Some(String::new())
        );
        assert_eq!(rule(None, false).kept_message(""), None);
        let away = PresenceRule {
            availability: "away".into(),
            ..rule(None, true)
        };
        assert_eq!(away.kept_message(""), None);
    }

    #[test]
    fn a_disguise_is_written_as_the_window_reads_it() {
        let disguise = RankDisguise {
            enabled: true,
            queue: DisguiseQueue::Flex,
            tier: Tier::Master,
            division: Division::Two,
        };
        let json = serde_json::to_value(&disguise).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "enabled": true, "queue": "flex", "tier": "MASTER", "division": "II"
            })
        );
        assert_eq!(
            serde_json::from_value::<RankDisguise>(json).unwrap(),
            disguise
        );
    }

    #[test]
    fn hotkeys_are_written_one_way_and_need_a_real_modifier() {
        let cases = [
            ("Ctrl+Shift+W", Some("Ctrl+Shift+W")),
            (" shift + ctrl + w ", Some("Ctrl+Shift+W")),
            ("Control+KeyQ", Some("Ctrl+Q")),
            ("Alt+Digit1", Some("Alt+1")),
            ("Win+Alt+ArrowUp", Some("Alt+Super+Up")),
            ("Meta+F5", Some("Super+F5")),
            ("Ctrl+Numpad7", Some("Ctrl+Num7")),
            ("Ctrl+NumpadAdd", Some("Ctrl+NumAdd")),
            ("ctrl+backquote", Some("Ctrl+Backquote")),
            ("Ctrl+F24", Some("Ctrl+F24")),
            ("Ctrl+F", Some("Ctrl+F")),
            ("Shift+W", None),
            ("W", None),
            ("Ctrl+Shift", None),
            ("Ctrl+W+Q", None),
            ("Ctrl++W", None),
            ("Ctrl+F25", None),
            ("Ctrl+F0", None),
            ("Ctrl+Enter", None),
            ("Ctrl+NumpadEnter", None),
            ("", None),
        ];
        for (text, expected) in cases {
            assert_eq!(normalize_hotkey(text).as_deref(), expected, "{text:?}");
        }
        for key in HOTKEY_KEYS {
            assert_eq!(
                normalize_hotkey(&format!("Ctrl+{key}")),
                Some(format!("Ctrl+{key}")),
                "a listed key is its own spelling"
            );
        }
    }

    #[test]
    fn the_hotkey_starts_on_can_be_switched_off_and_a_bad_one_is_dropped() {
        assert_eq!(
            Settings::default().general.hotkey.as_deref(),
            Some(DEFAULT_HOTKEY)
        );
        let old: Settings = serde_json::from_str(r#"{"general":{"titles":false}}"#).unwrap();
        assert_eq!(
            old.general.hotkey.as_deref(),
            Some(DEFAULT_HOTKEY),
            "a file from before the hotkey gets the default"
        );
        let off: Settings = serde_json::from_str(r#"{"general":{"hotkey":null}}"#).unwrap();
        assert_eq!(off.normalized().general.hotkey, None);
        let mut settings = Settings::default();
        settings.general.hotkey = Some("alt + q".into());
        assert_eq!(
            settings.clone().normalized().general.hotkey.as_deref(),
            Some("Alt+Q")
        );
        settings.general.hotkey = Some("Q".into());
        assert_eq!(settings.normalized().general.hotkey, None);
        let plugin = PluginSettings::default();
        assert!(
            plugin.friend_status && plugin.lobby_panel,
            "both in-client additions start on"
        );
        let old: Settings = serde_json::from_str(r#"{"plugin":{"teamPanel":false}}"#).unwrap();
        assert!(
            old.plugin.friend_status && old.plugin.lobby_panel && !old.plugin.team_panel,
            "a file from before them gets them on and keeps its own switches"
        );
    }

    // The history panel in the client.
    #[test]
    fn the_history_panel_in_the_client_starts_on_and_can_be_switched_off() {
        assert!(PluginSettings::default().history_in_client);
        let old: Settings =
            serde_json::from_str(r#"{"plugin":{"lobbyPanel":false,"teamPanel":true}}"#).unwrap();
        assert!(
            old.plugin.history_in_client && !old.plugin.lobby_panel,
            "a file from before it gets it on and keeps its own switches"
        );
        let off: Settings =
            serde_json::from_str(r#"{"plugin":{"historyInClient":false}}"#).unwrap();
        assert!(!off.normalized().plugin.history_in_client);
    }

    #[test]
    fn loadouts_and_item_sets_start_off_and_act_only_where_they_can() {
        let settings = Settings::default();
        let automation = &settings.automation;
        assert!(!automation.loadout.enabled && automation.loadout.recommended);
        assert!(!automation.item_sets);
        assert!(
            settings.builds.enabled,
            "the panel only reads public numbers"
        );
        assert_eq!(settings.builds.rift_source, RiftSource::Tencent);
        assert!(
            !Scoped::Loadout.applicable().contains(&Mode::Arena),
            "Arena has no rune page and fixed spells"
        );
        assert!(!Scoped::ItemSets.applicable().contains(&Mode::Other));

        let mut on = Settings::default();
        on.automation.loadout.enabled = true;
        on.automation.item_sets = true;
        on.automation.scopes.loadout = vec![Mode::Arena, Mode::Aram, Mode::Ranked];
        let on = on.normalized();
        assert_eq!(on.automation.scopes.loadout, vec![Mode::Ranked, Mode::Aram]);
        assert!(on.automation.restores_loadout(Some(Mode::Aram)));
        assert!(!on.automation.restores_loadout(Some(Mode::Normal)));
        assert!(
            !on.automation.restores_loadout(None),
            "narrowed, so not where unknown"
        );
        assert!(on.automation.writes_item_sets(Some(Mode::Arena)));
        assert!(!on.automation.writes_item_sets(Some(Mode::Other)));

        // A file from before these existed keeps them off and scoped everywhere they can act.
        let old: Settings =
            serde_json::from_str(r#"{"automation":{"scopes":{"accept":["ranked"]}}}"#).unwrap();
        assert_eq!(old.automation.scopes.loadout, Scoped::Loadout.applicable());
        assert_eq!(old.builds, BuildSettings::default());
        // A file from before the per-mode sources keeps every source it had.
        let before: Settings =
            serde_json::from_str(r#"{"builds":{"enabled":true,"riftSource":"opGg"}}"#).unwrap();
        assert_eq!(
            before.builds,
            BuildSettings {
                rift_source: RiftSource::OpGg,
                ..BuildSettings::default()
            }
        );
        assert_eq!(
            (
                before.builds.aram_source,
                before.builds.arena_source,
                before.builds.hextech_fallback
            ),
            (ModeSource::OpGg, ModeSource::OpGg, true)
        );
    }

    #[test]
    fn candidates_put_the_position_first_then_any() {
        let pool = ChampionPool {
            any: vec![1, 2, 3],
            middle: vec![3, 4],
            ..ChampionPool::default()
        };
        assert_eq!(pool.candidates(Some(Position::Middle)), vec![3, 4, 1, 2]);
        assert_eq!(pool.candidates(None), vec![1, 2, 3]);
    }

    // ---- The callout's shortcut and the game's chat ----

    #[test]
    fn a_file_from_before_the_callouts_shortcut_has_none_and_types_nothing_in_game() {
        let old: Settings = serde_json::from_str(
            r#"{"automation":{"callout":{"auto":true,"template":"{name}","tiers":"horses"}}}"#,
        )
        .unwrap();
        let callout = old.normalized().automation.callout;
        assert_eq!(callout.hotkey, None, "no combination is taken by itself");
        assert!(!callout.in_game, "nothing is typed into the game unasked");
        assert_eq!(
            (
                callout.watch_template.as_str(),
                callout.target_template.as_str()
            ),
            ("", ""),
            "the enemy lines start as the defaults"
        );
        assert!(callout.auto && callout.template == "{name}" && callout.tiers == TierSet::Horses);
        let defaults = CalloutRule::default();
        assert!(defaults.hotkey.is_none() && !defaults.in_game);
    }

    #[test]
    fn the_callouts_shortcut_is_spelled_one_way_and_never_takes_the_windows() {
        let with = |window: Option<&str>, callout: Option<&str>| {
            let mut settings = Settings::default();
            settings.general.hotkey = window.map(str::to_owned);
            settings.automation.callout.hotkey = callout.map(str::to_owned);
            let settings = settings.normalized();
            (settings.general.hotkey, settings.automation.callout.hotkey)
        };
        assert_eq!(
            with(Some("Alt+Backquote"), Some(" shift + ctrl + x ")),
            (Some("Alt+Backquote".into()), Some("Ctrl+Shift+X".into()))
        );
        assert_eq!(
            with(Some("Alt+Backquote"), Some("alt+backquote")),
            (Some("Alt+Backquote".into()), None),
            "the window keeps its combination"
        );
        assert_eq!(
            with(Some("alt + q"), Some("Alt+Q")),
            (Some("Alt+Q".into()), None),
            "compared once both are spelled the same way"
        );
        assert_eq!(
            with(None, Some("Alt+Backquote")).1.as_deref(),
            Some("Alt+Backquote")
        );
        assert_eq!(
            with(None, Some("X")).1,
            None,
            "a key alone is typed in chat"
        );

        let mut settings = Settings::default();
        settings.automation.callout.watch_template = format!("  {}  ", "小".repeat(300));
        settings.automation.callout.target_template = " {name} ".into();
        let callout = settings.normalized().automation.callout;
        assert_eq!(callout.watch_template.chars().count(), 200);
        assert_eq!(callout.target_template, "{name}");
    }

    // History.

    #[test]
    fn custom_games_start_hidden_and_a_file_keeps_them_shown_once_chosen() {
        assert!(Settings::default().history.hide_custom_games);
        let older: Settings = serde_json::from_str(r#"{"general":{"titles":false}}"#).unwrap();
        assert!(
            older.history.hide_custom_games,
            "a file from before the switch"
        );
        let shown: Settings =
            serde_json::from_str(r#"{"history":{"hideCustomGames":false}}"#).unwrap();
        assert!(!shown.normalized().history.hide_custom_games);
    }
}
