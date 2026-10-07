//! What players take on a champion, from public statistics: items, runes, summoner spells, skill
//! orders, matchups and augments, with how often each is taken and how it goes. One parser per
//! source turns its answer into one shape, [`Build`]; the fetch functions at the end are the only
//! network code. Every source is public, unauthenticated and read-only, and is sent nothing but the
//! champion, the lane and the patch asked about.
//!
//! Each field's meaning was read off the answers themselves (`fixtures/builds/`, Jhin, 16.19): a
//! share of games sums to about 100 over its rows, a win rate sits near 50.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{
    model::PerkStyles,
    net,
    settings::{BuildSettings, Mode, ModeSource, RiftSource},
    view::{Position, Rarity},
};

/// The documented per-request bound for these hosts, as for ARAM.GG's descriptions.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Where a build's numbers come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BuildSource {
    /// 腾讯 101: the Tencent shards' own statistics, by lane and patch.
    Tencent,
    /// Tencent's Hextech ARAM statistics: items, skill orders and augments.
    TencentHextech,
    /// OP.GG's global statistics.
    OpGg,
    /// ARAM.GG's Hextech ARAM augment statistics.
    AramGg,
}

/// Where the numbers for a kind of game come from, in the order to try them: the next one only
/// when the one before fails. Other modes have none, and neither has a mode whose source the
/// settings switch off.
pub fn sources(mode: Mode, settings: &BuildSettings) -> &'static [BuildSource] {
    let one = |source: ModeSource| -> &'static [BuildSource] {
        match source {
            ModeSource::OpGg => &[BuildSource::OpGg],
            ModeSource::Off => &[],
        }
    };
    match mode {
        Mode::Ranked | Mode::Normal => match settings.rift_source {
            RiftSource::Tencent => &[BuildSource::Tencent],
            RiftSource::OpGg => &[BuildSource::OpGg],
        },
        Mode::Aram => one(settings.aram_source),
        Mode::Arena => one(settings.arena_source),
        Mode::Hextech if settings.hextech_fallback => {
            &[BuildSource::TencentHextech, BuildSource::AramGg]
        }
        Mode::Hextech => &[BuildSource::TencentHextech],
        Mode::Other => &[],
    }
}

/// How often an option was taken and how it went, as far as its source says.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Rates {
    /// The share of the champion's games that took it, 0–1.
    pub pick: Option<f64>,
    /// The share of those games won, 0–1; in Arena, finished in the top four.
    pub win: Option<f64>,
    /// The games behind the two.
    pub games: Option<i64>,
    /// Arena: the average finish, 1 (first) to 8.
    pub placement: Option<f64>,
    /// Arena: the share of games finished first, 0–1.
    pub first: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpellOption {
    pub spells: [i64; 2],
    pub rates: Rates,
}

/// A rune page's choices: its two styles and nine runes in page order (the keystone, three of the
/// primary style, two of the secondary, three shards).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunePage {
    pub primary_style: i64,
    pub sub_style: i64,
    pub perks: Vec<i64>,
}

impl RunePage {
    /// Two different styles and nine runes: what the client takes as a page.
    pub fn is_complete(&self) -> bool {
        self.primary_style > 0
            && self.sub_style > 0
            && self.primary_style != self.sub_style
            && self.perks.len() == 9
            && self.perks.iter().all(|&perk| perk > 0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuneOption {
    pub page: RunePage,
    pub rates: Rates,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ItemOption {
    /// One item, or several taken together; a repeated id is bought more than once.
    pub items: Vec<i64>,
    pub rates: Rates,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, TS)]
pub enum Ability {
    Q,
    W,
    E,
    R,
}

impl Ability {
    /// Tencent numbers them 1 to 4, OP.GG writes their letters.
    fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_uppercase().as_str() {
            "1" | "Q" => Self::Q,
            "2" | "W" => Self::W,
            "3" | "E" => Self::E,
            "4" | "R" => Self::R,
            _ => return None,
        })
    }
}

/// The order abilities are maxed in, with the levels most often taken that way.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrder {
    /// The basic abilities, maxed first to last.
    pub priority: Vec<Ability>,
    /// The ability taken at each level, from the first.
    pub sequence: Vec<Ability>,
    pub rates: Rates,
}

/// An opponent, with the champion's own win rate against it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Matchup {
    pub champion_id: i64,
    pub rates: Rates,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Matchups {
    /// The opponents the champion beats most often, best first.
    pub good: Vec<Matchup>,
    /// The ones it loses to most often, worst first.
    pub bad: Vec<Matchup>,
}

/// A source's grade for an augment on this champion, S the best.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS)]
pub enum AugmentTier {
    S,
    A,
    B,
    C,
}

impl AugmentTier {
    fn parse(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "S" | "1" => Self::S,
            "A" | "2" => Self::A,
            "B" | "3" => Self::B,
            "C" | "4" => Self::C,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AugmentOption {
    pub id: i64,
    /// From the client's own catalog.
    pub rarity: Rarity,
    pub tier: Option<AugmentTier>,
    pub rates: Rates,
}

/// What players take on one champion in one kind of game, from one source.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub source: BuildSource,
    /// The patch the numbers are from, `16.19`; empty where the source does not say.
    pub patch: String,
    pub champion_id: i64,
    pub mode: Mode,
    /// The lane the numbers are for, on the Rift.
    pub lane: Option<Position>,
    /// The champion's standing on the source's own scale, 1 the best (OP.GG: 1 to 5).
    pub tier: Option<u8>,
    /// The games behind the numbers.
    pub sample: Option<i64>,
    pub spells: Vec<SpellOption>,
    pub runes: Vec<RuneOption>,
    pub starting: Vec<ItemOption>,
    pub boots: Vec<ItemOption>,
    pub core: Vec<ItemOption>,
    /// Single items for the slots after the core, most taken first.
    pub late: Vec<ItemOption>,
    pub skill_orders: Vec<SkillOrder>,
    pub matchups: Matchups,
    /// Best first.
    pub augments: Vec<AugmentOption>,
    /// Ids the client cannot name (an item of another patch, an unknown augment, a rune of no
    /// style), left out.
    pub dropped: u32,
}

impl Build {
    pub(crate) fn new(source: BuildSource, champion_id: i64, mode: Mode) -> Self {
        Self {
            source,
            patch: String::new(),
            champion_id,
            mode,
            lane: None,
            tier: None,
            sample: None,
            spells: Vec::new(),
            runes: Vec::new(),
            starting: Vec::new(),
            boots: Vec::new(),
            core: Vec::new(),
            late: Vec::new(),
            skill_orders: Vec::new(),
            matchups: Matchups::default(),
            augments: Vec::new(),
            dropped: 0,
        }
    }

    /// Nothing to show: a source that answers with no rows has not counted this champion.
    pub fn is_empty(&self) -> bool {
        self.spells.is_empty()
            && self.runes.is_empty()
            && self.starting.is_empty()
            && self.boots.is_empty()
            && self.core.is_empty()
            && self.late.is_empty()
            && self.skill_orders.is_empty()
            && self.augments.is_empty()
    }
}

/// What the client knows, which a build is checked against.
#[derive(Clone, Debug, Default)]
pub struct Known {
    pub items: HashSet<i64>,
    /// Arena's and Hextech ARAM's augments, with their rarity.
    pub augments: HashMap<i64, Rarity>,
    pub styles: StyleBook,
}

impl Known {
    /// Drops the items the client cannot name from every option, and an option left empty by
    /// that; counts what went.
    fn keep_known_items(&self, build: &mut Build) {
        let mut dropped = 0;
        for list in [
            &mut build.starting,
            &mut build.boots,
            &mut build.core,
            &mut build.late,
        ] {
            for option in list.iter_mut() {
                let before = option.items.len();
                option.items.retain(|id| self.items.contains(id));
                dropped += before - option.items.len();
            }
            list.retain(|option| !option.items.is_empty());
        }
        build.dropped += dropped as u32;
    }

    /// The augment `id` as the client knows it, rarity and all.
    fn augment(&self, id: i64, tier: Option<AugmentTier>, rates: Rates) -> Option<AugmentOption> {
        let rarity = *self.augments.get(&id)?;
        Some(AugmentOption {
            id,
            rarity,
            tier,
            rates,
        })
    }
}

/// Which style each rune belongs to, from the client's own `perkstyles.json`. Shards belong to
/// every style alike and are left out.
#[derive(Clone, Debug, Default)]
pub struct StyleBook {
    styles: HashMap<i64, i64>,
}

impl StyleBook {
    pub fn new(styles: &PerkStyles) -> Self {
        let mut book = HashMap::new();
        for style in &styles.styles {
            for slot in style.slots.iter().filter(|slot| slot.kind != "kStatMod") {
                for &perk in &slot.perks {
                    book.insert(perk, style.id);
                }
            }
        }
        Self { styles: book }
    }

    /// The page of nine runes in page order, with its styles: the keystone's, and the one both
    /// secondary runes share.
    pub fn page(&self, perks: Vec<i64>) -> Option<RunePage> {
        if perks.len() != 9 {
            return None;
        }
        let primary = *self.styles.get(&perks[0])?;
        let sub = *self.styles.get(&perks[4])?;
        if self.styles.get(&perks[5]) != Some(&sub) {
            return None;
        }
        let page = RunePage {
            primary_style: primary,
            sub_style: sub,
            perks,
        };
        page.is_complete().then_some(page)
    }
}

// Shared row parsing.

fn number(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn integer(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

/// `74.5` → 0.745: Tencent's 101 writes shares in percent.
fn percent(text: &str) -> Option<f64> {
    number(text).map(|value| value / 100.0)
}

fn ids(text: &str, separator: char) -> Vec<i64> {
    text.split(separator).filter_map(integer).collect()
}

/// A number the source writes as `4788` in one field and `"4788"` in the next.
fn loose_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => number(text),
        _ => None,
    }
}

/// A share in hundredths of a percent, `4788` → 0.4788.
fn basis_points(value: &Value) -> Option<f64> {
    loose_number(value).map(|value| value / 10_000.0)
}

fn abilities(text: &str, separator: char) -> Vec<Ability> {
    text.split(separator).filter_map(Ability::parse).collect()
}

fn share(part: i64, whole: i64) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

/// Lanes in each source's words.
fn tencent_lane(lane: Position) -> &'static str {
    match lane {
        Position::Top => "TOP",
        Position::Jungle => "JUNGLE",
        Position::Middle => "MIDDLE",
        Position::Bottom => "BOTTOM",
        Position::Utility => "SUPPORT",
    }
}

fn opgg_position(lane: Position) -> &'static str {
    match lane {
        Position::Top => "top",
        Position::Jungle => "jungle",
        Position::Middle => "mid",
        Position::Bottom => "adc",
        Position::Utility => "support",
    }
}

fn opgg_lane(name: &str) -> Option<Position> {
    match name.to_ascii_lowercase().as_str() {
        "adc" => Some(Position::Bottom),
        other => Position::parse(other),
    }
}

// Tencent (mlol.qt.qq.com).

/// A Tencent answer's payload: one field of an unpredictable name (`R18087`) holding a JSON
/// document as a string, whose own fields are strings again. `None` when the string is empty,
/// which is how a lane or patch with no numbers answers.
pub fn tencent_payload(body: &[u8]) -> Result<Option<HashMap<String, String>>, String> {
    let envelope: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    if let Some(code) = envelope.get("code").and_then(Value::as_i64)
        && code != 0
    {
        let message = ["message", "msg", "errMsg"]
            .into_iter()
            .find_map(|key| envelope.get(key)?.as_str().filter(|text| !text.is_empty()))
            .unwrap_or("no message");
        return Err(format!("the server answered code {code}: {message}"));
    }
    let text = envelope
        .pointer("/data/_fieldValues")
        .and_then(Value::as_object)
        .and_then(|fields| fields.values().find_map(Value::as_str))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(None);
    }
    let fields: HashMap<String, Value> =
        serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(Some(
        fields
            .into_iter()
            .filter_map(|(key, value)| match value {
                Value::String(text) => Some((key, text)),
                _ => None,
            })
            .collect(),
    ))
}

/// Tencent's patches, newest first: `16.19`, `16.18`, …
pub fn tencent_patches(body: &[u8]) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct List {
        #[serde(default)]
        data: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        #[serde(default)]
        name: String,
    }
    let list: List = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let patches: Vec<String> = list
        .data
        .into_iter()
        .map(|entry| entry.name)
        .filter(|name| is_patch(name))
        .collect();
    if patches.is_empty() {
        return Err("the patch list is empty".into());
    }
    Ok(patches)
}

/// `16.19`, and nothing that could change the meaning of the URL it goes into.
fn is_patch(name: &str) -> bool {
    let mut parts = name.split('.');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    };
    numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none()
}

/// One rune row of 101's `rune_top_details`.
#[derive(Clone, Debug, PartialEq)]
struct RuneRow {
    rank: i64,
    perks: Vec<i64>,
    rates: Rates,
}

/// `<rank>_<keystone>_<abbreviation>_<nine runes>_<pick %>_<win %>_<games>`, `#`-joined. The
/// games are the row's own: dividing them by its pick share gives the same total for every row of
/// a lane (1 241 522 for Jhin bottom), which is the lane's sample.
fn rune_rows(text: &str) -> Vec<RuneRow> {
    let mut rows: Vec<RuneRow> = text
        .split('#')
        .filter_map(|row| {
            let fields: Vec<&str> = row.split('_').collect();
            if fields.len() < 7 {
                return None;
            }
            Some(RuneRow {
                rank: integer(fields[0])?,
                perks: ids(fields[3], ','),
                rates: Rates {
                    pick: percent(fields[4]),
                    win: percent(fields[5]),
                    games: integer(fields[6]),
                    ..Rates::default()
                },
            })
        })
        .collect();
    rows.sort_by_key(|row| row.rank);
    rows
}

/// A lane's games, from the row counted most often.
fn lane_sample(rows: &[RuneRow]) -> Option<i64> {
    rows.iter()
        .filter_map(|row| Some((row.rates.games?, row.rates.pick?)))
        .filter(|&(_, pick)| pick > 0.0)
        .max_by_key(|&(games, _)| games)
        .map(|(games, pick)| (games as f64 / pick).round() as i64)
}

/// The rune rows of one lane's `runeinfo` payload.
pub fn tencent_rune_rows(payload: &HashMap<String, String>) -> usize {
    payload
        .get("rune_top_details")
        .map_or(0, |text| rune_rows(text).len())
}

/// The lane a champion is played in most, by the games behind each lane's rune rows; `None`
/// when no lane has any.
pub fn busiest_lane(lanes: &[(Position, Option<HashMap<String, String>>)]) -> Option<Position> {
    lanes
        .iter()
        .filter_map(|(lane, payload)| {
            let rows = rune_rows(payload.as_ref()?.get("rune_top_details")?);
            let games: i64 = rows.iter().filter_map(|row| row.rates.games).sum();
            (games > 0).then_some((games, *lane))
        })
        .max_by_key(|&(games, _)| games)
        .map(|(_, lane)| lane)
}

/// `<ids>_<rank>_<pick %>_<win %>`, `#`-joined: 101's item rows.
fn item_rows(text: &str) -> Vec<ItemOption> {
    let mut rows: Vec<(i64, ItemOption)> = text
        .split('#')
        .filter_map(|row| {
            let fields: Vec<&str> = row.split('_').collect();
            if fields.len() < 4 {
                return None;
            }
            Some((
                integer(fields[1])?,
                ItemOption {
                    items: ids(fields[0], ','),
                    rates: Rates {
                        pick: percent(fields[2]),
                        win: percent(fields[3]),
                        ..Rates::default()
                    },
                },
            ))
        })
        .collect();
    rows.sort_by_key(|(rank, _)| *rank);
    rows.into_iter().map(|(_, option)| option).collect()
}

/// Options of one item each, in order, each item once: the first time it is listed wins.
fn single_items(
    lists: impl IntoIterator<Item = ItemOption>,
    skip: &HashSet<i64>,
) -> Vec<ItemOption> {
    let mut seen = skip.clone();
    lists
        .into_iter()
        .filter(|option| option.items.len() == 1 && seen.insert(option.items[0]))
        .collect()
}

/// The five 101 payloads of one champion, lane and patch.
#[derive(Clone, Debug, Default)]
pub struct TencentPayloads {
    pub build: Option<HashMap<String, String>>,
    pub runeinfo: Option<HashMap<String, String>>,
    pub spells: Option<HashMap<String, String>>,
    pub skills: Option<HashMap<String, String>>,
    pub matchups: Option<HashMap<String, String>>,
}

/// 101's numbers for one champion in one lane.
pub fn parse_tencent(
    champion_id: i64,
    mode: Mode,
    lane: Position,
    patch: &str,
    payloads: &TencentPayloads,
    known: &Known,
) -> Build {
    let mut build = Build::new(BuildSource::Tencent, champion_id, mode);
    build.patch = patch.to_owned();
    build.lane = Some(lane);
    let field = |payload: &Option<HashMap<String, String>>, key: &str| {
        payload
            .as_ref()
            .and_then(|payload| payload.get(key))
            .cloned()
            .unwrap_or_default()
    };

    let rows = rune_rows(&field(&payloads.runeinfo, "rune_top_details"));
    build.sample = lane_sample(&rows);
    for row in rows {
        match known.styles.page(row.perks) {
            Some(page) => build.runes.push(RuneOption {
                page,
                rates: row.rates,
            }),
            None => build.dropped += 1,
        }
    }

    build.starting = item_rows(&field(&payloads.build, "starting_details"));
    build.boots = item_rows(&field(&payloads.build, "shoes_details"));
    build.core = item_rows(&field(&payloads.build, "core_details"));
    let core: HashSet<i64> = build
        .core
        .first()
        .map(|option| option.items.iter().copied().collect())
        .unwrap_or_default();
    build.late = single_items(
        ["forth_details", "fifth_details", "sixth_details"]
            .into_iter()
            .flat_map(|key| item_rows(&field(&payloads.build, key))),
        &core,
    );

    // `<spell>_<spell>_<win %>_<pick %>`: the shares come last here, unlike the item rows (they sum
    // to 99.7 for Jhin bottom; the column before sits at 44–50).
    let mut spells: Vec<SpellOption> = field(&payloads.spells, "data_details")
        .split('#')
        .filter_map(|row| {
            let fields: Vec<&str> = row.split('_').collect();
            if fields.len() < 4 {
                return None;
            }
            Some(SpellOption {
                spells: [integer(fields[0])?, integer(fields[1])?],
                rates: Rates {
                    win: percent(fields[2]),
                    pick: percent(fields[3]),
                    ..Rates::default()
                },
            })
        })
        .collect();
    spells.sort_by(|a, b| {
        b.rates
            .pick
            .partial_cmp(&a.rates.pick)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    build.spells = spells;

    // `<max order>:<pick %>:<win %>` then `@<15 levels>_<pick %>_<win %>`…, groups `$`-joined.
    build.skill_orders = field(&payloads.skills, "detaildetails")
        .split('$')
        .filter_map(|group| {
            let mut parts = group.split('@');
            let head: Vec<&str> = parts.next()?.split(':').collect();
            if head.len() < 3 {
                return None;
            }
            let sequence = parts
                .filter_map(|row| {
                    let fields: Vec<&str> = row.split('_').collect();
                    Some((number(fields.get(1)?)?, abilities(fields[0], ',')))
                })
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, sequence)| sequence)
                .unwrap_or_default();
            Some(SkillOrder {
                priority: abilities(head[0], ','),
                sequence,
                rates: Rates {
                    pick: percent(head[1]),
                    win: percent(head[2]),
                    ..Rates::default()
                },
            })
        })
        .filter(|order| !order.priority.is_empty())
        .collect();

    // `<rank>_<champion>_<win %>_<…>`: the champion's own win rate against the opponent, 54 % at
    // the top of `high_op_details` for Jhin bottom and 44 % of `low_op_details`. OP.GG's counters
    // agree (Ezreal 54.3 % and 54.1 %). The last column is not read: what it measures is unknown.
    let matchups = |key: &str| -> Vec<Matchup> {
        let mut rows: Vec<(i64, Matchup)> = field(&payloads.matchups, key)
            .split('#')
            .filter_map(|row| {
                let fields: Vec<&str> = row.split('_').collect();
                if fields.len() < 3 {
                    return None;
                }
                Some((
                    integer(fields[0])?,
                    Matchup {
                        champion_id: integer(fields[1]).filter(|&id| id > 0)?,
                        rates: Rates {
                            win: percent(fields[2]),
                            ..Rates::default()
                        },
                    },
                ))
            })
            .collect();
        rows.sort_by_key(|(rank, _)| *rank);
        rows.into_iter().map(|(_, matchup)| matchup).collect()
    };
    build.matchups = Matchups {
        good: matchups("high_op_details"),
        bad: matchups("low_op_details"),
    };

    known.keep_known_items(&mut build);
    build
}

/// Tencent's Hextech ARAM numbers for one champion (`fuwen_hero_rank`).
pub fn parse_tencent_hextech(
    champion_id: i64,
    payload: &HashMap<String, String>,
    known: &Known,
) -> Build {
    let mut build = Build::new(BuildSource::TencentHextech, champion_id, Mode::Hextech);
    let field = |key: &str| payload.get(key).map(String::as_str).unwrap_or_default();
    // `<ids>$<pick 0–1>$<win 0–1>`, `#`-joined.
    let rows = |key: &str| -> Vec<ItemOption> {
        field(key)
            .split('#')
            .filter_map(|row| {
                let fields: Vec<&str> = row.split('$').collect();
                if fields.len() < 3 {
                    return None;
                }
                Some(ItemOption {
                    items: ids(fields[0], ','),
                    rates: Rates {
                        pick: number(fields[1]),
                        win: number(fields[2]),
                        ..Rates::default()
                    },
                })
            })
            .filter(|option| !option.items.is_empty())
            .collect()
    };
    let by_pick = |options: &mut Vec<ItemOption>| {
        options.sort_by(|a, b| {
            b.rates
                .pick
                .partial_cmp(&a.rates.pick)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    };
    build.starting = rows("itemout");
    by_pick(&mut build.starting);
    build.boots = rows("itemshoes");
    by_pick(&mut build.boots);

    // JSON objects keyed by rank: `{"1": {"itemcore": "6676&3031&3036", "winrate": 4874,
    // "showrate": 1725}}`, both rates in hundredths of a percent.
    let ranked = |key: &str| -> Vec<Value> {
        let map: HashMap<String, Value> = serde_json::from_str(field(key)).unwrap_or_default();
        let mut entries: Vec<(i64, Value)> = map
            .into_iter()
            .filter_map(|(rank, value)| Some((integer(&rank)?, value)))
            .collect();
        entries.sort_by_key(|(rank, _)| *rank);
        entries.into_iter().map(|(_, value)| value).collect()
    };
    let item_option = |entry: &Value, key: &str| ItemOption {
        items: ids(
            entry.get(key).and_then(Value::as_str).unwrap_or_default(),
            '&',
        ),
        rates: Rates {
            pick: entry.get("showrate").and_then(basis_points),
            win: entry.get("winrate").and_then(basis_points),
            ..Rates::default()
        },
    };
    build.core = ranked("itemcore_json")
        .iter()
        .map(|entry| item_option(entry, "itemcore"))
        .filter(|option| !option.items.is_empty())
        .collect();
    let mut taken: HashSet<i64> = build
        .core
        .first()
        .map(|option| option.items.iter().copied().collect())
        .unwrap_or_default();
    taken.extend(
        build
            .boots
            .iter()
            .flat_map(|option| option.items.iter().copied()),
    );
    build.late = single_items(
        ranked("itemone_json")
            .iter()
            .map(|entry| item_option(entry, "itemone")),
        &taken,
    );

    build.skill_orders = ranked("skill_json")
        .iter()
        .filter_map(|entry| {
            let priority = abilities(entry.get("qwe")?.as_str()?, '&');
            let sequences: HashMap<String, Value> = entry
                .get("sks")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default();
            let sequence = sequences
                .values()
                .filter_map(|value| {
                    Some((
                        value.get("sk_s").and_then(loose_number)?,
                        abilities(value.get("sk")?.as_str()?, '&'),
                    ))
                })
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, sequence)| sequence)
                .unwrap_or_default();
            (!priority.is_empty()).then(|| SkillOrder {
                priority,
                sequence,
                rates: Rates {
                    pick: entry.get("qwe_s").and_then(basis_points),
                    win: entry.get("qwe_w").and_then(basis_points),
                    ..Rates::default()
                },
            })
        })
        .collect();

    // `255:` then `<rank>|<champion>|<augment>|255|<pick 0–1>|<S|A|B|C>`, `#`-joined, all
    // rarities in one list; `&kGold:…` and the others repeat the same rows by rarity. The rank runs
    // through the tiers, best first.
    let mut augments: Vec<(i64, AugmentOption)> = Vec::new();
    let list = field("augment_json_irank");
    let all = list
        .split('&')
        .find_map(|segment| segment.strip_prefix("255:"))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            list.split('&')
                .filter_map(|segment| Some(segment.split_once(':')?.1))
                .collect::<Vec<_>>()
                .join("#")
        });
    let mut seen = HashSet::new();
    for row in all.split('#') {
        let fields: Vec<&str> = row.split('|').collect();
        if fields.len() < 6 {
            continue;
        }
        let (Some(rank), Some(id)) = (integer(fields[0]), integer(fields[2])) else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let rates = Rates {
            pick: number(fields[4]),
            ..Rates::default()
        };
        match known.augment(id, AugmentTier::parse(fields[5]), rates) {
            Some(augment) => augments.push((rank, augment)),
            None => build.dropped += 1,
        }
    }
    augments.sort_by_key(|(rank, augment)| (augment.tier, *rank));
    build.augments = augments.into_iter().map(|(_, augment)| augment).collect();

    known.keep_known_items(&mut build);
    build
}

// OP.GG (lol-api-champion.op.gg).

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggAnswer {
    data: OpggData,
    meta: OpggMeta,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggMeta {
    version: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggData {
    summary: OpggSummary,
    summoner_spells: Vec<OpggRow>,
    runes: Vec<OpggRunes>,
    starter_items: Vec<OpggRow>,
    boots: Vec<OpggRow>,
    core_items: Vec<OpggRow>,
    last_items: Vec<OpggRow>,
    skill_masteries: Vec<OpggMastery>,
    counters: Vec<OpggCounter>,
    augment_group: Vec<OpggAugments>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggSummary {
    average_stats: OpggStats,
    positions: Option<Vec<OpggPosition>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggPosition {
    name: String,
    stats: OpggStats,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggStats {
    play: Option<i64>,
    role_rate: Option<f64>,
    tier_data: Option<OpggTier>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggTier {
    tier: Option<u8>,
}

/// One row of a list: what was taken, in how many games, how many won.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggRow {
    ids: Vec<i64>,
    play: i64,
    win: i64,
    pick_rate: Option<f64>,
    total_place: Option<i64>,
    first_place: Option<i64>,
}

impl OpggRow {
    fn rates(&self) -> Rates {
        rates(
            self.play,
            self.win,
            self.pick_rate,
            self.total_place,
            self.first_place,
        )
    }

    fn items(&self) -> ItemOption {
        ItemOption {
            items: self.ids.clone(),
            rates: self.rates(),
        }
    }
}

/// OP.GG's counts as rates. Arena's `total_place` counts finishes from 0: read from 1, Jhin's
/// average would be 3.55 of 8 while only 48.6 % of his games finish in the top four; from 0 it is
/// 4.55, just below the middle, as that share says (`fixtures/builds/opgg-arena-202.json`).
fn rates(
    play: i64,
    win: i64,
    pick: Option<f64>,
    total_place: Option<i64>,
    first_place: Option<i64>,
) -> Rates {
    Rates {
        pick,
        win: share(win, play),
        games: (play > 0).then_some(play),
        placement: total_place
            .and_then(|total| share(total, play))
            .map(|average| average + 1.0),
        first: first_place.and_then(|first| share(first, play)),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggRunes {
    primary_page_id: i64,
    primary_rune_ids: Vec<i64>,
    secondary_page_id: i64,
    secondary_rune_ids: Vec<i64>,
    stat_mod_ids: Vec<i64>,
    play: i64,
    win: i64,
    pick_rate: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggMastery {
    ids: Vec<String>,
    play: i64,
    win: i64,
    pick_rate: Option<f64>,
    total_place: Option<i64>,
    first_place: Option<i64>,
    builds: Vec<OpggSkills>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggSkills {
    order: Vec<String>,
}

/// The champion's own wins against the opponent: Jhin's 11 330 of 20 872 against Ezreal (54.3 %)
/// is the 54.1 % Tencent shows for the same matchup.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggCounter {
    champion_id: i64,
    play: i64,
    win: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggAugments {
    augments: Vec<OpggAugment>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpggAugment {
    id: i64,
    play: i64,
    win: i64,
    pick_rate: Option<f64>,
    total_place: Option<i64>,
    first_place: Option<i64>,
}

/// How many matchups to name on each side.
const MATCHUPS: usize = 5;

/// OP.GG's mode in its URLs: it counts ranked games only, which stand in for normal ones.
fn opgg_mode(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Ranked | Mode::Normal => Some("ranked"),
        Mode::Aram => Some("aram"),
        Mode::Arena => Some("arena"),
        Mode::Hextech | Mode::Other => None,
    }
}

/// The lanes OP.GG says the champion is played in, the main one first. Every answer carries
/// them, whichever lane was asked for: an off-lane answer (Jhin top) still names his bottom lane.
pub fn opgg_lanes(body: &[u8]) -> Result<Vec<Position>, String> {
    let answer: OpggAnswer = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let mut positions: Vec<(f64, Position)> = answer
        .data
        .summary
        .positions
        .unwrap_or_default()
        .iter()
        .filter_map(|position| {
            Some((
                position.stats.role_rate.unwrap_or_default(),
                opgg_lane(&position.name)?,
            ))
        })
        .collect();
    positions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(positions.into_iter().map(|(_, lane)| lane).collect())
}

/// OP.GG's numbers for one champion in one mode (and lane, on the Rift).
pub fn parse_opgg(
    champion_id: i64,
    mode: Mode,
    lane: Option<Position>,
    body: &[u8],
    known: &Known,
) -> Result<Build, String> {
    let answer: OpggAnswer = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let data = answer.data;
    let mut build = Build::new(BuildSource::OpGg, champion_id, mode);
    build.patch = answer.meta.version;
    build.lane = lane;

    // The lane's own standing where the summary lists it, else the champion's over all lanes.
    let position = lane.and_then(|lane| {
        data.summary
            .positions
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|position| opgg_lane(&position.name) == Some(lane))
    });
    let stats = position.map_or(&data.summary.average_stats, |position| &position.stats);
    build.tier = stats
        .tier_data
        .as_ref()
        .and_then(|tier| tier.tier)
        .or_else(|| data.summary.average_stats.tier_data.as_ref()?.tier);
    build.sample = stats.play.or(data.summary.average_stats.play);

    build.spells = data
        .summoner_spells
        .iter()
        .filter(|row| row.ids.len() == 2)
        .map(|row| SpellOption {
            spells: [row.ids[0], row.ids[1]],
            rates: row.rates(),
        })
        .collect();
    for runes in &data.runes {
        let perks: Vec<i64> = runes
            .primary_rune_ids
            .iter()
            .chain(&runes.secondary_rune_ids)
            .chain(&runes.stat_mod_ids)
            .copied()
            .collect();
        let page = RunePage {
            primary_style: runes.primary_page_id,
            sub_style: runes.secondary_page_id,
            perks,
        };
        if page.is_complete() {
            build.runes.push(RuneOption {
                page,
                rates: rates(runes.play, runes.win, runes.pick_rate, None, None),
            });
        } else {
            build.dropped += 1;
        }
    }
    build.starting = data.starter_items.iter().map(OpggRow::items).collect();
    build.boots = data.boots.iter().map(OpggRow::items).collect();
    build.core = data.core_items.iter().map(OpggRow::items).collect();
    let mut taken: HashSet<i64> = build
        .core
        .first()
        .map(|option| option.items.iter().copied().collect())
        .unwrap_or_default();
    taken.extend(
        build
            .boots
            .iter()
            .flat_map(|option| option.items.iter().copied()),
    );
    build.late = single_items(data.last_items.iter().map(OpggRow::items), &taken);

    build.skill_orders = data
        .skill_masteries
        .iter()
        .map(|mastery| SkillOrder {
            priority: mastery
                .ids
                .iter()
                .filter_map(|id| Ability::parse(id))
                .collect(),
            sequence: mastery
                .builds
                .first()
                .map(|skills| {
                    skills
                        .order
                        .iter()
                        .filter_map(|id| Ability::parse(id))
                        .collect()
                })
                .unwrap_or_default(),
            rates: rates(
                mastery.play,
                mastery.win,
                mastery.pick_rate,
                mastery.total_place,
                mastery.first_place,
            ),
        })
        .filter(|order| !order.priority.is_empty())
        .collect();

    // Matchups with a fair share of the games: a handful of games is noise, whichever way it went.
    let games: i64 = data.counters.iter().map(|counter| counter.play).sum();
    let mut counters: Vec<Matchup> = data
        .counters
        .iter()
        .filter(|counter| counter.champion_id > 0 && counter.play * 200 >= games)
        .map(|counter| Matchup {
            champion_id: counter.champion_id,
            rates: rates(counter.play, counter.win, None, None, None),
        })
        .collect();
    counters.sort_by(|a, b| {
        b.rates
            .win
            .partial_cmp(&a.rates.win)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    build.matchups = Matchups {
        good: counters
            .iter()
            .filter(|matchup| matchup.rates.win.is_some_and(|win| win > 0.5))
            .take(MATCHUPS)
            .cloned()
            .collect(),
        bad: counters
            .iter()
            .rev()
            .filter(|matchup| matchup.rates.win.is_some_and(|win| win < 0.5))
            .take(MATCHUPS)
            .cloned()
            .collect(),
    };

    // Arena: judged by the average finish and then by first places, not by "wins".
    for augment in data.augment_group.iter().flat_map(|group| &group.augments) {
        let rates = rates(
            augment.play,
            augment.win,
            augment.pick_rate,
            augment.total_place,
            augment.first_place,
        );
        match known.augment(augment.id, None, rates) {
            Some(augment) => build.augments.push(augment),
            None => build.dropped += 1,
        }
    }
    build.augments.sort_by(|a, b| {
        let key = |augment: &AugmentOption| {
            (
                augment.rates.placement.unwrap_or(f64::MAX),
                -augment.rates.first.unwrap_or_default(),
            )
        };
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    known.keep_known_items(&mut build);
    Ok(build)
}

// ARAM.GG (aramgg.com).

/// ARAM.GG's Hextech ARAM augments for one champion: `[[champion, "<document>", patch, date]]`,
/// the document holding each augment's tier (1 the best, the same four as Tencent's S to C: its
/// rows name Tencent as their source) and rank.
pub fn parse_aramgg(champion_id: i64, body: &[u8], known: &Known) -> Result<Build, String> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Document {
        augments: HashMap<String, Entry>,
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Entry {
        tier: Value,
        rank: Value,
        win_rate: Value,
        pick_rate: Value,
        num_games: Value,
    }
    let rows: Vec<Vec<Value>> = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let row = rows
        .into_iter()
        .find(|row| {
            row.first()
                .and_then(loose_number)
                .is_some_and(|id| id as i64 == champion_id)
        })
        .ok_or_else(|| format!("no numbers for champion {champion_id}"))?;
    let document: Document = row
        .get(1)
        .and_then(Value::as_str)
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let mut build = Build::new(BuildSource::AramGg, champion_id, Mode::Hextech);
    build.patch = row
        .get(2)
        .and_then(Value::as_str)
        .filter(|patch| is_patch(patch))
        .unwrap_or_default()
        .to_owned();
    let mut augments: Vec<(i64, AugmentOption)> = Vec::new();
    for (id, entry) in document.augments {
        let Some(id) = integer(&id) else {
            continue;
        };
        let tier = match &entry.tier {
            Value::String(text) => AugmentTier::parse(text),
            Value::Number(number) => AugmentTier::parse(&number.to_string()),
            _ => None,
        };
        let rates = Rates {
            pick: loose_number(&entry.pick_rate),
            win: loose_number(&entry.win_rate),
            games: loose_number(&entry.num_games).map(|games| games as i64),
            ..Rates::default()
        };
        let rank = loose_number(&entry.rank).map_or(i64::MAX, |rank| rank as i64);
        match known.augment(id, tier, rates) {
            Some(augment) => augments.push((rank, augment)),
            None => build.dropped += 1,
        }
    }
    augments.sort_by_key(|(rank, augment)| (augment.tier, *rank));
    build.augments = augments.into_iter().map(|(_, augment)| augment).collect();
    Ok(build)
}

// The network.

async fn get(url: &str) -> Result<Vec<u8>, String> {
    let response = net::client()?
        .get(url)
        .header(ACCEPT, "application/json")
        .timeout(TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    // Through serde_json, not reqwest's `json` feature: that one is only on when the whole
    // workspace builds together.
    Ok(response
        .bytes()
        .await
        .map_err(|error| error.to_string())?
        .to_vec())
}

/// What one champion, mode and lane is asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Query {
    pub champion_id: i64,
    pub mode: Mode,
    /// On the Rift; `None` picks the lane the champion is played in most.
    pub lane: Option<Position>,
}

pub async fn fetch_tencent_patches() -> Result<Vec<String>, String> {
    tencent_patches(&get("https://mlol.qt.qq.com/go/database/versionlist?zone=lol&from=h5").await?)
}

async fn tencent_101(
    kind: &str,
    champion_id: i64,
    lane: Position,
    patch: &str,
) -> Result<Option<HashMap<String, String>>, String> {
    let url = format!(
        "https://mlol.qt.qq.com/go/battle_info/odp_proxy/lol_101strategy_{kind}?itier=255&version_id={patch}&lane={}&championid={champion_id}",
        tencent_lane(lane)
    );
    tencent_payload(&get(&url).await?)
}

/// 101's numbers, from the newest patch, or the one before when the newest has none yet for the
/// lane (the first days of a patch).
pub async fn fetch_tencent(
    query: Query,
    patches: &[String],
    known: &Known,
) -> Result<Build, String> {
    let champion = query.champion_id;
    for patch in patches.iter().take(2) {
        let (lane, runeinfo) = match query.lane {
            Some(lane) => (lane, tencent_101("runeinfo", champion, lane, patch).await?),
            None => {
                let answers = futures_util::future::join_all(
                    Position::ALL.map(|lane| tencent_101("runeinfo", champion, lane, patch)),
                )
                .await;
                let lanes: Vec<(Position, Option<HashMap<String, String>>)> = Position::ALL
                    .into_iter()
                    .zip(answers)
                    .map(|(lane, answer)| answer.map(|payload| (lane, payload)))
                    .collect::<Result<_, _>>()?;
                let Some(lane) = busiest_lane(&lanes) else {
                    continue;
                };
                let payload = lanes
                    .into_iter()
                    .find(|(other, _)| *other == lane)
                    .and_then(|(_, payload)| payload);
                (lane, payload)
            }
        };
        if runeinfo
            .as_ref()
            .is_none_or(|payload| tencent_rune_rows(payload) == 0)
        {
            continue;
        }
        let (build, spells, skills, matchups) = tokio::join!(
            tencent_101("build", champion, lane, patch),
            tencent_101("skill", champion, lane, patch),
            tencent_101("skill_point", champion, lane, patch),
            tencent_101("confront", champion, lane, patch),
        );
        let payloads = TencentPayloads {
            build: build?,
            runeinfo,
            spells: spells?,
            skills: skills?,
            matchups: matchups?,
        };
        return Ok(parse_tencent(
            champion, query.mode, lane, patch, &payloads, known,
        ));
    }
    Err(format!("no numbers for champion {champion}"))
}

pub async fn fetch_tencent_hextech(champion_id: i64, known: &Known) -> Result<Build, String> {
    let url = format!(
        "https://mlol.qt.qq.com/go/battle_info/odp_proxy/fuwen_hero_rank?championid={champion_id}"
    );
    let payload = tencent_payload(&get(&url).await?)?
        .ok_or_else(|| format!("no numbers for champion {champion_id}"))?;
    Ok(parse_tencent_hextech(champion_id, &payload, known))
}

fn opgg_url(mode: &str, champion_id: i64, position: Option<&str>) -> String {
    let tier = if mode == "ranked" {
        "emerald_plus"
    } else {
        "all"
    };
    match position {
        Some(position) => format!(
            "https://lol-api-champion.op.gg/api/global/champions/{mode}/{champion_id}/{position}?tier={tier}"
        ),
        None => format!(
            "https://lol-api-champion.op.gg/api/global/champions/{mode}/{champion_id}?tier={tier}"
        ),
    }
}

/// OP.GG's numbers. On the Rift without a lane, the lane is asked for first (`opgg_lanes`): a
/// second request only when the champion's main lane is not the one guessed.
pub async fn fetch_opgg(query: Query, known: &Known) -> Result<Build, String> {
    let mode = opgg_mode(query.mode).ok_or("OP.GG has no numbers for this mode")?;
    let champion = query.champion_id;
    let (lane, body) = match (query.mode, query.lane) {
        (Mode::Ranked | Mode::Normal, Some(lane)) => (
            Some(lane),
            get(&opgg_url(mode, champion, Some(opgg_position(lane)))).await?,
        ),
        (Mode::Ranked | Mode::Normal, None) => {
            const GUESS: Position = Position::Middle;
            let body = get(&opgg_url(mode, champion, Some(opgg_position(GUESS)))).await?;
            match opgg_lanes(&body)?.first() {
                Some(&lane) if lane != GUESS => (
                    Some(lane),
                    get(&opgg_url(mode, champion, Some(opgg_position(lane)))).await?,
                ),
                _ => (Some(GUESS), body),
            }
        }
        (Mode::Aram, _) => (None, get(&opgg_url(mode, champion, Some("none"))).await?),
        _ => (None, get(&opgg_url(mode, champion, None)).await?),
    };
    parse_opgg(champion, query.mode, lane, &body, known)
}

pub async fn fetch_aramgg(champion_id: i64, known: &Known) -> Result<Build, String> {
    let url = format!("https://aramgg.com/data/champion-augments/{champion_id}.json");
    parse_aramgg(champion_id, &get(&url).await?, known)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;
    use crate::{model::ClientAugment, test_support::fixture};

    fn bytes(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/builds")
            .join(name);
        fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    fn payload(name: &str) -> Option<HashMap<String, String>> {
        tencent_payload(&bytes(name)).unwrap()
    }

    /// A share read from text, compared as the number it stands for: 49.7 % is 0.497.
    fn near(value: Option<f64>, expected: f64) -> bool {
        value.is_some_and(|value| (value - expected).abs() < 1e-9)
    }

    /// The client's catalog as the fixtures have it, with a few augments named.
    fn known() -> Known {
        let items: Vec<crate::model::Item> = fixture("live/static/items.json");
        let augments: Vec<ClientAugment> = serde_json::from_value(serde_json::json!([
            {"id": "1336", "nameTRA": "升级：无尽之刃", "rarity": "kGold"},
            {"id": "1356", "nameTRA": "暴击飞弹", "rarity": "kGold"},
            {"id": "1220", "nameTRA": "连拨击锤", "rarity": "kPrismatic"},
            {"id": "1007", "nameTRA": "大力", "rarity": "kSilver"},
            {"id": "1092", "nameTRA": "易损", "rarity": "kGold"},
            {"id": "220", "nameTRA": "连拨击锤", "rarity": "kPrismatic"},
            {"id": "195", "nameTRA": "巨人杀手", "rarity": "kPrismatic"},
            {"id": "82", "nameTRA": "残暴之力", "rarity": "kSilver"},
            {"id": "206", "nameTRA": "魔法转物理", "rarity": "kSilver"},
            {"id": "1320", "nameTRA": "升级：收集者", "rarity": "kSilver"},
            {"id": "84", "nameTRA": "穿针引线", "rarity": "kGold"}
        ]))
        .unwrap();
        Known {
            items: items.into_iter().map(|item| item.id).collect(),
            augments: crate::catalog::augments_of(augments)
                .into_iter()
                .filter_map(|augment| Some((augment.id, augment.rarity?)))
                .collect(),
            styles: StyleBook::new(&fixture("live/static/perkstyles.json")),
        }
    }

    fn tencent_bottom() -> TencentPayloads {
        TencentPayloads {
            build: payload("tencent-build-202-bottom.json"),
            runeinfo: payload("tencent-runeinfo-202-bottom.json"),
            spells: payload("tencent-skill-202-bottom.json"),
            skills: payload("tencent-skill_point-202-bottom.json"),
            matchups: payload("tencent-confront-202-bottom.json"),
        }
    }

    #[test]
    fn each_mode_reads_from_its_own_source_and_hextech_falls_back_to_aramgg() {
        let defaults = BuildSettings::default();
        let with = |change: fn(&mut BuildSettings)| {
            let mut settings = BuildSettings::default();
            change(&mut settings);
            settings
        };
        assert_eq!(sources(Mode::Ranked, &defaults), [BuildSource::Tencent]);
        assert_eq!(
            sources(
                Mode::Normal,
                &with(|settings| settings.rift_source = RiftSource::OpGg)
            ),
            [BuildSource::OpGg]
        );
        assert_eq!(sources(Mode::Aram, &defaults), [BuildSource::OpGg]);
        assert_eq!(
            sources(
                Mode::Hextech,
                &with(|settings| settings.rift_source = RiftSource::OpGg)
            ),
            [BuildSource::TencentHextech, BuildSource::AramGg]
        );
        assert_eq!(sources(Mode::Arena, &defaults), [BuildSource::OpGg]);
        assert!(sources(Mode::Other, &defaults).is_empty());
        // Each switch takes only its own mode's source away.
        let aram_off = with(|settings| settings.aram_source = ModeSource::Off);
        assert!(sources(Mode::Aram, &aram_off).is_empty());
        assert_eq!(sources(Mode::Arena, &aram_off), [BuildSource::OpGg]);
        assert_eq!(
            sources(Mode::Hextech, &aram_off),
            [BuildSource::TencentHextech, BuildSource::AramGg],
            "Hextech ARAM is not ARAM's source"
        );
        let arena_off = with(|settings| settings.arena_source = ModeSource::Off);
        assert!(sources(Mode::Arena, &arena_off).is_empty());
        assert_eq!(sources(Mode::Aram, &arena_off), [BuildSource::OpGg]);
        let no_fallback = with(|settings| settings.hextech_fallback = false);
        assert_eq!(
            sources(Mode::Hextech, &no_fallback),
            [BuildSource::TencentHextech]
        );
        assert_eq!(sources(Mode::Ranked, &no_fallback), [BuildSource::Tencent]);
        // The lanes in every source's words.
        assert_eq!(tencent_lane(Position::Utility), "SUPPORT");
        assert_eq!(opgg_position(Position::Bottom), "adc");
        assert_eq!(opgg_position(Position::Middle), "mid");
        assert_eq!(opgg_lane("ADC"), Some(Position::Bottom));
        assert_eq!(opgg_lane("SUPPORT"), Some(Position::Utility));
    }

    #[test]
    fn the_envelope_holds_a_document_in_a_string_and_an_empty_one_means_no_numbers() {
        let build = payload("tencent-build-202-bottom.json").unwrap();
        assert!(build["starting_details"].starts_with("1055,2003_1_74.5_49.7"));
        assert_eq!(payload("tencent-build-202-bottom-empty.json"), None);
        assert!(
            tencent_payload(br#"{"code":-1,"msg":"busy"}"#)
                .unwrap_err()
                .contains("busy")
        );
        assert_eq!(
            tencent_patches(&bytes("tencent-versionlist.json")).unwrap()[..2],
            ["16.19", "16.18"]
        );
        assert!(is_patch("16.19") && !is_patch("16.19&lane=TOP") && !is_patch("16"));
    }

    #[test]
    fn tencent_101_reads_items_runes_spells_skills_and_matchups() {
        let build = parse_tencent(
            202,
            Mode::Ranked,
            Position::Bottom,
            "16.19",
            &tencent_bottom(),
            &known(),
        );
        assert_eq!(
            (build.source, build.patch.as_str()),
            (BuildSource::Tencent, "16.19")
        );
        assert_eq!(build.lane, Some(Position::Bottom));

        // Doran's Blade and a potion, three in four games; rows are ranked, shares in percent.
        let start = &build.starting[0];
        assert_eq!(start.items, [1055, 2003]);
        assert!(
            near(start.rates.pick, 0.745) && near(start.rates.win, 0.497),
            "{start:?}"
        );
        assert_eq!(build.boots[0].items, [3009]);
        assert_eq!(build.core[0].items, [6697, 6676, 3031]);
        // Fourth to sixth items, each once and none of the core.
        let late: Vec<i64> = build.late.iter().map(|option| option.items[0]).collect();
        assert_eq!(late[..3], [3036, 3094, 3033]);
        assert!(!late.contains(&3031) && late.len() == late.iter().collect::<HashSet<_>>().len());

        // The keystone's style first, the secondary runes' second; ranked rows, rank 1 first.
        let runes = &build.runes[0];
        assert_eq!(
            runes.page,
            RunePage {
                primary_style: 8100,
                sub_style: 8000,
                perks: vec![8128, 8139, 8140, 8135, 8009, 8014, 5008, 5008, 5001],
            }
        );
        assert_eq!(runes.rates.games, Some(585_750));
        assert_eq!(build.sample, Some(1_241_522), "585 750 games at 47.18 %");

        // Spells: the shares come last in these rows and sum to about one.
        assert_eq!(build.spells[0].spells, [21, 4]);
        assert!(near(build.spells[0].rates.pick, 0.7124));
        let picks: f64 = build
            .spells
            .iter()
            .filter_map(|option| option.rates.pick)
            .sum();
        assert!((picks - 1.0).abs() < 0.01, "{picks}");

        let order = &build.skill_orders[0];
        assert_eq!(order.priority, [Ability::Q, Ability::W, Ability::E]);
        assert_eq!(order.sequence.len(), 15);
        assert_eq!(
            order.sequence[..6],
            [
                Ability::Q,
                Ability::W,
                Ability::E,
                Ability::Q,
                Ability::Q,
                Ability::R
            ]
        );
        assert!(near(order.rates.pick, 0.8269));

        let good: Vec<i64> = build
            .matchups
            .good
            .iter()
            .map(|matchup| matchup.champion_id)
            .collect();
        assert_eq!(good, [81, 110, 800, 51, 901]);
        assert!(near(build.matchups.good[0].rates.win, 0.5409));
        assert_eq!(build.matchups.bad[0].champion_id, 895);
        assert!(
            build
                .matchups
                .bad
                .iter()
                .all(|matchup| matchup.rates.win < Some(0.5))
        );
        assert_eq!(build.dropped, 0);
    }

    #[test]
    fn without_a_lane_the_one_with_the_most_games_is_taken() {
        let lanes: Vec<(Position, Option<HashMap<String, String>>)> = Position::ALL
            .into_iter()
            .map(|lane| {
                let name = format!(
                    "tencent-runeinfo-202-{}.json",
                    tencent_lane(lane).to_lowercase()
                );
                (lane, payload(&name))
            })
            .collect();
        assert_eq!(busiest_lane(&lanes), Some(Position::Bottom));
        let without_bottom: Vec<_> = lanes
            .into_iter()
            .map(|(lane, payload)| (lane, payload.filter(|_| lane != Position::Bottom)))
            .collect();
        assert_eq!(busiest_lane(&without_bottom), Some(Position::Middle));
        assert_eq!(busiest_lane(&[(Position::Top, None)]), None);
    }

    #[test]
    fn a_rune_row_without_a_style_the_client_knows_is_dropped_and_counted() {
        let mut payloads = tencent_bottom();
        payloads.runeinfo = Some(HashMap::from([(
            "rune_top_details".to_owned(),
            "1_8128_jm_8128,8139,8140,8135,8009,8014,5008,5008,5001_47.18_49.14_585750#2_9999_xx_9999,8139,8140,8135,8009,8014,5008,5008,5001_1_50_10".to_owned(),
        )]));
        let build = parse_tencent(
            202,
            Mode::Ranked,
            Position::Bottom,
            "16.19",
            &payloads,
            &known(),
        );
        assert_eq!((build.runes.len(), build.dropped), (1, 1));
    }

    #[test]
    fn hextech_numbers_rank_augments_by_tier_and_drop_what_the_client_does_not_know() {
        let known = known();
        let build =
            parse_tencent_hextech(202, &payload("tencent-hextech-202.json").unwrap(), &known);
        assert_eq!(
            (build.source, build.mode),
            (BuildSource::TencentHextech, Mode::Hextech)
        );
        let first = &build.augments[0];
        assert_eq!(
            (first.id, first.rarity, first.tier),
            (1336, Rarity::Gold, Some(AugmentTier::S))
        );
        assert!(near(first.rates.pick, 0.2073));
        // Best first: every S before the first A, whatever the pick rates.
        let tiers: Vec<AugmentTier> = build
            .augments
            .iter()
            .filter_map(|augment| augment.tier)
            .collect();
        assert!(tiers.windows(2).all(|pair| pair[0] <= pair[1]), "{tiers:?}");
        assert_eq!(
            build
                .augments
                .iter()
                .find(|augment| augment.id == 1092)
                .unwrap()
                .tier,
            Some(AugmentTier::A)
        );
        // 109 rows, each once; only the six the test catalog names are kept.
        assert_eq!(build.augments.len(), 6);
        assert_eq!(build.dropped, 109 - 6);

        // Items: the mode's own copy (126697) is in the client's catalog, so it stays.
        assert_eq!(build.core[0].items, [6676, 3031, 3036]);
        assert!(near(build.core[0].rates.pick, 0.1725));
        assert_eq!(build.boots[0].items, [3009]);
        assert_eq!(
            build.starting[0].items,
            [1036, 1037],
            "the most taken start"
        );
        assert!(build.late.iter().any(|option| option.items == [126697]));
        assert!(
            build
                .late
                .iter()
                .all(|option| ![6676, 3031, 3036, 3009].contains(&option.items[0]))
        );
        let order = &build.skill_orders[0];
        assert_eq!(order.priority, [Ability::Q, Ability::E, Ability::W]);
        assert!(near(order.rates.pick, 0.366) && near(order.rates.win, 0.4792));
        assert_eq!(order.sequence[..3], [Ability::Q, Ability::W, Ability::E]);
    }

    #[test]
    fn opgg_ranked_numbers_come_with_the_lanes_standing_and_its_matchups() {
        let build = parse_opgg(
            202,
            Mode::Ranked,
            Some(Position::Bottom),
            &bytes("opgg-ranked-202-adc.json"),
            &known(),
        )
        .unwrap();
        assert_eq!(
            (build.patch.as_str(), build.tier, build.sample),
            ("16.19", Some(1), Some(384_427))
        );
        assert_eq!(build.spells[0].spells, [4, 21]);
        let flash = &build.spells[0].rates;
        assert_eq!(flash.pick, Some(0.8096));
        assert_eq!(flash.win, Some(147_083.0 / 296_216.0));
        assert_eq!(
            build.runes[0].page,
            RunePage {
                primary_style: 8000,
                sub_style: 8200,
                perks: vec![8021, 8009, 9103, 8014, 8234, 8236, 5008, 5008, 5001],
            }
        );
        assert_eq!(build.starting[0].items, [1055, 2003]);
        assert_eq!(build.core[0].items, [6697, 6676, 3031]);
        assert!(
            build
                .late
                .iter()
                .all(|option| ![6697, 6676, 3031, 3009].contains(&option.items[0]))
        );
        assert_eq!(
            build.skill_orders[0].priority,
            [Ability::Q, Ability::W, Ability::E]
        );
        assert_eq!(build.skill_orders[0].sequence.len(), 15);
        // The best and worst matchups by the champion's own win rate, with enough games.
        assert_eq!(build.matchups.good[0].champion_id, 81);
        assert_eq!(build.matchups.bad[0].champion_id, 157, "Yasuo, 46.2 %");
        assert!(
            !build
                .matchups
                .good
                .iter()
                .any(|matchup| matchup.champion_id == 800),
            "1 210 games are too few"
        );
        assert!(build.augments.is_empty());
    }

    #[test]
    fn opgg_names_the_main_lane_whichever_lane_was_asked() {
        assert_eq!(
            opgg_lanes(&bytes("opgg-ranked-202-top.json")).unwrap(),
            [Position::Bottom]
        );
        let top = parse_opgg(
            202,
            Mode::Ranked,
            Some(Position::Top),
            &bytes("opgg-ranked-202-top.json"),
            &known(),
        )
        .unwrap();
        assert_eq!(top.lane, Some(Position::Top));
        assert_eq!(
            top.tier,
            Some(2),
            "off its lanes, the champion's own standing"
        );
    }

    #[test]
    fn opgg_aram_has_no_lane_and_no_matchups() {
        let build = parse_opgg(
            202,
            Mode::Aram,
            None,
            &bytes("opgg-aram-202.json"),
            &known(),
        )
        .unwrap();
        assert_eq!((build.lane, build.tier), (None, Some(2)));
        assert_eq!(build.spells[0].spells, [4, 6]);
        assert_eq!(build.runes[0].page.primary_style, 8100);
        assert!(build.matchups.good.is_empty() && build.matchups.bad.is_empty());
        assert!(!build.starting.is_empty());
    }

    #[test]
    fn arena_augments_are_judged_by_their_average_finish() {
        let build = parse_opgg(
            202,
            Mode::Arena,
            None,
            &bytes("opgg-arena-202.json"),
            &known(),
        )
        .unwrap();
        // Fan the Hammer: 71 005 over 22 616 games is 3.14, counted from 0, so a 4.14 finish.
        let fan = &build.augments[0];
        assert_eq!(fan.id, 220);
        assert_eq!(fan.rates.placement, Some(71_005.0 / 22_616.0 + 1.0));
        assert_eq!(fan.rates.first, Some(4_543.0 / 22_616.0));
        let finishes: Vec<f64> = build
            .augments
            .iter()
            .filter_map(|augment| augment.rates.placement)
            .collect();
        assert!(
            finishes.windows(2).all(|pair| pair[0] <= pair[1]),
            "{finishes:?}"
        );
        assert!(finishes.iter().all(|&finish| (1.0..=8.0).contains(&finish)));
        assert_eq!(build.augments[0].rarity, Rarity::Prismatic);
        // Arena's own items (22xxxx) are in the client's catalog; no starting items in Arena.
        assert_eq!(build.core[0].items, [226676, 223031, 223036]);
        assert!(build.starting.is_empty());
        assert!(build.dropped > 0, "augments the test catalog does not name");
    }

    #[test]
    fn aramgg_tiers_are_tencents_four_by_number() {
        let build =
            parse_aramgg(202, &bytes("aramgg-champion-augments-202.json"), &known()).unwrap();
        assert_eq!(
            (build.source, build.patch.as_str()),
            (BuildSource::AramGg, "16.19")
        );
        let first = &build.augments[0];
        assert_eq!((first.id, first.tier), (1336, Some(AugmentTier::S)));
        assert_eq!(first.rates.games, Some(31_763));
        assert_eq!(build.augments.len() as u32 + build.dropped, 109);
        assert!(parse_aramgg(1, &bytes("aramgg-champion-augments-202.json"), &known()).is_err());
    }

    #[test]
    fn tiers_read_as_letters_or_their_numbers_and_nothing_else() {
        let tiers: Vec<Option<AugmentTier>> = ["S", " A ", "3", "4", "1", "D", "", "5"]
            .into_iter()
            .map(AugmentTier::parse)
            .collect();
        assert_eq!(
            tiers,
            [
                Some(AugmentTier::S),
                Some(AugmentTier::A),
                Some(AugmentTier::B),
                Some(AugmentTier::C),
                Some(AugmentTier::S),
                None,
                None,
                None
            ]
        );
        assert!(AugmentTier::S < AugmentTier::C, "best first when sorted");
    }

    #[test]
    fn unknown_items_leave_their_options_and_empty_options_go() {
        let known = Known {
            items: HashSet::from([3009, 3031]),
            ..Known::default()
        };
        let mut build = Build::new(BuildSource::OpGg, 1, Mode::Arena);
        build.core = vec![
            ItemOption {
                items: vec![443031, 3031],
                rates: Rates::default(),
            },
            ItemOption {
                items: vec![447120],
                rates: Rates::default(),
            },
        ];
        build.boots = vec![ItemOption {
            items: vec![3009],
            rates: Rates::default(),
        }];
        known.keep_known_items(&mut build);
        assert_eq!(build.core.len(), 1);
        assert_eq!(build.core[0].items, [3031]);
        assert_eq!(build.dropped, 2);
    }

    #[test]
    fn style_pages_need_a_known_keystone_and_two_secondary_runes_of_one_style() {
        let book = StyleBook::new(&fixture("live/static/perkstyles.json"));
        let page = |perks: &[i64]| book.page(perks.to_vec());
        assert_eq!(
            page(&[8021, 8009, 9103, 8014, 8234, 8236, 5008, 5008, 5001])
                .map(|page| (page.primary_style, page.sub_style)),
            Some((8000, 8200))
        );
        assert_eq!(
            page(&[8021, 8009, 9103, 8014, 8234, 8139, 5008, 5008, 5001]),
            None,
            "two styles below"
        );
        assert_eq!(
            page(&[8021, 8009, 9103, 8014, 8009, 8014, 5008, 5008, 5001]),
            None,
            "the primary twice"
        );
        assert_eq!(page(&[8021, 8009]), None);
    }
}
