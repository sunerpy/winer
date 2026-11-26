//! Runes, summoner spells and item sets for the champion in hand: what winer recommends (its
//! numbers, else the client's own recommended pages), the user's own setup for the champion and
//! kind of game (我的方案, kept in `loadout-presets.json`), which of them to set up and when, where a
//! rune page can go; and winer's item sets inside the client's own list. Carrying them out is the
//! service's job (`service::loadout`); nothing here touches the client.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json, value::RawValue};
use ts_rs::TS;

use crate::{
    builds::{Build, BuildSource, ItemOption, Rates, RuneOption, RunePage, SpellOption},
    model::ChampSelectSession,
    settings::{Automation, Language, Mode, write_atomic},
    view::Position,
};

// The client's own shapes (`/lol-perks/v1/…`), reduced to what is read.

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PerkPage {
    pub id: i64,
    pub name: String,
    pub current: bool,
    pub is_editable: bool,
    pub is_deletable: bool,
    /// The client's own page for the champion in hand, made when there is no room for another.
    pub is_temporary: bool,
    pub is_valid: bool,
    pub primary_style_id: i64,
    pub sub_style_id: i64,
    pub selected_perk_ids: Vec<i64>,
}

impl PerkPage {
    /// Its choices, when it is a full page the client accepts.
    pub fn runes(&self) -> Option<RunePage> {
        let page = RunePage {
            primary_style: self.primary_style_id,
            sub_style: self.sub_style_id,
            perks: self.selected_perk_ids.clone(),
        };
        (self.is_valid && page.is_complete()).then_some(page)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PerkInventory {
    pub can_add_custom_page: bool,
    pub custom_page_count: i64,
    pub owned_page_count: i64,
}

/// One of the pages the client recommends for a champion, position and map.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RecommendedPage {
    pub is_default_position: bool,
    pub primary_perk_style_id: i64,
    pub secondary_perk_style_id: i64,
    pub perks: Vec<RecommendedPerk>,
    pub summoner_spell_ids: Vec<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RecommendedPerk {
    pub id: i64,
}

// The user's own setups (我的方案).

/// A champion's runes and spells for one kind of game: the user's own (我的方案), or what winer set
/// up. Either half may be missing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    /// `None` where there was no full page to keep.
    pub runes: Option<RunePage>,
    pub spells: Option<[i64; 2]>,
    /// Epoch milliseconds the user saved it at.
    #[serde(default)]
    pub saved_at: i64,
}

/// The user's own setups and the file an older winer remembered games in, for the automation page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutSummary {
    /// Setups of the user's own (我的方案), one a champion, kind of game and lane.
    pub presets: u32,
    /// `loadouts.json`, where winer up to 0.0.24 remembered each game's setup by itself: `None`
    /// where there is no such file, else how many setups it holds (0 for one that does not read).
    /// Nothing reads it to set anything up any more.
    pub legacy: Option<u32>,
}

/// Where a setup lives in a file: `"202:ranked"`.
fn key(champion_id: i64, mode: Mode) -> String {
    let mode = serde_json::to_value(mode)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("{champion_id}:{mode}")
}

fn lane_name(lane: Position) -> &'static str {
    match lane {
        Position::Top => "top",
        Position::Jungle => "jungle",
        Position::Middle => "middle",
        Position::Bottom => "bottom",
        Position::Utility => "utility",
    }
}

/// Where the user's own setup for a champion lives: `"202:hextech"`; on the Rift with a lane,
/// `"202:ranked:bottom"`, since a champion is set up apart for each lane it is played in.
pub fn preset_key(champion_id: i64, mode: Mode, lane: Option<Position>) -> String {
    match lane.filter(|_| matches!(mode, Mode::Ranked | Mode::Normal)) {
        Some(lane) => format!("{}:{}", key(champion_id, mode), lane_name(lane)),
        None => key(champion_id, mode),
    }
}

/// The keys the user's setup for `champion_id` may be under, in the order they are looked up:
/// the lane's own, then the one without a lane (saved in a game that assigned none).
pub fn preset_keys(champion_id: i64, mode: Mode, lane: Option<Position>) -> Vec<String> {
    let own = preset_key(champion_id, mode, lane);
    let bare = key(champion_id, mode);
    if own == bare {
        vec![own]
    } else {
        vec![own, bare]
    }
}

/// A file of setups by key, beside the settings (`loadout-presets.json`). Writes are atomic, as the
/// settings' are.
pub struct LoadoutStore {
    path: PathBuf,
    setups: Mutex<BTreeMap<String, Setup>>,
}

impl LoadoutStore {
    /// Reads `path`; a missing file is an empty one, and a file that does not parse is moved aside
    /// rather than overwritten.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let setups = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                let aside = path.with_extension("json.invalid");
                tracing::warn!(%error, aside = %aside.display(), "setups file is invalid, starting empty");
                let _ = fs::rename(&path, aside);
                BTreeMap::new()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                tracing::warn!(%error, "setups file is unreadable, starting empty");
                BTreeMap::new()
            }
        };
        Self {
            path,
            setups: Mutex::new(setups),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Setup>> {
        self.setups
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The first setup under one of `keys` (`preset_keys`), with the key it was under.
    pub fn find(&self, keys: &[String]) -> Option<(String, Setup)> {
        let setups = self.lock();
        keys.iter()
            .find_map(|key| setups.get(key).map(|setup| (key.clone(), setup.clone())))
    }

    pub fn put(&self, key: String, setup: Setup) -> io::Result<()> {
        let mut setups = self.lock();
        let mut next = setups.clone();
        next.insert(key, setup);
        self.write(&next)?;
        *setups = next;
        Ok(())
    }

    /// Takes out every setup under `keys`; whether there was one. The file is written only then.
    pub fn remove(&self, keys: &[String]) -> io::Result<bool> {
        let mut setups = self.lock();
        let mut next = setups.clone();
        let removed = keys
            .iter()
            .filter(|key| next.remove(*key).is_some())
            .count();
        if removed == 0 {
            return Ok(false);
        }
        self.write(&next)?;
        *setups = next;
        Ok(true)
    }

    pub fn clear(&self) -> io::Result<()> {
        let mut setups = self.lock();
        self.write(&BTreeMap::new())?;
        setups.clear();
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, setups: &BTreeMap<String, Setup>) -> io::Result<()> {
        write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(setups).expect("setups serialize"),
        )
    }
}

/// What `loadouts.json`, the file winer up to 0.0.24 remembered each game's setup in, holds now:
/// `None` without the file, else how many setups (0 for a file that does not read). Only read, never
/// moved or written: deleting it is the user's call (`remove_legacy`).
pub fn legacy_count(path: &Path) -> Option<u32> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(0),
    };
    let setups: BTreeMap<String, Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    Some(u32::try_from(setups.len()).unwrap_or(u32::MAX))
}

/// Deletes `loadouts.json`; a file that is gone already is fine.
pub fn remove_legacy(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

// What winer recommends, and what it sets up.

/// Where a half of what winer sets up came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LoadoutOrigin {
    /// The user's own setup for the champion and kind of game (我的方案).
    Mine,
    /// winer's numbers (腾讯 101, OP.GG).
    Recommended,
    /// The client's own recommended pages.
    Client,
}

/// Where a recommendation's options come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RecommendedBy {
    Tencent,
    OpGg,
    /// Hextech ARAM's augments and items when Tencent's do not answer.
    AramGg,
    /// The client's own recommended pages: no rates.
    Client,
}

impl RecommendedBy {
    pub fn origin(self) -> LoadoutOrigin {
        match self {
            Self::Tencent | Self::OpGg | Self::AramGg => LoadoutOrigin::Recommended,
            Self::Client => LoadoutOrigin::Client,
        }
    }
}

/// Whether a kind of game takes a rune page: Hextech ARAM has augments instead, and Arena hands
/// out its own. Neither the panels nor the rule touch runes in the others.
pub fn takes_runes(mode: Mode) -> bool {
    !matches!(mode, Mode::Hextech | Mode::Arena)
}

/// Hextech ARAM augments the panel lists, and core builds.
pub const ADVICE_AUGMENTS: usize = 6;
pub const ADVICE_BUILDS: usize = 2;

/// One of a champion's Hextech ARAM augments as the source grades it: its tier (S to C) and how
/// often it is taken, never its win rate, which Riot's policy keeps off every product.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AugmentPick {
    pub id: i64,
    pub tier: Option<crate::builds::AugmentTier>,
    /// The share of the champion's games that took it, 0–1.
    pub pick: Option<f64>,
}

/// The options a recommendation lists for each half: the first is the one set up.
pub const RECOMMENDED_OPTIONS: usize = 3;

/// What winer recommends for a champion in a kind of game: rune pages and spell pairs, most taken
/// first, each half from winer's numbers where the source has it, else from the client's own
/// recommended pages.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutRecommendation {
    pub runes: Vec<RuneOption>,
    pub spells: Vec<SpellOption>,
    /// Where the rune pages come from; `None` while there are none.
    pub runes_from: Option<RecommendedBy>,
    pub spells_from: Option<RecommendedBy>,
    /// The source's patch, `16.19`; empty where it says none or the client's pages are used.
    pub patch: String,
    /// Hextech ARAM: the champion's augments, best first, at most [`ADVICE_AUGMENTS`].
    pub augments: Vec<AugmentPick>,
    /// Hextech ARAM: core builds, most taken first, at most [`ADVICE_BUILDS`], and boots.
    pub core: Vec<ItemOption>,
    pub boots: Vec<ItemOption>,
    /// Where the augments and items come from.
    pub build_from: Option<RecommendedBy>,
}

impl LoadoutRecommendation {
    pub fn is_empty(&self) -> bool {
        self.runes.is_empty()
            && self.spells.is_empty()
            && self.augments.is_empty()
            && self.core.is_empty()
            && self.boots.is_empty()
    }
}

/// Adds Hextech ARAM's advice from `build` (Tencent's numbers, or ARAM.GG's when they did not
/// answer): the best augments by their tier and how often they are taken, without win rates, and
/// the core builds and boots. Nothing for another kind of build.
pub fn hextech_advice(recommendation: &mut LoadoutRecommendation, build: Option<&Build>) {
    let Some(build) = build else {
        return;
    };
    let by = match build.source {
        BuildSource::TencentHextech => RecommendedBy::Tencent,
        BuildSource::AramGg => RecommendedBy::AramGg,
        BuildSource::Tencent | BuildSource::OpGg => return,
    };
    recommendation.augments = build
        .augments
        .iter()
        .take(ADVICE_AUGMENTS)
        .map(|augment| AugmentPick {
            id: augment.id,
            tier: augment.tier,
            pick: augment.rates.pick,
        })
        .collect();
    recommendation.core = build.core.iter().take(ADVICE_BUILDS).cloned().collect();
    recommendation.boots = build.boots.iter().take(1).cloned().collect();
    if !(recommendation.augments.is_empty()
        && recommendation.core.is_empty()
        && recommendation.boots.is_empty())
    {
        recommendation.build_from = Some(by);
        if recommendation.patch.is_empty() {
            recommendation.patch = build.patch.clone();
        }
    }
}

/// Two different spells, both set.
fn spells(spells: [i64; 2]) -> Option<[i64; 2]> {
    (spells[0] > 0 && spells[1] > 0 && spells[0] != spells[1]).then_some(spells)
}

/// The same two spells, on whichever keys.
pub fn same_spells(a: [i64; 2], b: [i64; 2]) -> bool {
    a == b || a == [b[1], b[0]]
}

/// What winer recommends from `build` (its numbers, when there are some for the mode) and the
/// client's own recommended `pages`: each half from the build where it has one, else the pages',
/// the page for the position first (`isDefaultPosition`), at most [`RECOMMENDED_OPTIONS`] each.
pub fn recommendation(build: Option<&Build>, pages: &[RecommendedPage]) -> LoadoutRecommendation {
    let by = build.and_then(|build| match build.source {
        BuildSource::Tencent => Some(RecommendedBy::Tencent),
        BuildSource::OpGg => Some(RecommendedBy::OpGg),
        // Their numbers have no runes nor spells.
        BuildSource::TencentHextech | BuildSource::AramGg => None,
    });
    let mut ordered: Vec<&RecommendedPage> = pages.iter().collect();
    ordered.sort_by_key(|page| !page.is_default_position);
    let mut recommendation = LoadoutRecommendation::default();

    let built_runes: Vec<RuneOption> = build
        .filter(|_| by.is_some())
        .map(|build| {
            build
                .runes
                .iter()
                .filter(|option| option.page.is_complete())
                .take(RECOMMENDED_OPTIONS)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if built_runes.is_empty() {
        let mut runes: Vec<RuneOption> = Vec::new();
        for page in &ordered {
            let page = RunePage {
                primary_style: page.primary_perk_style_id,
                sub_style: page.secondary_perk_style_id,
                perks: page.perks.iter().map(|perk| perk.id).collect(),
            };
            if page.is_complete() && !runes.iter().any(|option| option.page == page) {
                runes.push(RuneOption {
                    page,
                    rates: Rates::default(),
                });
            }
        }
        runes.truncate(RECOMMENDED_OPTIONS);
        recommendation.runes_from = (!runes.is_empty()).then_some(RecommendedBy::Client);
        recommendation.runes = runes;
    } else {
        recommendation.runes = built_runes;
        recommendation.runes_from = by;
    }

    let built_spells: Vec<SpellOption> = build
        .filter(|_| by.is_some())
        .map(|build| {
            build
                .spells
                .iter()
                .filter(|option| spells(option.spells).is_some())
                .take(RECOMMENDED_OPTIONS)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if built_spells.is_empty() {
        let mut pairs: Vec<SpellOption> = Vec::new();
        for page in &ordered {
            let Some(pair) = (match page.summoner_spell_ids[..] {
                [first, second] => spells([first, second]),
                _ => None,
            }) else {
                continue;
            };
            if !pairs.iter().any(|option| same_spells(option.spells, pair)) {
                pairs.push(SpellOption {
                    spells: pair,
                    rates: Rates::default(),
                });
            }
        }
        pairs.truncate(RECOMMENDED_OPTIONS);
        recommendation.spells_from = (!pairs.is_empty()).then_some(RecommendedBy::Client);
        recommendation.spells = pairs;
    } else {
        recommendation.spells = built_spells;
        recommendation.spells_from = by;
    }

    if [recommendation.runes_from, recommendation.spells_from]
        .iter()
        .any(|from| matches!(from, Some(RecommendedBy::Tencent | RecommendedBy::OpGg)))
    {
        recommendation.patch = build.map(|build| build.patch.clone()).unwrap_or_default();
    }
    recommendation
}

/// What winer sets up for a champion: each half and where it came from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub runes: Option<(RunePage, LoadoutOrigin)>,
    pub spells: Option<([i64; 2], LoadoutOrigin)>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.runes.is_none() && self.spells.is_none()
    }

    /// The plan for a game of `mode`: without runes where it takes none ([`takes_runes`]).
    pub fn for_mode(mut self, mode: Mode) -> Self {
        if !takes_runes(mode) {
            self.runes = None;
        }
        self
    }
}

/// What to set up: each half from the user's own setup (`mine`) where it has that half, else the
/// recommendation's first option, where there is one (`recommended`).
pub fn plan(mine: Option<&Setup>, recommended: Option<&LoadoutRecommendation>) -> Plan {
    let runes = mine
        .and_then(|mine| mine.runes.clone())
        .map(|runes| (runes, LoadoutOrigin::Mine))
        .or_else(|| {
            let recommended = recommended?;
            let option = recommended.runes.first()?;
            Some((option.page.clone(), recommended.runes_from?.origin()))
        });
    let spells = mine
        .and_then(|mine| mine.spells)
        .map(|spells| (spells, LoadoutOrigin::Mine))
        .or_else(|| {
            let recommended = recommended?;
            let option = recommended.spells.first()?;
            Some((option.spells, recommended.spells_from?.origin()))
        });
    Plan { runes, spells }
}

/// Whether `mine` has both halves, so nothing needs recommending.
pub fn whole(mine: Option<&Setup>) -> bool {
    mine.is_some_and(|mine| mine.runes.is_some() && mine.spells.is_some())
}

/// Whether a kind of game has runes and spells to choose: Arena hands out both.
pub fn shows(mode: Mode) -> bool {
    mode != Mode::Arena
}

// What the window's and the client's panels show of the champion in hand, and what they ask.

/// The runes and spells of the local player's champion in this champ select: what they have, their
/// own setup, winer's recommendation and what winer set up. A request to act on it names its
/// `revision` ([`LoadoutTarget`]).
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutView {
    pub game_id: i64,
    /// The champion held, or hovered before it is.
    pub champion_id: i64,
    pub mode: Mode,
    /// The lane assigned on the Rift.
    pub lane: Option<Position>,
    /// The champion is the player's for the game (locked in, or handed out), not only hovered.
    pub held: bool,
    /// Taken from one counter for the whole run, the next one whenever anything else here
    /// changes: no two views with other contents share one.
    pub revision: u32,
    /// What the rule sets up once the champion is the player's, so the panels promise no more.
    pub auto: LoadoutAuto,
    /// The kind of game takes a rune page ([`takes_runes`]): in Hextech ARAM the panels show the
    /// spells, augments and items, and nothing sets runes.
    pub takes_runes: bool,
    pub current: LoadoutCurrent,
    /// The user's own setup for the champion, kind of game and lane (我的方案).
    pub mine: Option<Setup>,
    pub recommendation: RecommendationState,
    /// What winer set up for this champion in this champ select, by itself or when asked.
    pub applied: Option<AppliedLoadout>,
    /// The runes, styles and spells the view names, with their names and icons: the client's
    /// panel has no catalog of its own.
    pub assets: Vec<crate::view::AssetInfo>,
}

/// What 自动配置符文和召唤师技能 does for the champion in hand once it is the player's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LoadoutAuto {
    /// The rule is off, or not for this kind of game: nothing goes up by itself.
    Off,
    /// The user's own setup only: 没有我的方案时用推荐 is off, or the kind of game has no
    /// recommendation.
    Mine,
    /// The user's own setup, else the recommendation.
    MineOrRecommended,
}

impl LoadoutAuto {
    /// What the rule does in a game of `mode` under `rule`.
    pub fn of(rule: &Automation, mode: Mode) -> Self {
        if !rule.restores_loadout(Some(mode)) {
            Self::Off
        } else if rule.loadout.recommended && recommends(mode) {
            Self::MineOrRecommended
        } else {
            Self::Mine
        }
    }
}

/// Whether winer has a recommendation for a kind of game: the Rift's and the two ARAMs'.
pub fn recommends(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Ranked | Mode::Normal | Mode::Aram | Mode::Hextech
    )
}

/// What the player has now.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutCurrent {
    /// The client's current rune page, where it is a full one; `None` until it has been read.
    pub runes: Option<RunePage>,
    /// Its name: `winer · 烬`, or one of the player's.
    pub page: String,
    pub spells: [i64; 2],
}

/// winer's recommendation for the champion in hand, as far as it has come.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RecommendationState {
    /// Being looked up.
    Loading,
    Ready {
        recommendation: LoadoutRecommendation,
    },
    /// Not looked up: neither the rule nor the client's panel asks for it.
    Off,
    /// Nothing to recommend: no source for the mode, and no page of the client's.
    Nothing,
    /// The lookup failed.
    Failed { message: String },
}

impl RecommendationState {
    pub fn ready(&self) -> Option<&LoadoutRecommendation> {
        match self {
            Self::Ready { recommendation } => Some(recommendation),
            _ => None,
        }
    }
}

/// What winer set up, and where each half came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppliedLoadout {
    /// `None` where winer set no runes.
    pub runes: Option<LoadoutOrigin>,
    pub spells: Option<LoadoutOrigin>,
    /// Runes were to go up, but no page could take them (`PageOutcome::NoPage`).
    pub no_page: bool,
}

/// One half of a setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LoadoutPart {
    Runes,
    Spells,
}

/// The view a request acts on, as the window or the client's panel drew it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutTarget {
    pub game_id: i64,
    pub champion_id: i64,
    pub revision: u32,
}

/// What the window's or the client's panel asks for. The runes and spells are worked out by the
/// service from the view, never taken from the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoadoutAction {
    /// The recommendation's rune page `index`, counted from 0.
    ApplyRunes { index: u8 },
    /// The recommendation's spell pair `index`.
    ApplySpells { index: u8 },
    /// The user's own setup, each half it has.
    ApplyMine,
    /// What the player has now becomes their own setup for the champion, kind of game and lane.
    SaveMine,
    /// The user's own setup goes, and the recommendation goes up.
    RestoreRecommended,
    /// winer's item set for the champion, from the build the recommendation's items come from.
    WriteItemSet,
}

/// What came of a request: the rune page, where runes went up, and whether the spells did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutDone {
    pub runes: Option<PageOutcome>,
    pub spells: bool,
    /// winer's item set was written.
    pub item_set: bool,
}

/// The champion the local player has in `session`: the one held, or the one hovered before.
pub fn champion_in_hand(session: &ChampSelectSession) -> Option<i64> {
    let me = session.local_player()?;
    [me.champion_id, me.champion_pick_intent]
        .into_iter()
        .find(|&champion| champion > 0)
}

/// Whether `target` names `view`, the view drawn last, and `session`, the client's now, is still
/// the champ select and champion it was drawn for. Anything else is a request made against an
/// older view, which acts on nothing.
pub fn names(target: &LoadoutTarget, view: &LoadoutView, session: &ChampSelectSession) -> bool {
    target.game_id == view.game_id
        && target.champion_id == view.champion_id
        && target.revision == view.revision
        && session.game_id == target.game_id
        && champion_in_hand(session) == Some(target.champion_id)
}

// When to set a champion up.

/// The local player's champion once it is theirs for this game: their pick completed, or, where
/// champions are handed out (ARAM's bench and rerolls, no pick turn of their own), whatever they
/// hold now, so a swap from the bench counts as a new champion.
pub fn held_champion(session: &ChampSelectSession) -> Option<i64> {
    let me = session.local_player()?;
    if me.champion_id <= 0 {
        return None;
    }
    let mut picks = session
        .all_actions()
        .filter(|action| action.actor_cell_id == me.cell_id && action.kind == "pick")
        .peekable();
    let handed_out = session.bench_enabled || picks.peek().is_none();
    (handed_out || picks.any(|action| action.completed)).then_some(me.champion_id)
}

/// What has been set up in the champ select under way, so nothing is set up twice for one
/// champion: a change the user makes afterwards stands. A champion swapped away and back is set
/// up again, since what is on the page then belongs to the other one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    game_id: i64,
    loadout: Option<i64>,
    item_set: Option<i64>,
}

impl Watch {
    fn follow(&mut self, game_id: i64) {
        if self.game_id != game_id {
            *self = Self {
                game_id,
                ..Self::default()
            };
        }
    }

    /// The champion whose runes and spells to set up now, marked as done.
    pub fn loadout_due(&mut self, session: &ChampSelectSession) -> Option<i64> {
        let champion = held_champion(session)?;
        self.follow(session.game_id);
        (self.loadout != Some(champion)).then(|| {
            self.loadout = Some(champion);
            champion
        })
    }

    /// The champion whose item set to write now, marked as done.
    pub fn item_set_due(&mut self, session: &ChampSelectSession) -> Option<i64> {
        let champion = held_champion(session)?;
        self.follow(session.game_id);
        (self.item_set != Some(champion)).then(|| {
            self.item_set = Some(champion);
            champion
        })
    }
}

/// The map the client recommends pages for: the Rift's or the Howling Abyss's. Arena has no rune
/// page, and the other modes' maps are not known from the queue.
pub fn recommendation_map(mode: Mode) -> Option<i64> {
    match mode {
        Mode::Ranked | Mode::Normal => Some(11),
        Mode::Aram | Mode::Hextech => Some(12),
        Mode::Arena | Mode::Other => None,
    }
}

/// The local player's assigned position as the client's recommendations name it, or `none`.
pub fn recommendation_position(session: &ChampSelectSession) -> &'static str {
    match session
        .local_player()
        .and_then(|me| Position::parse(&me.assigned_position))
    {
        Some(Position::Top) => "top",
        Some(Position::Jungle) => "jungle",
        Some(Position::Middle) => "middle",
        Some(Position::Bottom) => "bottom",
        Some(Position::Utility) => "utility",
        None => "none",
    }
}

/// What the player has now, to keep as their own setup: the current page where it is a full one,
/// and the two spells. `None` when neither is set.
pub fn current_setup(page: Option<&PerkPage>, spells_now: [i64; 2], now: i64) -> Option<Setup> {
    let setup = Setup {
        runes: page.and_then(PerkPage::runes),
        spells: spells(spells_now),
        saved_at: now,
    };
    (setup.runes.is_some() || setup.spells.is_some()).then_some(setup)
}

/// Whether two reads of the current page show the same page with the same runes; a read that is
/// missing (`None`) says nothing against it.
pub fn same_page(a: Option<&PerkPage>, b: Option<&PerkPage>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.id == b.id
                && a.primary_style_id == b.primary_style_id
                && a.sub_style_id == b.sub_style_id
                && a.selected_perk_ids == b.selected_perk_ids
        }
        _ => true,
    }
}

/// `wanted` on the keys the player already uses for them: a spell held now keeps its slot (Flash
/// on F stays on F), the other takes the slot that is left.
pub fn arrange_spells(current: [i64; 2], wanted: [i64; 2]) -> [i64; 2] {
    if wanted[0] == current[1] || wanted[1] == current[0] {
        [wanted[1], wanted[0]]
    } else {
        wanted
    }
}

// Where a rune page goes.

/// winer's pages start with this, so it finds its own one again whatever the champion.
pub const PAGE_PREFIX: &str = "winer · ";

/// winer's page for a champion: `winer · 烬`.
pub fn page_name(champion: &str) -> String {
    format!(
        "{PAGE_PREFIX}{}",
        champion.trim().chars().take(16).collect::<String>()
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageTarget {
    /// winer's own page, rewritten in place.
    Own(i64),
    /// A new page: winer has none and the client has room for another.
    New,
    /// The client's own temporary page, the current one, written over.
    Temporary(i64),
    /// Nowhere: no page of winer's, no room for one, and the current page is one of the user's.
    Full,
}

/// winer owns at most one page: its own if it has one, a new one while there is room, else the
/// client's temporary page; never one of the user's own.
pub fn page_target(pages: &[PerkPage], inventory: &PerkInventory) -> PageTarget {
    if let Some(own) = pages
        .iter()
        .find(|page| page.is_editable && page.name.starts_with(PAGE_PREFIX))
    {
        return PageTarget::Own(own.id);
    }
    if inventory.can_add_custom_page {
        return PageTarget::New;
    }
    pages
        .iter()
        .find(|page| page.current && page.is_editable && page.is_temporary)
        .map_or(PageTarget::Full, |page| PageTarget::Temporary(page.id))
}

/// What became of a rune page winer was asked to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PageOutcome {
    /// winer's page holds it and is the current page.
    Written,
    /// No page could take it (`PageTarget::Full`).
    NoPage,
}

/// What the client derives from a page's runes for its own display (the keystone, the styles'
/// names and icons, the tooltip art). Sent back after the runes change they would describe the
/// old ones, so they are left for the client to work out again, as it does for a new page.
const DERIVED_PAGE_FIELDS: [&str; 8] = [
    "uiPerks",
    "pageKeystone",
    "primaryStyleName",
    "primaryStyleIconPath",
    "secondaryStyleName",
    "secondaryStyleIconPath",
    "tooltipBgPath",
    "autoModifiedSelections",
];

/// The body that writes `runes` over `page` as read: every field the client keeps for the page
/// (its id, order, flags) stays as it sent it.
pub fn page_body(page: &Value, name: &str, runes: &RunePage) -> Value {
    let mut body = page.clone();
    if let Some(fields) = body.as_object_mut() {
        for derived in DERIVED_PAGE_FIELDS {
            fields.remove(derived);
        }
        fields.insert("name".into(), json!(name));
        fields.insert("primaryStyleId".into(), json!(runes.primary_style));
        fields.insert("subStyleId".into(), json!(runes.sub_style));
        fields.insert("selectedPerkIds".into(), json!(runes.perks));
        fields.insert("current".into(), json!(true));
    }
    body
}

/// The body of a new page.
pub fn new_page_body(name: &str, runes: &RunePage) -> Value {
    json!({
        "name": name,
        "primaryStyleId": runes.primary_style,
        "subStyleId": runes.sub_style,
        "selectedPerkIds": runes.perks,
        "current": true,
    })
}

// Item sets (`/lol-item-sets/v1/item-sets/{summonerId}/sets`).

/// winer's sets carry this in their `uid`; nothing else in the list is ever touched.
pub const ITEM_SET_PREFIX: &str = "winer-";
/// winer keeps its sets for this many champions and maps, the newest: a set matters only while its
/// champion is played, and the client refuses a list past a size (`ITEM_SETS_BUDGET`).
pub const MAX_OWN_ITEM_SETS: usize = 20;
/// The size winer keeps the whole list under when it writes. The client took a list of 64,881 bytes
/// and refused the next, about 66 KB, with HTTP 413 (2026-10-31).
pub const ITEM_SETS_BUDGET: usize = 60_000;

pub fn item_set_uid(champion_id: i64, mode: Mode) -> String {
    format!(
        "{ITEM_SET_PREFIX}{}",
        key(champion_id, mode).replace(':', "-")
    )
}

/// The champion and kind of game one of winer's own uids is for.
fn our_uid(uid: &str) -> Option<(i64, Mode)> {
    let (champion, mode) = uid.strip_prefix(ITEM_SET_PREFIX)?.split_once('-')?;
    let mode = serde_json::from_value(Value::String(mode.to_owned())).ok()?;
    Some((champion.parse().ok()?, mode))
}

/// The client's list of item sets. The sets stay as raw JSON, so every one that is not winer's goes
/// back exactly as it came: the client replaces the whole list on every write.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSets {
    /// As the client sent it.
    #[serde(default)]
    pub account_id: Value,
    #[serde(default)]
    pub item_sets: Vec<Box<RawValue>>,
    #[serde(default)]
    pub timestamp: u64,
}

fn uid(set: &RawValue) -> Option<String> {
    #[derive(Deserialize)]
    struct Uid {
        uid: String,
    }
    serde_json::from_str::<Uid>(set.get())
        .ok()
        .map(|set| set.uid)
}

impl ItemSets {
    /// With `set`, winer's set for `champion_id` in a game of `mode`, in place of winer's sets the
    /// shop would list beside it: the same champion on the same map. Ranked and normal games share
    /// the Rift and the two ARAMs the Howling Abyss, so the champion keeps one set per map. The set
    /// goes last, so winer's sets run from the oldest written to the newest, and the oldest go while
    /// winer keeps more than [`MAX_OWN_ITEM_SETS`] or the list is longer than [`ITEM_SETS_BUDGET`].
    /// The sets that are not winer's keep their places.
    pub fn with(mut self, champion_id: i64, mode: Mode, set: Box<RawValue>, now: u64) -> Self {
        let own = item_set_uid(champion_id, mode);
        let maps = item_set_maps(mode);
        self.item_sets.retain(|other| {
            !uid(other).is_some_and(|uid| {
                uid == own
                    || our_uid(&uid).is_some_and(|(champion, other)| {
                        champion == champion_id
                            && item_set_maps(other).iter().any(|map| maps.contains(map))
                    })
            })
        });
        self.item_sets.push(set);
        self.timestamp = now;
        self.trim_ours();
        self
    }

    /// Lets go of winer's oldest sets, never the newest (the last), while winer keeps more than
    /// [`MAX_OWN_ITEM_SETS`] or the list is longer than [`ITEM_SETS_BUDGET`]. A list the player's
    /// own sets fill is left to the client to refuse.
    fn trim_ours(&mut self) {
        let ours = |set: &RawValue| uid(set).is_some_and(|uid| uid.starts_with(ITEM_SET_PREFIX));
        let mut count = self.item_sets.iter().filter(|set| ours(set)).count();
        let mut len = serde_json::to_vec(&*self).map_or(0, |body| body.len());
        while count > MAX_OWN_ITEM_SETS || len > ITEM_SETS_BUDGET {
            let newest = self.item_sets.len().saturating_sub(1);
            let Some(index) = self.item_sets[..newest].iter().position(|set| ours(set)) else {
                break;
            };
            let gone = self.item_sets.remove(index);
            count -= 1;
            // The set and the comma after it: it was not the last.
            len -= gone.get().len() + 1;
        }
    }

    /// Without any of winer's sets; how many went.
    pub fn without_ours(mut self, now: u64) -> (Self, u32) {
        let before = self.item_sets.len();
        self.item_sets
            .retain(|set| !uid(set).is_some_and(|uid| uid.starts_with(ITEM_SET_PREFIX)));
        let removed = (before - self.item_sets.len()) as u32;
        if removed > 0 {
            self.timestamp = now;
        }
        (self, removed)
    }
}

/// The maps a kind of game is played on, for a set to show on.
fn item_set_maps(mode: Mode) -> Vec<i64> {
    match mode {
        Mode::Ranked | Mode::Normal => vec![11],
        Mode::Aram | Mode::Hextech => vec![12],
        // Arena's queue (1700) names map 30.
        Mode::Arena => vec![30],
        Mode::Other => Vec::new(),
    }
}

fn source_name(source: BuildSource, language: Language) -> &'static str {
    match (source, language) {
        (BuildSource::Tencent, Language::ZhCn) => "腾讯 101",
        (BuildSource::Tencent, Language::En) => "Tencent 101",
        (BuildSource::TencentHextech, Language::ZhCn) => "腾讯海克斯大乱斗",
        (BuildSource::TencentHextech, Language::En) => "Tencent Hextech ARAM",
        (BuildSource::OpGg, _) => "OP.GG",
        (BuildSource::AramGg, _) => "ARAM.GG",
    }
}

/// The items of the first `options`, each once, a repeated one counted (two potions).
fn block_items(options: &[ItemOption], take: usize) -> Vec<(i64, u16)> {
    let mut items: Vec<(i64, u16)> = Vec::new();
    for option in options.iter().take(take) {
        let mut counts: Vec<(i64, u16)> = Vec::new();
        for &id in &option.items {
            match counts.iter_mut().find(|(other, _)| *other == id) {
                Some((_, count)) => *count += 1,
                None => counts.push((id, 1)),
            }
        }
        for (id, count) in counts {
            match items.iter_mut().find(|(other, _)| *other == id) {
                Some((_, most)) => *most = (*most).max(count),
                None => items.push((id, count)),
            }
        }
    }
    items
}

/// winer's item set for a build: starting items, boots, core and late options, each block named
/// with where its numbers come from.
pub fn item_set(build: &Build, champion: &str, language: Language) -> Box<RawValue> {
    let [starting, boots, core, late] = match language {
        Language::ZhCn => ["起始装备", "鞋子", "核心装备", "后期可选"],
        Language::En => ["Starting items", "Boots", "Core items", "Late options"],
    };
    let source = source_name(build.source, language);
    let origin = if build.patch.is_empty() {
        source.to_owned()
    } else {
        format!("{source} {}", build.patch)
    };
    let blocks: Vec<Value> = [
        (starting, block_items(&build.starting, 2)),
        (boots, block_items(&build.boots, 3)),
        (core, block_items(&build.core, 3)),
        (late, block_items(&build.late, 8)),
    ]
    .into_iter()
    .filter(|(_, items)| !items.is_empty())
    .map(|(name, items)| {
        json!({
            "type": format!("{name} · {origin}"),
            "hideIfSummonerSpell": "",
            "showIfSummonerSpell": "",
            "items": items
                .iter()
                .map(|(id, count)| json!({ "id": id.to_string(), "count": count }))
                .collect::<Vec<_>>(),
        })
    })
    .collect();
    let set = json!({
        "uid": item_set_uid(build.champion_id, build.mode),
        "title": page_name(champion),
        "mode": "any",
        "map": "any",
        "type": "custom",
        "sortrank": 0,
        "startedFrom": "blank",
        "associatedChampions": [build.champion_id],
        "associatedMaps": item_set_maps(build.mode),
        "blocks": blocks,
        "preferredItemSlots": [],
    });
    serde_json::value::to_raw_value(&set).expect("an item set serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hextech_aram_gets_augments_and_items_without_win_rates_and_no_runes() {
        use crate::builds::{AugmentOption, AugmentTier};
        let mut build = Build::new(BuildSource::AramGg, 103, Mode::Hextech);
        build.patch = "16.23".into();
        build.augments = (0..8)
            .map(|n| AugmentOption {
                id: 1000 + n,
                rarity: crate::view::Rarity::Gold,
                tier: Some(AugmentTier::S),
                rates: Rates {
                    pick: Some(0.1),
                    win: Some(0.6),
                    ..Rates::default()
                },
            })
            .collect();
        let items = |ids: &[i64], pick: f64| ItemOption {
            items: ids.to_vec(),
            rates: Rates {
                pick: Some(pick),
                win: Some(0.5),
                ..Rates::default()
            },
        };
        build.core = vec![
            items(&[1, 2, 3], 0.3),
            items(&[4, 5, 6], 0.2),
            items(&[7, 8, 9], 0.1),
        ];
        build.boots = vec![items(&[3020], 0.5), items(&[3111], 0.3)];
        let mut recommendation = LoadoutRecommendation::default();
        hextech_advice(&mut recommendation, Some(&build));
        assert_eq!(recommendation.augments.len(), ADVICE_AUGMENTS);
        assert_eq!(
            recommendation.augments[0],
            AugmentPick {
                id: 1000,
                tier: Some(AugmentTier::S),
                pick: Some(0.1),
            },
            "the tier and how often it is taken, never the win rate"
        );
        assert_eq!(recommendation.core.len(), ADVICE_BUILDS);
        assert_eq!(recommendation.boots.len(), 1);
        assert_eq!(recommendation.build_from, Some(RecommendedBy::AramGg));
        assert_eq!(recommendation.patch, "16.23");
        assert!(!recommendation.is_empty());

        // Another source's build has no advice of this kind.
        let mut rift = LoadoutRecommendation::default();
        hextech_advice(
            &mut rift,
            Some(&Build::new(BuildSource::Tencent, 103, Mode::Ranked)),
        );
        assert_eq!(rift, LoadoutRecommendation::default());

        // Nothing sets runes where the game takes none.
        assert!(!takes_runes(Mode::Hextech) && takes_runes(Mode::Aram));
        let setup = Setup {
            runes: Some(precision()),
            spells: Some([4, 32]),
            saved_at: 1,
        };
        let plan = plan(Some(&setup), None);
        assert!(plan.clone().for_mode(Mode::Hextech).runes.is_none());
        assert_eq!(plan.clone().for_mode(Mode::Hextech).spells, plan.spells);
        assert!(plan.for_mode(Mode::Aram).runes.is_some());
    }

    #[test]
    fn the_view_says_what_the_rule_sets_up_and_no_more() {
        let mut rule = Automation::default();
        assert_eq!(
            LoadoutAuto::of(&rule, Mode::Ranked),
            LoadoutAuto::Off,
            "off by default"
        );
        rule.loadout.enabled = true;
        rule.scopes.loadout = vec![Mode::Ranked, Mode::Other];
        assert_eq!(
            LoadoutAuto::of(&rule, Mode::Ranked),
            LoadoutAuto::MineOrRecommended
        );
        assert_eq!(
            LoadoutAuto::of(&rule, Mode::Other),
            LoadoutAuto::Mine,
            "no recommendation in other modes"
        );
        assert_eq!(
            LoadoutAuto::of(&rule, Mode::Aram),
            LoadoutAuto::Off,
            "outside its modes"
        );
        rule.loadout.recommended = false;
        assert_eq!(LoadoutAuto::of(&rule, Mode::Ranked), LoadoutAuto::Mine);
    }
    use crate::test_support::fixture;

    fn session(name: &str) -> ChampSelectSession {
        fixture(&format!(
            "lcu-rest/lol-champ-select-v1-session--champ-select-{name}.json"
        ))
    }

    fn aram(champion: i64) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "gameId": 77,
            "localPlayerCellId": 2,
            "benchEnabled": true,
            "myTeam": [{"cellId": 2, "championId": champion, "spell1Id": 4, "spell2Id": 32}]
        }))
        .unwrap()
    }

    #[test]
    fn a_champion_counts_once_its_pick_is_locked_or_handed_out() {
        assert_eq!(held_champion(&session("planning")), None);
        assert_eq!(
            held_champion(&session("ban-pick")),
            None,
            "nothing picked yet"
        );
        assert_eq!(held_champion(&session("finalization")), Some(236));

        // Hovered during the pick turn: not yet.
        let hovering: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 0,
            "myTeam": [{"cellId": 0, "championId": 157}],
            "actions": [[{"id": 1, "actorCellId": 0, "type": "pick", "isInProgress": true, "championId": 157}]]
        }))
        .unwrap();
        assert_eq!(held_champion(&hovering), None);
        let mut locked = hovering.clone();
        locked.actions[0][0].completed = true;
        assert_eq!(held_champion(&locked), Some(157));
        // Champions handed out, with or without a pick turn: whatever is held.
        assert_eq!(held_champion(&aram(22)), Some(22));
        assert_eq!(held_champion(&aram(0)), None);
    }

    #[test]
    fn each_champion_is_set_up_once_and_again_after_a_swap() {
        let mut watch = Watch::default();
        assert_eq!(watch.loadout_due(&session("ban-pick")), None);
        let locked = session("finalization");
        assert_eq!(watch.loadout_due(&locked), Some(236));
        assert_eq!(
            watch.loadout_due(&locked),
            None,
            "once, so a change by hand stands"
        );
        assert_eq!(
            watch.item_set_due(&locked),
            Some(236),
            "the item set keeps its own count"
        );
        assert_eq!(watch.item_set_due(&locked), None);

        // ARAM: a swap from the bench is a new champion, and so is swapping back.
        let mut watch = Watch::default();
        assert_eq!(watch.loadout_due(&aram(22)), Some(22));
        assert_eq!(watch.loadout_due(&aram(22)), None);
        assert_eq!(watch.loadout_due(&aram(99)), Some(99));
        assert_eq!(watch.loadout_due(&aram(22)), Some(22));

        // A new game starts over.
        let mut next = aram(22);
        next.game_id = 78;
        let mut watch = Watch::default();
        watch.loadout_due(&aram(22));
        assert_eq!(watch.loadout_due(&next), Some(22));
    }

    #[test]
    fn recommendations_follow_the_map_and_the_assigned_position() {
        assert_eq!(recommendation_map(Mode::Ranked), Some(11));
        assert_eq!(recommendation_map(Mode::Hextech), Some(12));
        assert_eq!(recommendation_map(Mode::Arena), None);
        assert_eq!(recommendation_position(&session("ban-pick")), "top");
        assert_eq!(recommendation_position(&aram(22)), "none");

        // The client's own pages: the one for the position first, its spells with it.
        let pages: Vec<RecommendedPage> = fixture("loadout/perks-recommended-202-bottom-11.json");
        let client = recommendation(None, &pages);
        assert_eq!(client.runes_from, Some(RecommendedBy::Client));
        assert_eq!(
            client.runes[0].page,
            RunePage {
                primary_style: 8100,
                sub_style: 8000,
                perks: vec![8128, 8139, 8140, 8135, 8014, 8009, 5008, 5008, 5001],
            }
        );
        assert_eq!(
            client.runes[0].rates,
            Rates::default(),
            "the client gives no rates"
        );
        assert_eq!(client.spells[0].spells, [21, 4]);
        assert_eq!(client.spells_from, Some(RecommendedBy::Client));
        assert!(client.runes.len() <= RECOMMENDED_OPTIONS && client.patch.is_empty());
        let abyss: Vec<RecommendedPage> = fixture("loadout/perks-recommended-202-none-12.json");
        assert_eq!(recommendation(None, &abyss).spells[0].spells, [4, 6]);
        assert!(recommendation(None, &[]).is_empty());
        assert_eq!(recommendation(None, &[]).runes_from, None);
        // The default position's page wins over the first.
        let mut mixed = pages.clone();
        mixed[0].is_default_position = false;
        mixed[1].is_default_position = true;
        mixed[1].primary_perk_style_id = 8000;
        assert_eq!(recommendation(None, &mixed).runes[0].page.perks[0], 8021);
    }

    fn rune_option(page: RunePage, pick: f64) -> RuneOption {
        RuneOption {
            page,
            rates: Rates {
                pick: Some(pick),
                ..Rates::default()
            },
        }
    }

    fn precision() -> RunePage {
        RunePage {
            primary_style: 8000,
            sub_style: 8100,
            perks: vec![8005, 8009, 9103, 8014, 8139, 8135, 5005, 5008, 5001],
        }
    }

    #[test]
    fn winers_numbers_come_first_and_the_clients_pages_fill_in_what_they_lack() {
        let abyss: Vec<RecommendedPage> = fixture("loadout/perks-recommended-202-none-12.json");
        let mut build = Build::new(BuildSource::OpGg, 202, Mode::Aram);
        build.patch = "16.19".into();
        build.runes = (0..5)
            .map(|n| rune_option(precision(), 0.5 - f64::from(n) / 10.0))
            .collect();
        build.runes[1].page.perks.pop();
        build.spells = vec![
            SpellOption {
                spells: [4, 32],
                rates: Rates::default(),
            },
            SpellOption {
                spells: [4, 4],
                rates: Rates::default(),
            },
        ];
        let both = recommendation(Some(&build), &abyss);
        assert_eq!(both.runes_from, Some(RecommendedBy::OpGg));
        assert_eq!(
            both.runes.len(),
            RECOMMENDED_OPTIONS,
            "the source's best three full pages"
        );
        assert!(both.runes.iter().all(|option| option.page.is_complete()));
        assert_eq!(both.spells.len(), 1, "not two of one spell");
        assert_eq!(
            (both.spells[0].spells, both.spells_from),
            ([4, 32], Some(RecommendedBy::OpGg))
        );
        assert_eq!(both.patch, "16.19");

        // The source without spells: the client's pages give them, and none of their runes.
        build.spells.clear();
        let mixed = recommendation(Some(&build), &abyss);
        assert_eq!(mixed.runes_from, Some(RecommendedBy::OpGg));
        assert_eq!(
            (mixed.spells[0].spells, mixed.spells_from),
            ([4, 6], Some(RecommendedBy::Client))
        );

        // Hextech ARAM's numbers have neither half: the client's pages give both.
        let mut hextech = Build::new(BuildSource::TencentHextech, 202, Mode::Hextech);
        hextech.runes = vec![rune_option(precision(), 0.9)];
        let client = recommendation(Some(&hextech), &abyss);
        assert_eq!(client.runes_from, Some(RecommendedBy::Client));
        assert_eq!(client.spells_from, Some(RecommendedBy::Client));
        assert!(client.patch.is_empty());
    }

    #[test]
    fn a_setup_of_the_users_own_wins_half_by_half() {
        let recommended = LoadoutRecommendation {
            runes: vec![rune_option(precision(), 0.4)],
            spells: vec![SpellOption {
                spells: [4, 32],
                rates: Rates::default(),
            }],
            runes_from: Some(RecommendedBy::Tencent),
            spells_from: Some(RecommendedBy::Client),
            patch: "16.19".into(),
            ..LoadoutRecommendation::default()
        };
        let none = plan(None, Some(&recommended));
        assert_eq!(none.runes, Some((precision(), LoadoutOrigin::Recommended)));
        assert_eq!(none.spells, Some(([4, 32], LoadoutOrigin::Client)));

        let ghost = Setup {
            runes: None,
            spells: Some([6, 4]),
            saved_at: 1,
        };
        let half = plan(Some(&ghost), Some(&recommended));
        assert_eq!(
            half.spells,
            Some(([6, 4], LoadoutOrigin::Mine)),
            "Snowball swapped for Ghost"
        );
        assert_eq!(half.runes, Some((precision(), LoadoutOrigin::Recommended)));
        assert!(!whole(Some(&ghost)));

        let mut mine = ghost.clone();
        mine.runes = Some(RunePage {
            primary_style: 8100,
            ..precision()
        });
        assert!(whole(Some(&mine)));
        let all = plan(Some(&mine), None);
        assert_eq!(
            all.runes.map(|(_, origin)| origin),
            Some(LoadoutOrigin::Mine)
        );
        assert_eq!(
            all.spells.map(|(_, origin)| origin),
            Some(LoadoutOrigin::Mine)
        );
        assert!(plan(None, None).is_empty());
        assert!(plan(None, Some(&LoadoutRecommendation::default())).is_empty());
    }

    #[test]
    fn what_the_player_has_now_is_kept_spells_alone_without_a_full_page() {
        let page: PerkPage = fixture("loadout/perks-currentpage.json");
        let setup = current_setup(Some(&page), [6, 7], 5).unwrap();
        assert_eq!(setup.spells, Some([6, 7]));
        assert_eq!(
            setup.runes.unwrap().perks,
            [8005, 8009, 9103, 8014, 8139, 8135, 5005, 5008, 5001]
        );
        assert_eq!(setup.saved_at, 5);
        assert_eq!(current_setup(None, [6, 7], 5).unwrap().runes, None);
        let mut broken = page.clone();
        broken.is_valid = false;
        assert_eq!(current_setup(Some(&broken), [6, 7], 5).unwrap().runes, None);
        assert_eq!(current_setup(None, [0, 7], 5), None, "neither half");
        assert_eq!(current_setup(Some(&broken), [4, 4], 5), None);
    }

    #[test]
    fn a_page_or_spells_changed_by_the_player_are_told_from_the_same_ones() {
        let page: PerkPage = fixture("loadout/perks-currentpage.json");
        assert!(same_page(Some(&page), Some(&page.clone())));
        let mut other = page.clone();
        other.selected_perk_ids[0] = 8010;
        assert!(!same_page(Some(&page), Some(&other)));
        let mut moved = page.clone();
        moved.id += 1;
        assert!(
            !same_page(Some(&page), Some(&moved)),
            "another page made current"
        );
        assert!(
            same_page(None, Some(&page)),
            "a read that failed says nothing"
        );
        assert!(same_spells([4, 14], [14, 4]));
        assert!(!same_spells([4, 14], [4, 12]));
    }

    #[test]
    fn spells_keep_the_keys_the_player_uses() {
        assert_eq!(arrange_spells([4, 14], [4, 21]), [4, 21]);
        assert_eq!(
            arrange_spells([14, 4], [4, 21]),
            [21, 4],
            "Flash stays on F"
        );
        assert_eq!(arrange_spells([4, 14], [21, 4]), [4, 21]);
        assert_eq!(arrange_spells([6, 7], [4, 14]), [4, 14]);
        assert_eq!(arrange_spells([21, 4], [4, 21]), [21, 4]);
    }

    fn page(id: i64, name: &str, current: bool, temporary: bool) -> PerkPage {
        PerkPage {
            id,
            name: name.into(),
            current,
            is_editable: true,
            is_deletable: !temporary,
            is_temporary: temporary,
            is_valid: true,
            ..PerkPage::default()
        }
    }

    #[test]
    fn a_page_goes_to_winers_own_then_a_new_one_then_the_temporary_one_and_never_the_users() {
        let room = PerkInventory {
            can_add_custom_page: true,
            ..PerkInventory::default()
        };
        let full = PerkInventory::default();
        let mine = page(1, "我的强攻", true, false);
        let ours = page(2, "winer · 阿狸", false, false);
        let temporary = page(3, "皮城女警 - 强攻", true, true);
        assert_eq!(
            page_target(&[mine.clone(), ours.clone()], &full),
            PageTarget::Own(2)
        );
        let alone = std::slice::from_ref(&mine);
        assert_eq!(page_target(alone, &room), PageTarget::New);
        assert_eq!(page_target(alone, &full), PageTarget::Full);
        assert_eq!(
            page_target(&[mine.clone(), temporary.clone()], &full),
            PageTarget::Temporary(3)
        );
        let mut locked = ours.clone();
        locked.is_editable = false;
        assert_eq!(page_target(&[locked], &room), PageTarget::New);

        // The measured client: a temporary page only, room for a custom one.
        let pages: Vec<PerkPage> = fixture("loadout/perks-pages.json");
        let inventory: PerkInventory = fixture("loadout/perks-inventory.json");
        assert_eq!(page_target(&pages, &inventory), PageTarget::New);
        assert_eq!(
            page_target(&pages, &full),
            PageTarget::Temporary(913_143_348)
        );
        assert_eq!(page_name(" 烬 "), "winer · 烬");
    }

    #[test]
    fn a_rewritten_page_keeps_what_the_client_sent_and_becomes_current() {
        let runes = RunePage {
            primary_style: 8100,
            sub_style: 8000,
            perks: vec![8128, 8139, 8140, 8135, 8009, 8014, 5008, 5008, 5001],
        };
        let read: Value = fixture("loadout/perks-currentpage.json");
        let body = page_body(&read, "winer · 烬", &runes);
        assert_eq!(body["id"], read["id"]);
        assert_eq!(body["order"], read["order"]);
        assert_eq!(body["isTemporary"], read["isTemporary"]);
        assert_eq!(body["name"], "winer · 烬");
        assert_eq!(body["selectedPerkIds"][0], 8128);
        assert_eq!(body["current"], true);
        // What described the old runes is not sent back to describe the new ones.
        assert!(read.get("uiPerks").is_some() && body.get("uiPerks").is_none());
        assert!(body.get("pageKeystone").is_none() && body.get("primaryStyleName").is_none());
        assert_eq!(new_page_body("winer · 烬", &runes)["subStyleId"], 8000);
    }

    #[test]
    fn setups_of_the_users_own_round_trip_by_champion_kind_of_game_and_lane() {
        assert_eq!(
            preset_key(202, Mode::Ranked, Some(Position::Bottom)),
            "202:ranked:bottom"
        );
        assert_eq!(
            preset_key(202, Mode::Hextech, Some(Position::Bottom)),
            "202:hextech"
        );
        assert_eq!(preset_key(202, Mode::Normal, None), "202:normal");
        assert_eq!(
            preset_keys(202, Mode::Ranked, Some(Position::Top)),
            ["202:ranked:top", "202:ranked"]
        );
        assert_eq!(preset_keys(202, Mode::Aram, None), ["202:aram"]);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loadout-presets.json");
        let store = LoadoutStore::open(&path);
        assert!(store.is_empty());
        let setup = Setup {
            runes: None,
            spells: Some([4, 7]),
            saved_at: 9,
        };
        store
            .put(preset_key(202, Mode::Hextech, None), setup.clone())
            .unwrap();
        store
            .put(preset_key(202, Mode::Ranked, None), setup.clone())
            .unwrap();
        let again = LoadoutStore::open(&path);
        assert_eq!(again.len(), 2);
        assert_eq!(
            again.find(&preset_keys(202, Mode::Ranked, Some(Position::Top))),
            Some(("202:ranked".into(), setup.clone())),
            "a lane without its own falls back to the one saved without a lane"
        );
        assert_eq!(
            again.find(&preset_keys(202, Mode::Aram, None)),
            None,
            "modes are kept apart"
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"202:hextech\""), "{text}");
        assert!(
            !again.remove(&["202:normal".into()]).unwrap(),
            "nothing there"
        );
        assert!(
            again
                .remove(&preset_keys(202, Mode::Ranked, Some(Position::Top)))
                .unwrap()
        );
        assert_eq!(LoadoutStore::open(&path).len(), 1);
        again.clear().unwrap();
        assert!(LoadoutStore::open(&path).is_empty());

        fs::write(&path, b"{ not json").unwrap();
        assert!(LoadoutStore::open(&path).is_empty());
        assert!(path.with_extension("json.invalid").exists());
    }

    #[test]
    fn the_old_files_setups_are_counted_never_moved_and_go_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loadouts.json");
        assert_eq!(legacy_count(&path), None);
        fs::write(
            &path,
            br#"{"202:ranked":{"runes":null,"spells":[4,7],"savedAt":1}}"#,
        )
        .unwrap();
        assert_eq!(legacy_count(&path), Some(1));
        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(legacy_count(&path), Some(0));
        assert!(path.exists(), "read only: left where it is");
        assert!(!path.with_extension("json.invalid").exists());
        remove_legacy(&path).unwrap();
        assert_eq!(legacy_count(&path), None);
        remove_legacy(&path).unwrap();
    }

    fn sets(json: &str) -> ItemSets {
        serde_json::from_str(json).unwrap()
    }

    const FOREIGN: &str = r#"{"uid":"a1b2","title":"My Jhin","mode":"any","map":"any","type":"custom","sortrank":3,"startedFrom":"blank","associatedChampions":[202],"associatedMaps":[11],"blocks":[{"type":"Rush","hideIfSummonerSpell":"","showIfSummonerSpell":"","items":[{"id":"1055","count":1}]}],"preferredItemSlots":[]}"#;

    #[test]
    fn item_sets_keep_every_other_set_exactly_and_replace_only_winers() {
        assert!(
            sets(r#"{"accountId":0,"itemSets":[],"timestamp":0}"#)
                .item_sets
                .is_empty()
        );
        let list = sets(&format!(
            r#"{{"accountId":12,"itemSets":[{FOREIGN},{{"uid":"winer-202-ranked","title":"old"}}],"timestamp":1}}"#
        ));
        let build = jhin();
        let merged = list.with(
            202,
            Mode::Ranked,
            item_set(&build, "烬", Language::ZhCn),
            500,
        );
        assert_eq!(merged.item_sets.len(), 2, "ours replaced, not added");
        assert_eq!(merged.item_sets[0].get(), FOREIGN, "byte for byte");
        assert_eq!((&merged.account_id, merged.timestamp), (&json!(12), 500));
        let ours: Value = serde_json::from_str(merged.item_sets[1].get()).unwrap();
        assert_eq!(ours["title"], "winer · 烬");

        // Another champion's set is added beside it, then clearing takes only winer's.
        let ashe = Build {
            champion_id: 22,
            ..jhin()
        };
        let more = merged.with(
            22,
            Mode::Ranked,
            item_set(&ashe, "艾希", Language::ZhCn),
            600,
        );
        assert_eq!(more.item_sets.len(), 3);
        let text = serde_json::to_string(&more).unwrap();
        assert!(text.contains(FOREIGN));
        let (cleared, removed) = more.without_ours(700);
        assert_eq!(removed, 2);
        assert_eq!(cleared.item_sets.len(), 1);
        assert_eq!(cleared.item_sets[0].get(), FOREIGN);
        let (unchanged, none) = cleared.without_ours(800);
        assert_eq!((none, unchanged.timestamp), (0, 700));
    }

    #[test]
    fn a_champion_keeps_one_of_winers_sets_per_map() {
        let uids = |sets: &ItemSets| -> Vec<String> {
            sets.item_sets.iter().filter_map(|set| uid(set)).collect()
        };
        let list = sets(&format!(
            r#"{{"accountId":"12","itemSets":[{{"uid":"winer-202-ranked"}},{FOREIGN},{{"uid":"winer-202-aram"}}],"timestamp":1}}"#
        ));
        // A normal game is played on the Rift too: its set replaces the ranked one, and goes last as
        // winer's newest.
        let normal = Build {
            mode: Mode::Normal,
            ..jhin()
        };
        let list = list.with(
            202,
            Mode::Normal,
            item_set(&normal, "烬", Language::ZhCn),
            2,
        );
        assert_eq!(uids(&list), ["a1b2", "winer-202-aram", "winer-202-normal"]);
        assert_eq!(
            list.account_id,
            json!("12"),
            "the account id goes back as it came"
        );
        // Hextech ARAM shares the Howling Abyss with ARAM; Arena has a map of its own.
        let hextech = Build {
            mode: Mode::Hextech,
            ..jhin()
        };
        let list = list.with(
            202,
            Mode::Hextech,
            item_set(&hextech, "烬", Language::ZhCn),
            3,
        );
        let arena = Build {
            mode: Mode::Arena,
            ..jhin()
        };
        let list = list.with(202, Mode::Arena, item_set(&arena, "烬", Language::ZhCn), 4);
        assert_eq!(
            uids(&list),
            [
                "a1b2",
                "winer-202-normal",
                "winer-202-hextech",
                "winer-202-arena"
            ]
        );
        assert_eq!(our_uid("winer-202-hextech"), Some((202, Mode::Hextech)));
        assert_eq!(our_uid("a1b2"), None);
        assert_eq!(our_uid("winer-x-ranked"), None);
    }

    /// winer's set for `champion` in a game of `mode`, as written.
    fn set_for(champion: i64, mode: Mode) -> Box<RawValue> {
        let build = Build {
            champion_id: champion,
            mode,
            ..jhin()
        };
        item_set(&build, "烬", Language::ZhCn)
    }

    fn hextech_set(champion: i64) -> Box<RawValue> {
        set_for(champion, Mode::Hextech)
    }

    fn body_len(sets: &ItemSets) -> usize {
        serde_json::to_vec(sets).unwrap().len()
    }

    #[test]
    fn winers_oldest_sets_make_room_for_its_newest() {
        // Measured on 2026-10-31: 58 of winer's Hextech sets filled the list to 64,881 bytes, and
        // the client refused the next write (HTTP 413).
        let mut list = sets(&format!(
            r#"{{"accountId":12,"itemSets":[{FOREIGN}],"timestamp":1}}"#
        ));
        for champion in 1..=58 {
            list = list.with(champion, Mode::Hextech, hextech_set(champion), 2);
        }
        let list = list.with(59, Mode::Hextech, hextech_set(59), 3);
        let uids: Vec<String> = list.item_sets.iter().filter_map(|set| uid(set)).collect();
        assert_eq!(uids.len(), 1 + MAX_OWN_ITEM_SETS);
        assert_eq!(
            list.item_sets[0].get(),
            FOREIGN,
            "the player's set stays first"
        );
        let newest: Vec<String> = (59 - MAX_OWN_ITEM_SETS as i64 + 1..=59)
            .map(|champion| item_set_uid(champion, Mode::Hextech))
            .collect();
        assert_eq!(uids[1..], newest[..], "the newest of winer's, oldest first");
        assert!(body_len(&list) <= ITEM_SETS_BUDGET);

        // Played again, a champion's set moves to the end and none of the others goes.
        let first_kept = 59 - MAX_OWN_ITEM_SETS as i64 + 1;
        let again = list.with(first_kept, Mode::Aram, set_for(first_kept, Mode::Aram), 4);
        let uids: Vec<String> = again.item_sets.iter().filter_map(|set| uid(set)).collect();
        assert_eq!(uids.len(), 1 + MAX_OWN_ITEM_SETS);
        assert_eq!(
            uids.last().map(String::as_str),
            Some(item_set_uid(first_kept, Mode::Aram).as_str())
        );
    }

    #[test]
    fn winers_sets_give_way_to_the_players_own_within_the_size_the_client_takes() {
        // The player's own sets take most of the room: winer keeps fewer of its own.
        let big = |n: usize| {
            format!(
                r#"{{"uid":"own-{n}","title":"{}","mode":"any","map":"any","type":"custom","sortrank":0,"startedFrom":"blank","associatedChampions":[],"associatedMaps":[],"blocks":[],"preferredItemSlots":[]}}"#,
                "x".repeat(4_900)
            )
        };
        let own: Vec<String> = (0..10).map(big).collect();
        let mut list = sets(&format!(
            r#"{{"accountId":12,"itemSets":[{}],"timestamp":1}}"#,
            own.join(",")
        ));
        for champion in 1..=15 {
            list = list.with(champion, Mode::Hextech, hextech_set(champion), 2);
        }
        assert!(body_len(&list) <= ITEM_SETS_BUDGET);
        let ours = list
            .item_sets
            .iter()
            .filter(|set| uid(set).is_some_and(|uid| uid.starts_with(ITEM_SET_PREFIX)))
            .count();
        assert!(ours < MAX_OWN_ITEM_SETS && ours > 0, "{ours} of winer's");
        assert_eq!(
            list.item_sets.iter().filter_map(|set| uid(set)).next_back(),
            Some(item_set_uid(15, Mode::Hextech))
        );
        for (index, set) in own.iter().enumerate() {
            assert_eq!(
                list.item_sets[index].get(),
                set,
                "the player's sets byte for byte"
            );
        }

        // Too full even for one: winer's newest set is kept and the client gets to say.
        let crowded: Vec<String> = (0..13).map(big).collect();
        let list = sets(&format!(
            r#"{{"accountId":12,"itemSets":[{}],"timestamp":1}}"#,
            crowded.join(",")
        ))
        .with(1, Mode::Hextech, hextech_set(1), 2)
        .with(2, Mode::Hextech, hextech_set(2), 3);
        assert_eq!(list.item_sets.len(), crowded.len() + 1);
        assert_eq!(
            list.item_sets.last().and_then(|set| uid(set)),
            Some(item_set_uid(2, Mode::Hextech))
        );
    }

    fn option(items: &[i64], pick: f64) -> ItemOption {
        ItemOption {
            items: items.to_vec(),
            rates: Rates {
                pick: Some(pick),
                ..Rates::default()
            },
        }
    }

    fn jhin() -> Build {
        let mut build = Build::new(BuildSource::Tencent, 202, Mode::Ranked);
        build.patch = "16.19".into();
        build.starting = vec![option(&[1055, 2003], 0.7), option(&[1086, 2003, 2003], 0.1)];
        build.boots = vec![option(&[3009], 0.9), option(&[3047], 0.06)];
        build.core = vec![
            option(&[6697, 6676, 3031], 0.2),
            option(&[6676, 3031, 3094], 0.07),
        ];
        build.late = vec![option(&[3036], 0.3), option(&[3094], 0.2)];
        build
    }

    /// Tencent 101's captured numbers for Jhin bottom, read as the panel reads them.
    fn tencent_jhin() -> Build {
        use crate::builds::{Known, StyleBook, TencentPayloads, parse_tencent, tencent_payload};
        let payload = |name: &str| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/builds")
                .join(name);
            tencent_payload(&fs::read(path).unwrap()).unwrap()
        };
        let items: Vec<crate::model::Item> = fixture("live/static/items.json");
        let known = Known {
            items: items.into_iter().map(|item| item.id).collect(),
            styles: StyleBook::new(&fixture("live/static/perkstyles.json")),
            ..Known::default()
        };
        let payloads = TencentPayloads {
            build: payload("tencent-build-202-bottom.json"),
            runeinfo: payload("tencent-runeinfo-202-bottom.json"),
            ..TencentPayloads::default()
        };
        parse_tencent(
            202,
            Mode::Ranked,
            Position::Bottom,
            "16.19",
            &payloads,
            &known,
        )
    }

    #[test]
    fn an_item_set_from_the_captured_numbers_has_the_four_blocks_in_order() {
        let set: Value =
            serde_json::from_str(item_set(&tencent_jhin(), "烬", Language::ZhCn).get()).unwrap();
        let blocks = set["blocks"].as_array().unwrap();
        let names: Vec<&str> = blocks
            .iter()
            .map(|block| block["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "起始装备 · 腾讯 101 16.19",
                "鞋子 · 腾讯 101 16.19",
                "核心装备 · 腾讯 101 16.19",
                "后期可选 · 腾讯 101 16.19"
            ]
        );
        let ids = |block: &Value| -> Vec<String> {
            block["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned())
                .collect()
        };
        // Doran's Blade and a potion, then the second start's Doran's Shield; two potions at most.
        assert_eq!(ids(&blocks[0]), ["1055", "2003", "1086"]);
        assert_eq!(blocks[0]["items"][1]["count"], 2);
        assert_eq!(ids(&blocks[1]), ["3009", "3047", "3006"]);
        assert_eq!(ids(&blocks[2]), ["6697", "6676", "3031", "3094"]);
        let late = ids(&blocks[3]);
        assert!(!late.is_empty() && late.len() <= 8);
        assert!(
            !late
                .iter()
                .any(|id| ["6697", "6676", "3031"].contains(&id.as_str())),
            "the core is not offered again: {late:?}"
        );
    }

    #[test]
    fn an_item_set_names_its_blocks_after_the_source_and_shows_on_the_modes_map() {
        let set: Value =
            serde_json::from_str(item_set(&jhin(), "烬", Language::ZhCn).get()).unwrap();
        assert_eq!(set["uid"], "winer-202-ranked");
        assert_eq!(set["associatedChampions"], json!([202]));
        assert_eq!(set["associatedMaps"], json!([11]));
        assert_eq!(set["type"], "custom");
        let blocks = set["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0]["type"], "起始装备 · 腾讯 101 16.19");
        // Two starts merged: the potions counted twice once, every item an id string.
        assert_eq!(
            blocks[0]["items"],
            json!([{"id": "1055", "count": 1}, {"id": "2003", "count": 2}, {"id": "1086", "count": 1}])
        );
        assert_eq!(
            blocks[2]["items"].as_array().unwrap().len(),
            4,
            "the core's items, each once"
        );
        let arena = Build {
            mode: Mode::Arena,
            source: BuildSource::OpGg,
            patch: String::new(),
            ..jhin()
        };
        let arena: Value =
            serde_json::from_str(item_set(&arena, "Jhin", Language::En).get()).unwrap();
        assert_eq!(arena["associatedMaps"], json!([30]));
        assert_eq!(arena["blocks"][1]["type"], "Boots · OP.GG");
        assert_eq!(item_set_uid(1, Mode::Hextech), "winer-1-hextech");
    }
}
