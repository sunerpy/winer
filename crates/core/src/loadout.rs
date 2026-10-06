//! Runes, summoner spells and item sets for the champion in hand: the setup last played on it in a
//! kind of game, kept in `loadouts.json`; when to set it up again and where a rune page can go; and
//! winer's item sets inside the client's own list. Carrying them out is the service's job
//! (`service::loadout`); nothing here touches the client.

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
    builds::{Build, BuildSource, ItemOption, RunePage},
    model::ChampSelectSession,
    settings::{Language, Mode, write_atomic},
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

// What is remembered.

/// A champion's setup for one kind of game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    /// `None` where the game started without a full page.
    pub runes: Option<RunePage>,
    pub spells: Option<[i64; 2]>,
    /// Epoch milliseconds; zero for one that was never played (the client's recommendation).
    #[serde(default)]
    pub saved_at: i64,
}

/// How many setups are remembered, for the automation page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutSummary {
    pub remembered: u32,
}

/// Where a remembered setup lives in the file: `"202:ranked"`.
fn key(champion_id: i64, mode: Mode) -> String {
    let mode = serde_json::to_value(mode)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("{champion_id}:{mode}")
}

/// `loadouts.json`, beside the settings. Writes are atomic, as the settings' are.
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
                tracing::warn!(%error, aside = %aside.display(), "loadouts file is invalid, starting empty");
                let _ = fs::rename(&path, aside);
                BTreeMap::new()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                tracing::warn!(%error, "loadouts file is unreadable, starting empty");
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

    pub fn get(&self, champion_id: i64, mode: Mode) -> Option<Setup> {
        self.lock().get(&key(champion_id, mode)).cloned()
    }

    pub fn put(&self, champion_id: i64, mode: Mode, setup: Setup) -> io::Result<()> {
        let mut setups = self.lock();
        let mut next = setups.clone();
        next.insert(key(champion_id, mode), setup);
        self.write(&next)?;
        *setups = next;
        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        let mut setups = self.lock();
        self.write(&BTreeMap::new())?;
        setups.clear();
        Ok(())
    }

    pub fn summary(&self) -> LoadoutSummary {
        LoadoutSummary {
            remembered: self.lock().len() as u32,
        }
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

/// Two different spells, both set.
fn spells(spells: [i64; 2]) -> Option<[i64; 2]> {
    (spells[0] > 0 && spells[1] > 0 && spells[0] != spells[1]).then_some(spells)
}

/// The client's recommendation as a setup: its default page for the position, or its first.
pub fn recommended_setup(pages: &[RecommendedPage]) -> Option<Setup> {
    let page = pages
        .iter()
        .find(|page| page.is_default_position)
        .or_else(|| pages.first())?;
    let runes = RunePage {
        primary_style: page.primary_perk_style_id,
        sub_style: page.secondary_perk_style_id,
        perks: page.perks.iter().map(|perk| perk.id).collect(),
    };
    let setup = Setup {
        runes: runes.is_complete().then_some(runes),
        spells: match page.summoner_spell_ids[..] {
            [first, second] => spells([first, second]),
            _ => None,
        },
        saved_at: 0,
    };
    (setup.runes.is_some() || setup.spells.is_some()).then_some(setup)
}

/// What the local player goes into the game with, to remember: the current page where it is a full
/// one, and the two spells. `None` when neither was chosen.
pub fn setup_at_start(
    session: &ChampSelectSession,
    page: Option<&PerkPage>,
    now: i64,
) -> Option<(i64, Setup)> {
    let me = session.local_player()?;
    if me.champion_id <= 0 {
        return None;
    }
    let setup = Setup {
        runes: page.and_then(PerkPage::runes),
        spells: spells([me.spell1_id, me.spell2_id]),
        saved_at: now,
    };
    (setup.runes.is_some() || setup.spells.is_some()).then_some((me.champion_id, setup))
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
    /// the Rift and the two ARAMs the Howling Abyss, so the champion keeps one set per map. Added
    /// after the others where there was none; the sets that are not winer's keep their places.
    pub fn with(mut self, champion_id: i64, mode: Mode, set: Box<RawValue>, now: u64) -> Self {
        let own = item_set_uid(champion_id, mode);
        let maps = item_set_maps(mode);
        let shadowed = |other: &RawValue| {
            uid(other).is_some_and(|uid| {
                uid == own
                    || our_uid(&uid).is_some_and(|(champion, other)| {
                        champion == champion_id
                            && item_set_maps(other).iter().any(|map| maps.contains(map))
                    })
            })
        };
        let mut set = Some(set);
        let mut sets = Vec::with_capacity(self.item_sets.len() + 1);
        for other in std::mem::take(&mut self.item_sets) {
            if !shadowed(&other) {
                sets.push(other);
            } else if let Some(set) = set.take() {
                sets.push(set);
            }
        }
        sets.extend(set);
        self.item_sets = sets;
        self.timestamp = now;
        self
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
    use crate::{builds::Rates, test_support::fixture};

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

        let pages: Vec<RecommendedPage> = fixture("loadout/perks-recommended-202-bottom-11.json");
        let setup = recommended_setup(&pages).unwrap();
        assert_eq!(
            setup.runes,
            Some(RunePage {
                primary_style: 8100,
                sub_style: 8000,
                perks: vec![8128, 8139, 8140, 8135, 8014, 8009, 5008, 5008, 5001],
            })
        );
        assert_eq!(setup.spells, Some([21, 4]));
        let abyss: Vec<RecommendedPage> = fixture("loadout/perks-recommended-202-none-12.json");
        assert_eq!(recommended_setup(&abyss).unwrap().spells, Some([4, 6]));
        assert_eq!(recommended_setup(&[]), None);
        // The default position's page wins over the first.
        let mut mixed = pages.clone();
        mixed[0].is_default_position = false;
        mixed[1].primary_perk_style_id = 8000;
        assert_eq!(
            recommended_setup(&mixed).unwrap().runes.unwrap().perks[0],
            8021
        );
    }

    #[test]
    fn the_setup_a_game_starts_with_is_remembered_spells_alone_without_a_page() {
        let locked = session("finalization");
        let page: PerkPage = fixture("loadout/perks-currentpage.json");
        let (champion, setup) = setup_at_start(&locked, Some(&page), 5).unwrap();
        assert_eq!(champion, 236);
        assert_eq!(setup.spells, Some([6, 7]));
        assert_eq!(
            setup.runes.unwrap().perks,
            [8005, 8009, 9103, 8014, 8139, 8135, 5005, 5008, 5001]
        );
        let (_, spells_only) = setup_at_start(&locked, None, 5).unwrap();
        assert_eq!(
            (spells_only.runes, spells_only.spells),
            (None, Some([6, 7]))
        );
        let mut broken = page.clone();
        broken.is_valid = false;
        assert_eq!(
            setup_at_start(&locked, Some(&broken), 5).unwrap().1.runes,
            None
        );
        let mut nothing = locked.clone();
        nothing.my_team[0].spell1_id = 0;
        assert_eq!(setup_at_start(&nothing, None, 5), None);
        assert_eq!(setup_at_start(&session("ban-pick"), Some(&page), 5), None);
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
    fn remembered_setups_round_trip_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loadouts.json");
        let store = LoadoutStore::open(&path);
        assert_eq!(store.summary().remembered, 0);
        let setup = Setup {
            runes: None,
            spells: Some([4, 7]),
            saved_at: 9,
        };
        store.put(202, Mode::Ranked, setup.clone()).unwrap();
        store.put(202, Mode::Aram, setup.clone()).unwrap();
        let again = LoadoutStore::open(&path);
        assert_eq!(again.get(202, Mode::Ranked), Some(setup));
        assert_eq!(again.get(202, Mode::Normal), None, "modes are kept apart");
        assert_eq!(again.summary().remembered, 2);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"202:aram\""), "{text}");
        again.clear().unwrap();
        assert_eq!(LoadoutStore::open(&path).summary().remembered, 0);

        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(LoadoutStore::open(&path).summary().remembered, 0);
        assert!(path.with_extension("json.invalid").exists());
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
        // A normal game is played on the Rift too: its set takes the ranked one's place.
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
        assert_eq!(uids(&list), ["winer-202-normal", "a1b2", "winer-202-aram"]);
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
                "winer-202-normal",
                "a1b2",
                "winer-202-hextech",
                "winer-202-arena"
            ]
        );
        assert_eq!(our_uid("winer-202-hextech"), Some((202, Mode::Hextech)));
        assert_eq!(our_uid("a1b2"), None);
        assert_eq!(our_uid("winer-x-ranked"), None);
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
