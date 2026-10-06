//! What friends see of the player, and what winer keeps there for them: the profile background,
//! the challenge tokens, title and banner, the rank in the friends list and the chat status put
//! back after the client resets it. The client's documents go in and the window's views come out;
//! the requests themselves are the service's.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use ts_rs::TS;

use crate::{
    model::ChatMe,
    settings::{DisguiseQueue, Division, PresenceRule, ProfileSettings, RankDisguise},
    view::{Phase, Tier},
};

// ---- Background --------------------------------------------------------------------------------

/// From this id up a champion is a copy a mode uses (`Jade_Annie` is 60001), and each of its skins
/// repeats one the champion already lists (`docs/platform-notes.md`).
const MODE_COPIES: i64 = 60_000;

/// One row of `/lol-champions/v1/inventories/{summonerId}/skins-minimal`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSkin {
    pub id: i64,
    pub champion_id: i64,
    pub name: String,
    pub is_base: bool,
    pub disabled: bool,
    pub ownership: SkinOwnership,
    pub tile_path: String,
    pub splash_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SkinOwnership {
    pub owned: bool,
}

/// A skin the profile background can be set to.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkinChoice {
    pub id: i64,
    pub champion_id: i64,
    /// As the client names it; a base skin carries the champion's title (`九尾妖狐`).
    pub name: String,
    pub owned: bool,
    pub base: bool,
    /// LCU asset paths, served to the window through the `lcu` protocol.
    pub tile: String,
    pub splash: String,
}

/// Every skin of every champion, owned or not, by champion and then by skin. The copies a mode
/// keeps of champions are left out, and so is a skin the client disabled.
pub fn skins(listed: Vec<ClientSkin>) -> Vec<SkinChoice> {
    let mut choices: Vec<SkinChoice> = listed
        .into_iter()
        .filter(|skin| {
            skin.id > 0 && (1..MODE_COPIES).contains(&skin.champion_id) && !skin.disabled
        })
        .map(|skin| SkinChoice {
            id: skin.id,
            champion_id: skin.champion_id,
            name: skin.name,
            owned: skin.ownership.owned,
            base: skin.is_base,
            tile: skin.tile_path,
            splash: skin.splash_path,
        })
        .collect();
    choices.sort_by_key(|skin| (skin.champion_id, skin.id));
    choices.dedup_by_key(|skin| skin.id);
    choices
}

/// `/lol-summoner/v1/current-summoner/summoner-profile`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SummonerProfile {
    /// Zero while the player has chosen none.
    pub background_skin_id: i64,
}

impl SummonerProfile {
    pub fn background(&self) -> Option<i64> {
        (self.background_skin_id > 0).then_some(self.background_skin_id)
    }
}

/// The body of `POST /lol-summoner/v1/current-summoner/summoner-profile` that sets the background.
pub fn background_request(skin_id: i64) -> Value {
    json!({ "key": "backgroundSkinId", "value": skin_id })
}

// ---- Challenges --------------------------------------------------------------------------------

/// The token slots a profile has.
pub const TOKEN_SLOTS: usize = 3;

/// A challenge of `/lol-challenges/v1/challenges/local-player` (a map by id), or of the summary's
/// `topChallenges`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientChallenge {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// `NONE` until the first level, then `IRON` … `CHALLENGER`.
    pub current_level: String,
    /// The token's picture at each level.
    pub level_to_icon_path: HashMap<String, String>,
}

/// `/lol-challenges/v1/summary-player-data/local-player`, the part the profile shows.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChallengeSummary {
    /// The tokens in their slots as one string of ids, comma-joined: `101101,101205,2023005`.
    pub selected_challenges_string: String,
    /// The same tokens, described.
    pub top_challenges: Vec<ClientChallenge>,
    pub title: Option<ClientTitle>,
    /// The banner chosen last, by its id in the regalia's inventory; empty while none is.
    pub banner_id: String,
}

/// A title of `/lol-challenges/v2/titles/local-player`, or the summary's current one.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientTitle {
    pub item_id: i64,
    pub name: String,
}

/// A challenge token the profile can show.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeToken {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// `None` for a token the client describes without a level.
    pub level: Option<Tier>,
    /// The token at its level, an LCU asset path; empty when the client names none.
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TitleChoice {
    /// The title's `itemId`, which is what the client takes.
    pub id: i64,
    pub name: String,
}

/// What the profile shows of challenges, and what it could show.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeProfile {
    /// The tokens in the profile's slots, left to right: three at most.
    pub tokens: Vec<ChallengeToken>,
    pub title: Option<TitleChoice>,
    /// Every challenge with a level, the highest first: what a slot can hold.
    pub challenges: Vec<ChallengeToken>,
    pub titles: Vec<TitleChoice>,
    /// The banner the profile shows, as the id of one of `banners`: empty for the default.
    pub banner: String,
    /// What the banner can be (`banner_choices`); empty when the client lists no banners.
    pub banners: Vec<BannerChoice>,
}

/// The ids in `selectedChallengesString`, in slot order. One token reads `101304`; whatever stands
/// between several of them, digits are all an id is made of.
pub fn selected_tokens(text: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    for part in text.split(|c: char| !c.is_ascii_digit()) {
        if let Ok(id) = part.parse::<i64>()
            && id > 0
            && !ids.contains(&id)
            && ids.len() < TOKEN_SLOTS
        {
            ids.push(id);
        }
    }
    ids
}

fn token(challenge: &ClientChallenge) -> ChallengeToken {
    ChallengeToken {
        id: challenge.id,
        name: challenge.name.clone(),
        description: challenge.description.clone(),
        level: Tier::parse(&challenge.current_level),
        icon: challenge
            .level_to_icon_path
            .get(&challenge.current_level)
            .cloned()
            .unwrap_or_default(),
    }
}

pub fn challenge_profile(
    challenges: &HashMap<String, ClientChallenge>,
    summary: &ChallengeSummary,
    titles: &[ClientTitle],
    banners: &HashMap<String, ClientBanner>,
    regalia: &Regalia,
) -> ChallengeProfile {
    let mut known: HashMap<i64, &ClientChallenge> = HashMap::new();
    for challenge in challenges.values().chain(&summary.top_challenges) {
        known.entry(challenge.id).or_insert(challenge);
    }
    let tokens = selected_tokens(&summary.selected_challenges_string)
        .into_iter()
        .map(|id| {
            known.get(&id).map_or_else(
                || ChallengeToken {
                    id,
                    name: String::new(),
                    description: String::new(),
                    level: None,
                    icon: String::new(),
                },
                |challenge| token(challenge),
            )
        })
        .collect();
    let mut choices: Vec<ChallengeToken> = challenges
        .values()
        .filter(|challenge| challenge.id > 0)
        .map(token)
        .filter(|token| token.level.is_some())
        .collect();
    choices.sort_by(|a, b| b.level.cmp(&a.level).then(a.id.cmp(&b.id)));
    let mut choices_of_title: Vec<TitleChoice> = Vec::with_capacity(titles.len());
    for title in titles {
        if title.item_id > 0 && !choices_of_title.iter().any(|seen| seen.id == title.item_id) {
            choices_of_title.push(TitleChoice {
                id: title.item_id,
                name: title.name.clone(),
            });
        }
    }
    let banner = shown_banner(&summary.banner_id, &regalia.preferred_banner_type);
    ChallengeProfile {
        tokens,
        title: summary
            .title
            .as_ref()
            .filter(|title| title.item_id > 0)
            .map(|title| TitleChoice {
                id: title.item_id,
                name: title.name.clone(),
            }),
        challenges: choices,
        titles: choices_of_title,
        banners: banner_choices(banners, regalia, &banner),
        banner,
    }
}

/// Up to three tokens, each once: what a profile has room for.
pub fn check_tokens(tokens: &[i64]) -> Result<(), String> {
    if tokens.len() > TOKEN_SLOTS {
        return Err(format!("a profile shows {TOKEN_SLOTS} tokens at most"));
    }
    for (index, id) in tokens.iter().enumerate() {
        if *id <= 0 || tokens[..index].contains(id) {
            return Err(format!("token {id} is not a challenge, or chosen twice"));
        }
    }
    Ok(())
}

/// The body of `POST /lol-challenges/v1/update-player-preferences`: the tokens in slot order, the
/// title by its `itemId`, which the client takes as a string, and the banner as `bannerAccent`, an
/// empty one for the default (`docs/platform-notes.md`).
pub fn preferences_request(tokens: &[i64], title: Option<i64>, banner: Option<&str>) -> Value {
    let mut body = json!({ "challengeIds": tokens });
    if let Some(title) = title {
        body["title"] = json!(title.to_string());
    }
    if let Some(banner) = banner {
        body["bannerAccent"] = json!(banner);
    }
    body
}

/// Whether the client shows what was asked for.
pub fn shows(
    profile: &ChallengeProfile,
    tokens: &[i64],
    title: Option<i64>,
    banner: Option<&str>,
) -> bool {
    profile
        .tokens
        .iter()
        .map(|token| token.id)
        .eq(tokens.iter().copied())
        && title.is_none_or(|id| profile.title.as_ref().is_some_and(|shown| shown.id == id))
        && banner.is_none_or(|id| profile.banner == id)
}

// ---- Banner ------------------------------------------------------------------------------------

/// The client's own default banner. A summary that names it, or names none, shows it, unless the
/// regalia draws the rank banner (`shown_banner`).
const DEFAULT_BANNER: &str = "1";
/// The banner of last season's highest rank, drawn in that tier's art.
pub const RANK_BANNER: &str = "2";
/// How the regalia draws the banner: plain for the default and every event's, the tier for the
/// rank banner.
const BLANK_TYPE: &str = "blank";
const RANK_TYPE: &str = "lastSeasonHighestRank";

/// One banner of `/lol-regalia/v3/inventory/REGALIA_BANNER`, a map by id of every banner there is,
/// owned or not.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientBanner {
    pub is_owned: bool,
    /// One per variant: the rank banner has one per tier (`idSecondary`), the others one.
    pub items: Vec<ClientBannerItem>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientBannerItem {
    pub id: String,
    /// The tier of a rank banner's variant (`GOLD`); empty for the others.
    pub id_secondary: String,
    pub asset_path: String,
    pub is_selectable: bool,
    /// Empty for the default and the rank banner.
    pub localized_name: String,
}

/// `/lol-regalia/v2/current-summoner/regalia`, the part a banner needs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Regalia {
    /// `blank` or `lastSeasonHighestRank`; empty while unread.
    pub preferred_banner_type: String,
    pub preferred_crest_type: String,
    pub selected_prestige_crest: i64,
    /// `None` without a ranked tier last season, when the client offers no rank banner.
    pub last_season_highest_rank: Option<String>,
}

/// What a banner is, which also says how the window names it: the client names neither its
/// default nor the rank banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BannerKind {
    Default,
    Rank,
    Event,
}

/// A banner the profile can show.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BannerChoice {
    /// What `bannerAccent` takes; empty for the default.
    pub id: String,
    pub kind: BannerKind,
    /// As the client names it; empty for the default and the rank banner.
    pub name: String,
    /// An LCU asset path; banners are drawn 580 × 1480.
    pub art: String,
}

/// The banner the profile shows, as a choice's id, read the way the client's own customizer reads
/// it: a summary naming none leaves it to the regalia's type, and one naming the default is the
/// default.
pub fn shown_banner(banner_id: &str, banner_type: &str) -> String {
    match banner_id {
        "" if banner_type == RANK_TYPE => RANK_BANNER.to_owned(),
        "" | DEFAULT_BANNER => String::new(),
        id => id.to_owned(),
    }
}

/// What the banner can be, as the client's customizer offers it: its default first, the rank
/// banner when there is a rank to show (or it is the one shown), then every event's banner the
/// player owns, by id. The banner shown is listed even when it is no longer owned, so the window
/// can draw it. Nothing at all when the client lists no banners.
pub fn banner_choices(
    inventory: &HashMap<String, ClientBanner>,
    regalia: &Regalia,
    shown: &str,
) -> Vec<BannerChoice> {
    if inventory.is_empty() {
        return Vec::new();
    }
    fn selectable(banner: &ClientBanner) -> Option<&ClientBannerItem> {
        banner.items.iter().find(|item| item.is_selectable)
    }
    let mut choices = vec![BannerChoice {
        id: String::new(),
        kind: BannerKind::Default,
        name: String::new(),
        art: inventory
            .get(DEFAULT_BANNER)
            .and_then(selectable)
            .map(|item| item.asset_path.clone())
            .unwrap_or_default(),
    }];
    let tier = regalia
        .last_season_highest_rank
        .as_deref()
        .filter(|tier| !tier.is_empty());
    if let Some(banner) = inventory.get(RANK_BANNER)
        && (tier.is_some() || shown == RANK_BANNER)
    {
        let art = tier
            .and_then(|tier| {
                banner
                    .items
                    .iter()
                    .find(|item| item.id_secondary.eq_ignore_ascii_case(tier))
            })
            .or_else(|| selectable(banner));
        choices.push(BannerChoice {
            id: RANK_BANNER.to_owned(),
            kind: BannerKind::Rank,
            name: String::new(),
            art: art.map(|item| item.asset_path.clone()).unwrap_or_default(),
        });
    }
    let mut events: Vec<(u64, BannerChoice)> = inventory
        .iter()
        .filter_map(|(id, banner)| {
            let number = id.parse::<u64>().ok().filter(|number| *number > 2)?;
            let item = selectable(banner)?;
            (banner.is_owned || id == shown).then(|| {
                let choice = BannerChoice {
                    id: id.clone(),
                    kind: BannerKind::Event,
                    name: item.localized_name.clone(),
                    art: item.asset_path.clone(),
                };
                (number, choice)
            })
        })
        .collect();
    events.sort_by_key(|(number, _)| *number);
    choices.extend(events.into_iter().map(|(_, choice)| choice));
    choices
}

/// A banner's id as `bannerAccent` takes it: digits, or nothing for the default.
pub fn check_banner(id: &str) -> Result<(), String> {
    if id.len() <= 12 && id.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(format!("{id} is not a banner"))
    }
}

/// The body of `PUT /lol-regalia/v2/current-summoner/regalia` that draws `banner` the way it is
/// drawn, the crest as it was, as the client's customizer saves it; `None` when the regalia
/// already draws it so, or was not read.
pub fn regalia_request(regalia: &Regalia, banner: &str) -> Option<Value> {
    let wanted = if banner == RANK_BANNER {
        RANK_TYPE
    } else {
        BLANK_TYPE
    };
    (!regalia.preferred_crest_type.is_empty() && regalia.preferred_banner_type != wanted).then(
        || {
            json!({
                "preferredCrestType": regalia.preferred_crest_type,
                "preferredBannerType": wanted,
                "selectedPrestigeCrest": regalia.selected_prestige_crest,
            })
        },
    )
}

// ---- Rank in the friends list ------------------------------------------------------------------

/// The keys of the chat presence's `lol` map that carry the rank friends see.
const RANK_KEYS: [&str; 3] = [
    "rankedLeagueQueue",
    "rankedLeagueTier",
    "rankedLeagueDivision",
];

/// The rank a chat presence shows friends, key by key; `None` where the key is absent, as it is for
/// an unranked player.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownRank([Option<String>; 3]);

impl ShownRank {
    pub fn of(lol: &HashMap<String, String>) -> Self {
        Self(RANK_KEYS.map(|key| lol.get(key).cloned()))
    }

    /// What `rule` has friends see. From Master up there are no divisions, and the client writes
    /// `NA` for none, as its ranked stats do.
    pub fn disguise(rule: &RankDisguise) -> Self {
        let queue = match rule.queue {
            DisguiseQueue::Solo => "RANKED_SOLO_5x5",
            DisguiseQueue::Flex => "RANKED_FLEX_SR",
        };
        let division = if rule.tier >= Tier::Master {
            "NA"
        } else {
            match rule.division {
                Division::One => "I",
                Division::Two => "II",
                Division::Three => "III",
                Division::Four => "IV",
            }
        };
        Self([
            Some(queue.to_owned()),
            Some(tier_name(rule.tier).to_owned()),
            Some(division.to_owned()),
        ])
    }

    /// Every key present but empty: for a client that merges `lol` instead of replacing it, where a
    /// key left out keeps its value.
    fn blank() -> Self {
        Self([
            Some(String::new()),
            Some(String::new()),
            Some(String::new()),
        ])
    }

    /// `lol` showing this rank: every other key as it was, a key of `None` taken out.
    pub fn applied_to(&self, lol: &HashMap<String, String>) -> HashMap<String, String> {
        let mut lol = lol.clone();
        for (key, value) in RANK_KEYS.iter().zip(&self.0) {
            match value {
                Some(value) => lol.insert((*key).to_owned(), value.clone()),
                None => lol.remove(*key),
            };
        }
        lol
    }
}

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Iron => "IRON",
        Tier::Bronze => "BRONZE",
        Tier::Silver => "SILVER",
        Tier::Gold => "GOLD",
        Tier::Platinum => "PLATINUM",
        Tier::Emerald => "EMERALD",
        Tier::Diamond => "DIAMOND",
        Tier::Master => "MASTER",
        Tier::Grandmaster => "GRANDMASTER",
        Tier::Challenger => "CHALLENGER",
    }
}

/// The body of `PUT /lol-chat/v1/me` that takes a disguise of winer's (`ours`) off: the client's
/// own rank back where winer saw it, the keys taken out where it never did. `None` when the
/// presence shows none of them, because the client already wrote its own.
pub fn undisguise(
    lol: &HashMap<String, String>,
    ours: &[ShownRank],
    real: Option<&ShownRank>,
) -> Option<Value> {
    if lol.is_empty() || !ours.contains(&ShownRank::of(lol)) {
        return None;
    }
    Some(json!({ "lol": real.cloned().unwrap_or_default().applied_to(lol) }))
}

/// The same with the keys emptied instead of removed, for a client whose `PUT` kept them.
pub fn blank_rank(lol: &HashMap<String, String>) -> Value {
    json!({ "lol": ShownRank::blank().applied_to(lol) })
}

// ---- The presence winer keeps ------------------------------------------------------------------

/// How long after connecting to a client, and after a game, a reset status is put back: long
/// enough for the client's own resets, short enough that a status picked in the client afterwards
/// is the player's to keep.
pub const STATUS_WINDOW: Duration = Duration::from_secs(60);
/// A correction the client undid sooner than this was refused: a client that refuses one puts its
/// own value back within a second or two, while its own updates come minutes apart.
const REFUSED_WITHIN: Duration = Duration::from_secs(10);
/// Corrections refused in a row before winer stops sending them.
const MAX_REFUSALS: u8 = 3;

/// The status message that goes with the mobile state under the mobile-message switch (`on`):
/// [`PresenceRule::MOBILE_MESSAGE`] once the client shows that state with no message, nothing where
/// winer's no longer belongs (another state, or the switch off). `None` leaves the message as it
/// is, which a message of the user's own always is, and so is any while the client sets the state
/// itself (`dnd`).
pub fn mobile_message_for(availability: &str, current: &str, on: bool) -> Option<String> {
    if !PresenceRule::AVAILABILITIES.contains(&availability) {
        return None;
    }
    if on && availability == "mobile" {
        current
            .trim()
            .is_empty()
            .then(|| PresenceRule::MOBILE_MESSAGE.to_owned())
    } else {
        (current == PresenceRule::MOBILE_MESSAGE).then(String::new)
    }
}

/// The client sets the status itself from champ select to the end of the game (`dnd`).
pub fn client_owns_status(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::ChampSelect | Phase::GameStart | Phase::InProgress | Phase::Reconnect
    )
}

/// One step on the way out of a game, champ select included: somewhere along it the client puts
/// its own status back, so each step watches for that afresh.
pub fn after_a_game(previous: Phase, phase: Phase) -> bool {
    let in_a_game = |phase| {
        client_owns_status(phase)
            || matches!(
                phase,
                Phase::WaitingForStats | Phase::PreEndOfGame | Phase::EndOfGame
            )
    };
    previous != phase && in_a_game(previous) && !client_owns_status(phase)
}

/// A correction of the chat presence.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    /// The body of `PUT /lol-chat/v1/me`.
    pub body: Value,
    /// The status it puts back, if it does.
    pub availability: Option<String>,
    /// The disguise it writes, if it does.
    rank: Option<ShownRank>,
}

/// Whether a correction may go out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    Send,
    /// The client refused one correction too many; say so, once.
    GiveUp,
    /// Given up already.
    Stopped,
}

/// What winer knows about one connection's chat presence. It corrects the presence once per change
/// and stops when the client keeps undoing it, so it never fights the client in a loop.
#[derive(Debug, Default)]
pub struct Keeper {
    /// The rank the client itself shows, learned from what it wrote; `None` while unknown.
    real: Option<ShownRank>,
    /// The last disguise winer wrote, which is never the client's own however the rule changed.
    wrote: Option<ShownRank>,
    /// A reset status is put back until then.
    window_until: Option<Instant>,
    /// A correction waits for the client to settle.
    pub scheduled: bool,
    last_fix: Option<Instant>,
    refusals: u8,
    gave_up: bool,
}

impl Keeper {
    /// Watches for the client resetting the status, from `now` for [`STATUS_WINDOW`].
    pub fn open_window(&mut self, now: Instant) {
        self.window_until = Some(now + STATUS_WINDOW);
    }

    /// Starts counting again: the user changed what winer keeps.
    pub fn reset(&mut self) {
        self.last_fix = None;
        self.refusals = 0;
        self.gave_up = false;
    }

    /// The client's own rank, as far as winer has seen it.
    pub fn real(&self) -> Option<&ShownRank> {
        self.real.as_ref()
    }

    /// The disguises that are winer's: `rule`'s and the last one written, which differ when the
    /// rule changed before it went out.
    pub fn ours(&self, rule: &RankDisguise) -> Vec<ShownRank> {
        let current = ShownRank::disguise(rule);
        let earlier = self.wrote.clone().filter(|wrote| *wrote != current);
        std::iter::once(current).chain(earlier).collect()
    }

    /// Learns the client's own rank from a reading: any rank it shows other than a disguise is the
    /// client's, since winer writes nothing else.
    pub fn observe(&mut self, lol: &HashMap<String, String>, disguise: Option<&RankDisguise>) {
        if lol.is_empty() {
            return;
        }
        let shown = ShownRank::of(lol);
        let ours = disguise.is_some_and(|rule| shown == ShownRank::disguise(rule))
            || self.wrote.as_ref() == Some(&shown);
        if !ours {
            self.real = Some(shown);
        }
    }

    /// Notes a correction that went out.
    pub fn sent(&mut self, fix: &Fix) {
        if let Some(rank) = &fix.rank {
            self.wrote = Some(rank.clone());
        }
    }

    /// What puts `me` right under `settings`: the disguised rank whenever it is on, the remembered
    /// status and message (`PresenceRule::kept_message`) only inside the window and outside a game,
    /// never over the client's own `dnd`. `None` when nothing needs to change.
    pub fn plan(
        &self,
        me: &ChatMe,
        settings: &ProfileSettings,
        phase: Phase,
        now: Instant,
    ) -> Option<Fix> {
        let mut body = Map::new();
        let mut rank = None;
        let disguise = &settings.rank_disguise;
        // A presence without `lol` belongs to a client still signing in to chat.
        if disguise.enabled && !me.lol.is_empty() {
            let wanted = ShownRank::disguise(disguise);
            if ShownRank::of(&me.lol) != wanted {
                body.insert("lol".into(), json!(wanted.applied_to(&me.lol)));
                rank = Some(wanted);
            }
        }
        let rule = &settings.presence;
        let mut availability = None;
        let watching = self.window_until.is_some_and(|until| now < until);
        if rule.remember && watching && !client_owns_status(phase) {
            if me.availability != rule.availability
                && me.availability != "dnd"
                && PresenceRule::AVAILABILITIES.contains(&rule.availability.as_str())
            {
                body.insert("availability".into(), json!(rule.availability));
                availability = Some(rule.availability.clone());
            }
            if let Some(message) = rule.kept_message(&me.status_message)
                && message != me.status_message
            {
                body.insert("statusMessage".into(), json!(message));
            }
        }
        (!body.is_empty()).then_some(Fix {
            body: Value::Object(body),
            availability,
            rank,
        })
    }

    /// Whether to send a correction at `now`. One the client undid within [`REFUSED_WITHIN`] was
    /// refused; after [`MAX_REFUSALS`] of those in a row winer stops, until the user changes the
    /// rule or the client connects again.
    pub fn admit(&mut self, now: Instant) -> Admit {
        if self.gave_up {
            return Admit::Stopped;
        }
        let undone = self
            .last_fix
            .is_some_and(|at| now.duration_since(at) < REFUSED_WITHIN);
        self.refusals = if undone { self.refusals + 1 } else { 0 };
        if self.refusals >= MAX_REFUSALS {
            self.gave_up = true;
            return Admit::GiveUp;
        }
        self.last_fix = Some(now);
        Admit::Send
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;

    fn chat_me() -> ChatMe {
        fixture("live/profile/chat-me.json")
    }

    fn ranked(lol: &mut HashMap<String, String>, tier: &str, division: &str) {
        lol.insert("rankedLeagueQueue".into(), "RANKED_SOLO_5x5".into());
        lol.insert("rankedLeagueTier".into(), tier.into());
        lol.insert("rankedLeagueDivision".into(), division.into());
    }

    fn disguise(tier: Tier, division: Division) -> RankDisguise {
        RankDisguise {
            enabled: true,
            queue: DisguiseQueue::Solo,
            tier,
            division,
        }
    }

    #[test]
    fn the_picker_offers_every_skin_but_a_modes_copies() {
        let choices = skins(fixture("live/profile/skins-minimal.json"));
        assert!(
            choices.iter().all(|skin| skin.champion_id < MODE_COPIES),
            "Jade_Annie's skins repeat Annie's"
        );
        assert_eq!(
            choices.iter().map(|skin| skin.id).collect::<Vec<_>>(),
            vec![1000, 1001, 7055, 103000, 103001, 103015, 103016]
        );
        let kda = choices.iter().find(|skin| skin.id == 103015).unwrap();
        assert_eq!(
            (kda.name.as_str(), kda.owned, kda.base),
            ("K/DA 阿狸", true, false)
        );
        assert!(
            kda.tile.starts_with("/lol-game-data/assets/"),
            "{}",
            kda.tile
        );
        assert!(kda.splash.contains("splash_centered"), "{}", kda.splash);
        let base = choices.iter().find(|skin| skin.id == 103000).unwrap();
        assert!(base.base && base.owned);
        assert!(
            !choices.iter().find(|skin| skin.id == 1001).unwrap().owned,
            "unowned skins are offered too"
        );
    }

    #[test]
    fn the_background_reads_and_sets_by_skin_id() {
        let profile: SummonerProfile = fixture("live/profile/summoner-profile.json");
        assert_eq!(profile.background(), Some(103015));
        assert_eq!(SummonerProfile::default().background(), None);
        assert_eq!(
            background_request(103015),
            json!({ "key": "backgroundSkinId", "value": 103015 })
        );
    }

    #[test]
    fn selected_tokens_keep_their_slots_whatever_joins_them() {
        assert_eq!(selected_tokens("101304"), vec![101304]);
        assert_eq!(
            selected_tokens("101304,101206, 505005"),
            vec![101304, 101206, 505005]
        );
        assert_eq!(selected_tokens(""), Vec::<i64>::new());
        assert_eq!(
            selected_tokens("3,3,0,4,5,6"),
            vec![3, 4, 5],
            "three at most, each once"
        );
    }

    #[test]
    fn the_challenge_profile_names_the_slots_and_offers_what_has_a_level() {
        let challenges: HashMap<String, ClientChallenge> =
            fixture("live/profile/challenges-local-player.json");
        let mut summary: ChallengeSummary = fixture("live/profile/challenges-summary.json");
        let titles: Vec<ClientTitle> = fixture("live/profile/titles-local-player.json");

        let profile = challenge_profile(&challenges, &summary, &titles, &banners(), &regalia());
        assert_eq!(profile.tokens.len(), 1);
        let token = &profile.tokens[0];
        assert_eq!(
            (token.id, token.name.as_str(), token.level),
            (101304, "闪电战", Some(Tier::Master))
        );
        assert_eq!(
            token.icon,
            "/lol-game-data/assets/ASSETS/Challenges/Config/101304/Tokens/MASTER.png"
        );
        assert_eq!(
            profile.title,
            Some(TitleChoice {
                id: 1436,
                name: "日光浴恶魔".into()
            })
        );
        assert!(
            profile.challenges.iter().all(|choice| choice.id != 101102),
            "a challenge without a level is not offered"
        );
        assert_eq!(
            profile
                .challenges
                .iter()
                .map(|choice| choice.level)
                .collect::<Vec<_>>()[..3],
            [Some(Tier::Master), Some(Tier::Master), Some(Tier::Diamond)],
            "highest first"
        );
        assert_eq!(profile.titles.len(), 6);
        assert_eq!(profile.titles[0].name, "初窥门径");

        // A token the local list does not describe is still named from the summary, and one
        // neither knows still holds its slot.
        summary.selected_challenges_string = "999,101304".into();
        let profile = challenge_profile(
            &HashMap::new(),
            &summary,
            &titles,
            &HashMap::new(),
            &Regalia::default(),
        );
        assert_eq!(
            profile
                .tokens
                .iter()
                .map(|token| (token.id, token.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(999, ""), (101304, "闪电战")]
        );
        assert_eq!(profile.banner, "");
        assert!(
            profile.banners.is_empty(),
            "no banners where the client lists none"
        );
    }

    #[test]
    fn preferences_go_out_in_slot_order_with_the_title_as_text() {
        assert_eq!(
            preferences_request(&[505005, 101304], Some(1436), None),
            json!({ "challengeIds": [505005, 101304], "title": "1436" })
        );
        assert_eq!(
            preferences_request(&[], None, None),
            json!({ "challengeIds": [] })
        );
        assert_eq!(
            preferences_request(&[101101], None, Some("24")),
            json!({ "challengeIds": [101101], "bannerAccent": "24" })
        );
        assert_eq!(
            preferences_request(&[101101], None, Some(""))["bannerAccent"],
            "",
            "the default goes out as an empty accent, which the client reads back as none"
        );
        assert!(check_tokens(&[1, 2, 3]).is_ok());
        assert!(check_tokens(&[1, 2, 3, 4]).is_err());
        assert!(check_tokens(&[1, 1]).is_err());
        assert!(check_tokens(&[0]).is_err());
    }

    #[test]
    fn a_profile_shows_what_was_asked_only_slot_for_slot() {
        let challenges: HashMap<String, ClientChallenge> =
            fixture("live/profile/challenges-local-player.json");
        let mut summary: ChallengeSummary = fixture("live/profile/challenges-summary.json");
        let profile = challenge_profile(&challenges, &summary, &[], &banners(), &regalia());
        assert!(shows(&profile, &[101304], Some(1436), None));
        assert!(shows(&profile, &[101304], None, Some("")));
        assert!(!shows(&profile, &[101304, 505005], Some(1436), None));
        assert!(!shows(&profile, &[101304], Some(1), None));
        assert!(!shows(&profile, &[101304], None, Some("24")));

        summary.banner_id = "24".into();
        let profile = challenge_profile(&challenges, &summary, &[], &banners(), &regalia());
        assert!(shows(&profile, &[101304], Some(1436), Some("24")));
        assert!(!shows(&profile, &[101304], Some(1436), Some("")));
    }

    fn banners() -> HashMap<String, ClientBanner> {
        fixture("live/profile/regalia-banners.json")
    }

    fn regalia() -> Regalia {
        fixture("live/profile/regalia.json")
    }

    #[test]
    fn the_banner_picker_offers_the_default_and_the_banners_the_player_owns() {
        let regalia = regalia();
        let choices = banner_choices(&banners(), &regalia, "");
        assert_eq!(
            choices
                .iter()
                .map(|choice| (choice.id.as_str(), choice.kind, choice.name.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("", BannerKind::Default, ""),
                ("6", BannerKind::Event, "北极星(2023)贵族旗帜"),
                ("24", BannerKind::Event, "魄罗之王的旗帜"),
            ],
            "no rank banner without last season's rank, and no banner the player does not own"
        );
        assert_eq!(
            choices[0].art,
            "/lol-game-data/assets/ASSETS/Regalia/BannerSkins/default.png"
        );
        assert_eq!(
            choices[2].art,
            "/lol-game-data/assets/ASSETS/Regalia/BannerSkins/ARAM_Banner.png"
        );

        // With a rank last season, its banner comes second, in that tier's art.
        let ranked = Regalia {
            last_season_highest_rank: Some("GOLD".into()),
            ..regalia.clone()
        };
        let choices = banner_choices(&banners(), &ranked, "");
        assert_eq!(
            (choices[1].id.as_str(), choices[1].kind),
            (RANK_BANNER, BannerKind::Rank)
        );
        assert!(choices[1].art.ends_with("/gold.png"), "{}", choices[1].art);

        // The banner shown is listed even when it is not owned, so the window can draw it.
        let shown = banner_choices(&banners(), &regalia, "3");
        assert!(shown.iter().any(|choice| choice.id == "3"));
        assert!(banner_choices(&HashMap::new(), &regalia, "").is_empty());
    }

    #[test]
    fn the_banner_shown_is_read_as_the_clients_customizer_reads_it() {
        assert_eq!(shown_banner("", "blank"), "", "none chosen: the default");
        assert_eq!(shown_banner("1", "blank"), "", "the default by its id");
        assert_eq!(shown_banner("24", "blank"), "24");
        assert_eq!(
            shown_banner("", "lastSeasonHighestRank"),
            RANK_BANNER,
            "none chosen and drawn by rank: the rank banner"
        );
        assert_eq!(shown_banner("24", "lastSeasonHighestRank"), "24");
        assert_eq!(shown_banner("1", "lastSeasonHighestRank"), "");
    }

    #[test]
    fn a_banner_changes_the_regalia_only_where_it_is_drawn_otherwise() {
        let regalia = regalia();
        assert_eq!(regalia_request(&regalia, "24"), None, "drawn plain already");
        assert_eq!(regalia_request(&regalia, ""), None);
        assert_eq!(
            regalia_request(&regalia, RANK_BANNER),
            Some(json!({
                "preferredCrestType": "prestige",
                "preferredBannerType": "lastSeasonHighestRank",
                "selectedPrestigeCrest": 14
            }))
        );
        let by_rank = Regalia {
            preferred_banner_type: "lastSeasonHighestRank".into(),
            ..regalia
        };
        assert_eq!(
            regalia_request(&by_rank, "24").unwrap()["preferredBannerType"],
            "blank"
        );
        assert_eq!(
            regalia_request(&Regalia::default(), RANK_BANNER),
            None,
            "a regalia not read is not written"
        );
        assert!(check_banner("").is_ok());
        assert!(check_banner("24").is_ok());
        assert!(check_banner("2a").is_err());
        assert!(check_banner("-1").is_err());
    }

    #[test]
    fn the_mobile_message_goes_up_with_the_mobile_state_and_down_after_it() {
        let mobile = PresenceRule::MOBILE_MESSAGE;
        assert_eq!(
            mobile_message_for("mobile", "", true),
            Some(mobile.to_owned())
        );
        assert_eq!(
            mobile_message_for("mobile", "今晚上分", true),
            None,
            "the user's own message stays"
        );
        assert_eq!(
            mobile_message_for("mobile", mobile, true),
            None,
            "up already"
        );
        assert_eq!(
            mobile_message_for("chat", mobile, true),
            Some(String::new()),
            "another state takes it down"
        );
        assert_eq!(mobile_message_for("away", "今晚上分", true), None);
        assert_eq!(
            mobile_message_for("mobile", mobile, false),
            Some(String::new()),
            "switched off, winer's message goes"
        );
        assert_eq!(mobile_message_for("mobile", "", false), None);
        assert_eq!(
            mobile_message_for("dnd", mobile, true),
            None,
            "in a game the client sets the state, and the message waits"
        );
    }

    #[test]
    fn a_disguise_changes_the_three_rank_keys_and_nothing_else() {
        let me = chat_me();
        let rule = disguise(Tier::Diamond, Division::Two);
        let lol = ShownRank::disguise(&rule).applied_to(&me.lol);
        assert_eq!(lol["rankedLeagueQueue"], "RANKED_SOLO_5x5");
        assert_eq!(lol["rankedLeagueTier"], "DIAMOND");
        assert_eq!(lol["rankedLeagueDivision"], "II");
        assert_eq!(
            lol.len(),
            me.lol.len() + 3,
            "the client's own keys all go back"
        );
        assert_eq!(lol["regalia"], me.lol["regalia"]);

        let apex = ShownRank::disguise(&RankDisguise {
            queue: DisguiseQueue::Flex,
            ..disguise(Tier::Grandmaster, Division::Three)
        })
        .applied_to(&me.lol);
        assert_eq!(
            (
                lol_value(&apex, "rankedLeagueQueue"),
                lol_value(&apex, "rankedLeagueTier"),
                lol_value(&apex, "rankedLeagueDivision")
            ),
            ("RANKED_FLEX_SR", "GRANDMASTER", "NA"),
            "no division from Master up"
        );
    }

    fn lol_value<'a>(lol: &'a HashMap<String, String>, key: &str) -> &'a str {
        lol.get(key).map_or("", String::as_str)
    }

    #[test]
    fn turning_the_disguise_off_puts_the_real_rank_back_or_removes_it() {
        let rule = disguise(Tier::Challenger, Division::One);
        let mut lol = chat_me().lol;
        ranked(&mut lol, "GOLD", "III");
        let real = ShownRank::of(&lol);
        let disguised = ShownRank::disguise(&rule).applied_to(&lol);

        let ours = [ShownRank::disguise(&rule)];
        let body = undisguise(&disguised, &ours, Some(&real)).unwrap();
        assert_eq!(body["lol"]["rankedLeagueTier"], "GOLD");
        assert_eq!(body["lol"]["rankedLeagueDivision"], "III");

        // Never read, as for a player with no rank: the keys go.
        let body = undisguise(&disguised, &ours, None).unwrap();
        assert!(body["lol"].get("rankedLeagueTier").is_none());
        assert_eq!(body["lol"]["gameStatus"], "outOfGame");

        assert_eq!(
            undisguise(&lol, &ours, Some(&real)),
            None,
            "the client already shows its own"
        );
        assert_eq!(blank_rank(&disguised)["lol"]["rankedLeagueTier"], "");
    }

    #[test]
    fn the_keeper_learns_the_real_rank_from_what_is_not_the_disguise() {
        let rule = disguise(Tier::Challenger, Division::One);
        let mut keeper = Keeper::default();
        let mut lol = chat_me().lol;
        ranked(&mut lol, "SILVER", "I");
        keeper.observe(&lol, Some(&rule));
        assert_eq!(keeper.real(), Some(&ShownRank::of(&lol)));

        // What winer wrote itself teaches nothing.
        keeper.observe(&ShownRank::disguise(&rule).applied_to(&lol), Some(&rule));
        assert_eq!(keeper.real(), Some(&ShownRank::of(&lol)));

        let mut fresh = Keeper::default();
        fresh.observe(&HashMap::new(), Some(&rule));
        assert_eq!(fresh.real(), None, "an empty presence says nothing");
        fresh.observe(&chat_me().lol, None);
        assert_eq!(
            fresh.real(),
            Some(&ShownRank::default()),
            "unranked: no keys"
        );
    }

    #[test]
    fn the_keeper_disguises_whenever_but_puts_the_status_back_only_in_its_window() {
        let now = Instant::now();
        let mut settings = ProfileSettings::default();
        let me = chat_me();
        let mut keeper = Keeper::default();
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now),
            None,
            "both off"
        );

        settings.rank_disguise = disguise(Tier::Diamond, Division::One);
        let fix = keeper.plan(&me, &settings, Phase::InProgress, now).unwrap();
        assert_eq!(fix.body["lol"]["rankedLeagueTier"], "DIAMOND");
        assert_eq!(fix.availability, None);
        let mut disguised = me.clone();
        disguised.lol = ShownRank::disguise(&settings.rank_disguise).applied_to(&me.lol);
        assert_eq!(keeper.plan(&disguised, &settings, Phase::None, now), None);
        let mut signing_in = me.clone();
        signing_in.lol.clear();
        assert_eq!(keeper.plan(&signing_in, &settings, Phase::None, now), None);

        settings.rank_disguise.enabled = false;
        settings.presence = PresenceRule {
            remember: true,
            availability: "offline".into(),
            status_message: Some("今晚上分".into()),
            mobile_message: false,
        };
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now),
            None,
            "outside the window a status is the player's own"
        );
        keeper.open_window(now);
        let fix = keeper.plan(&me, &settings, Phase::Lobby, now).unwrap();
        assert_eq!(fix.body, json!({ "availability": "offline" }));
        assert_eq!(fix.availability.as_deref(), Some("offline"));
        assert_eq!(
            keeper.plan(&me, &settings, Phase::ChampSelect, now),
            None,
            "the client owns the status in a game"
        );
        let mut in_game = me.clone();
        in_game.availability = "dnd".into();
        assert_eq!(
            keeper.plan(&in_game, &settings, Phase::EndOfGame, now),
            None
        );
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now + STATUS_WINDOW),
            None,
            "the window closes"
        );

        settings.presence.status_message = Some("下班了".into());
        let fix = keeper.plan(&me, &settings, Phase::None, now).unwrap();
        assert_eq!(
            fix.body,
            json!({ "availability": "offline", "statusMessage": "下班了" })
        );
    }

    #[test]
    fn the_keeper_puts_the_mobile_message_back_with_the_mobile_state() {
        let now = Instant::now();
        let mut keeper = Keeper::default();
        keeper.open_window(now);
        let mut settings = ProfileSettings {
            presence: PresenceRule {
                remember: true,
                availability: "mobile".into(),
                status_message: None,
                mobile_message: true,
            },
            ..ProfileSettings::default()
        };
        // The client came back online with no message.
        let mut me = chat_me();
        me.status_message = String::new();
        let fix = keeper.plan(&me, &settings, Phase::None, now).unwrap();
        assert_eq!(
            fix.body,
            json!({ "availability": "mobile", "statusMessage": PresenceRule::MOBILE_MESSAGE })
        );

        // A message the client shows while none is kept is left, as without the mobile message.
        me.status_message = "今晚上分".into();
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now).unwrap().body,
            json!({ "availability": "mobile" })
        );
        // One the user keeps comes back instead of the mobile message.
        settings.presence.status_message = Some("下班了".into());
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now).unwrap().body,
            json!({ "availability": "mobile", "statusMessage": "下班了" })
        );
        // Kept empty, the state's message goes up whatever the client shows.
        settings.presence.status_message = Some(String::new());
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now).unwrap().body,
            json!({ "availability": "mobile", "statusMessage": PresenceRule::MOBILE_MESSAGE })
        );

        // Another remembered state brings no mobile message.
        settings.presence.availability = "away".into();
        settings.presence.status_message = None;
        me.status_message = String::new();
        assert_eq!(
            keeper.plan(&me, &settings, Phase::None, now).unwrap().body,
            json!({ "availability": "away" })
        );
    }

    #[test]
    fn the_keeper_stops_after_the_client_undoes_three_corrections_in_a_row() {
        let start = Instant::now();
        let mut keeper = Keeper::default();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        assert_eq!(keeper.admit(at(0)), Admit::Send);
        assert_eq!(keeper.admit(at(2)), Admit::Send, "undone once");
        assert_eq!(keeper.admit(at(4)), Admit::Send, "undone twice");
        assert_eq!(keeper.admit(at(6)), Admit::GiveUp, "a third time: stop");
        assert_eq!(keeper.admit(at(600)), Admit::Stopped);

        keeper.reset();
        assert_eq!(
            keeper.admit(at(601)),
            Admit::Send,
            "the user changed the rule"
        );
        assert_eq!(keeper.admit(at(603)), Admit::Send);
        assert_eq!(
            keeper.admit(at(700)),
            Admit::Send,
            "a correction that held for a while was not refused"
        );
        assert_eq!(keeper.admit(at(702)), Admit::Send);
    }

    #[test]
    fn a_disguise_winer_wrote_is_never_learned_as_the_real_rank() {
        let first = disguise(Tier::Challenger, Division::One);
        let second = disguise(Tier::Gold, Division::Four);
        let mut lol = chat_me().lol;
        ranked(&mut lol, "SILVER", "II");
        let real = ShownRank::of(&lol);
        let settings = |rule: &RankDisguise| ProfileSettings {
            rank_disguise: rule.clone(),
            ..ProfileSettings::default()
        };
        let mut me = chat_me();
        me.lol = lol;

        let mut keeper = Keeper::default();
        keeper.observe(&me.lol, Some(&first));
        let fix = keeper
            .plan(&me, &settings(&first), Phase::None, Instant::now())
            .unwrap();
        keeper.sent(&fix);
        me.lol = ShownRank::disguise(&first).applied_to(&me.lol);

        // The user picks another tier: the first disguise is still on screen, and is not theirs.
        keeper.observe(&me.lol, Some(&second));
        assert_eq!(keeper.real(), Some(&real));
        // Switched off before the second went out, it is the first that comes off.
        assert_eq!(
            keeper.ours(&second),
            vec![ShownRank::disguise(&second), ShownRank::disguise(&first)]
        );
        assert!(undisguise(&me.lol, &keeper.ours(&second), keeper.real()).is_some());
        assert_eq!(keeper.ours(&first), vec![ShownRank::disguise(&first)]);
        let fix = keeper
            .plan(&me, &settings(&second), Phase::None, Instant::now())
            .unwrap();
        assert_eq!(fix.body["lol"]["rankedLeagueTier"], "GOLD");
        assert_eq!(fix.body["lol"]["rankedLeagueDivision"], "IV");
    }

    #[test]
    fn the_client_owns_the_status_from_champ_select_to_the_end_of_the_game() {
        assert!(client_owns_status(Phase::ChampSelect));
        assert!(client_owns_status(Phase::InProgress));
        assert!(!client_owns_status(Phase::Lobby));
        assert!(!client_owns_status(Phase::EndOfGame));
        assert!(!client_owns_status(Phase::None));
    }

    #[test]
    fn every_step_out_of_a_game_watches_for_the_status_reset() {
        assert!(after_a_game(Phase::InProgress, Phase::WaitingForStats));
        assert!(after_a_game(Phase::WaitingForStats, Phase::EndOfGame));
        assert!(after_a_game(Phase::EndOfGame, Phase::Lobby));
        assert!(after_a_game(Phase::ChampSelect, Phase::Lobby), "a dodge");
        assert!(!after_a_game(Phase::ChampSelect, Phase::GameStart));
        assert!(!after_a_game(Phase::Lobby, Phase::Matchmaking));
        assert!(!after_a_game(Phase::EndOfGame, Phase::EndOfGame));
        assert!(!after_a_game(Phase::None, Phase::Lobby));
    }
}
