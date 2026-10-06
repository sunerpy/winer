//! User settings: the schema the window edits, and the file that keeps it.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{callout, view::Position};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub appearance: Appearance,
    pub general: General,
    pub automation: Automation,
    pub plugin: PluginSettings,
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
}

impl Default for General {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: Language::ZhCn,
            augment_details: true,
            titles: true,
        }
    }
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
}

impl Automation {
    /// Whether to go back to the lobby after a game of `mode`.
    pub fn plays_again(&self, mode: Option<Mode>) -> bool {
        self.play_again && self.scopes.covers(Scoped::PlayAgain, mode)
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
}

impl Scoped {
    /// The kinds of game the rule can act in at all: nobody picks or bans in ARAM, and only ARAM
    /// has a bench.
    pub fn applicable(self) -> &'static [Mode] {
        const PICKED: [Mode; 4] = [Mode::Ranked, Mode::Normal, Mode::Arena, Mode::Other];
        match self {
            Self::Pick | Self::Ban => &PICKED,
            Self::Bench => &[Mode::Aram, Mode::Hextech],
            Self::Accept | Self::Callout | Self::PlayAgain => &Mode::ALL,
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
    /// `{quip}`. Empty means the language's default (`callout::template`).
    pub template: String,
    /// How the team is split, and what the tiers are called.
    pub tiers: TierSet,
    /// The user's own tier names, best first, for `TierSet::Custom`: two to five, blanks skipped.
    pub custom_tiers: Vec<String>,
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
}

impl Default for PickRule {
    fn default() -> Self {
        Self {
            enabled: false,
            lock_in: true,
            declare_intent: true,
            champions: ChampionPool::default(),
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
    /// Hide the activity centre and esports pop-ups on the client home page.
    pub hide_promotions: bool,
    /// In the client's own champ select, a click on an ARAM bench champion swaps at once: the
    /// plugin lifts the cooldown and winer carries the swap out.
    pub bench_no_cooldown: bool,
    /// Pengu Loader's directory, when it cannot be found from the client.
    pub loader_dir: Option<String>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            auto: true,
            team_panel: true,
            hide_promotions: false,
            bench_no_cooldown: true,
            loader_dir: None,
        }
    }
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
        self
    }

    /// Brings a file an older winer wrote up to date. Up to 0.0.2 the default callout line named
    /// the champion, and a template saved as exactly that text would have kept it for good: it
    /// becomes the default, which names the seat (the same language's, when the text was in the
    /// other one). A template the user changed stays as written.
    fn migrated(mut self) -> Self {
        let callout = &mut self.automation.callout;
        if let Some(language) = callout::former_default(&callout.template) {
            callout.template = if language == self.general.language {
                String::new()
            } else {
                callout::template(language).to_owned()
            };
        }
        self
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
            callout::template(Language::En),
            "an English line under the Chinese window stays English"
        );

        for own in [
            "{standing}：{champion} {name}",
            "{standing}：{champion} {name} 近{games}场胜率{winRate}",
            "{name} 玩 {champion}",
        ] {
            assert_eq!(loaded("zh-CN", own).template, own, "changed by the user");
        }
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
    fn candidates_put_the_position_first_then_any() {
        let pool = ChampionPool {
            any: vec![1, 2, 3],
            middle: vec![3, 4],
            ..ChampionPool::default()
        };
        assert_eq!(pool.candidates(Some(Position::Middle)), vec![3, 4, 1, 2]);
        assert_eq!(pool.candidates(None), vec![1, 2, 3]);
    }
}
