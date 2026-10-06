//! LCU response shapes, reduced to the fields winer reads.
//!
//! Every struct defaults every field, so a field the client renames or drops degrades to an empty
//! value instead of failing the whole document. Shapes follow the captures in `fixtures/` and the
//! live client (`docs/platform-notes.md`).

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Summoner {
    pub puuid: String,
    pub summoner_id: i64,
    pub game_name: String,
    pub tag_line: String,
    /// Empty on Tencent shards; never shown.
    pub display_name: String,
    pub summoner_level: i64,
    pub profile_icon_id: i64,
    pub privacy: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RankedStats {
    pub queue_map: HashMap<String, RankedEntry>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RankedEntry {
    /// `""` when unranked.
    pub tier: String,
    /// `"NA"` when no division applies (unranked, Master and above).
    pub division: String,
    pub league_points: i64,
    pub wins: i64,
    pub losses: i64,
    pub is_provisional: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReadyCheck {
    /// `InProgress` while the accept dialog is up.
    pub state: String,
    /// `None`, `Accepted` or `Declined`.
    pub player_response: String,
    pub timer: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GameflowSession {
    pub phase: String,
    pub game_data: GameData,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GameData {
    pub game_id: i64,
    pub is_custom_game: bool,
    pub queue: GameQueue,
    pub team_one: Vec<GamePlayer>,
    pub team_two: Vec<GamePlayer>,
    pub player_champion_selections: Vec<ChampionSelection>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GameQueue {
    pub id: i64,
    pub game_mode: String,
    pub map_id: i64,
    pub is_ranked: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GamePlayer {
    pub puuid: String,
    pub summoner_id: i64,
    pub champion_id: i64,
    pub selected_position: String,
    /// Shared by the members of one premade party.
    pub team_participant_id: i64,
    pub game_name: String,
    pub tag_line: String,
    pub summoner_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampionSelection {
    pub puuid: String,
    pub champion_id: i64,
    pub spell1_id: i64,
    pub spell2_id: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampSelectSession {
    pub game_id: i64,
    pub queue_id: i64,
    pub is_custom_game: bool,
    pub local_player_cell_id: i64,
    pub my_team: Vec<ChampSelectPlayer>,
    pub their_team: Vec<ChampSelectPlayer>,
    /// Turns, each a list of actions taken together.
    pub actions: Vec<Vec<ChampSelectAction>>,
    pub bans: Bans,
    pub timer: Timer,
    pub bench_enabled: bool,
    pub bench_champions: Vec<BenchChampion>,
    pub allow_rerolling: bool,
    pub rerolls_remaining: i64,
}

impl ChampSelectSession {
    pub fn local_player(&self) -> Option<&ChampSelectPlayer> {
        self.my_team
            .iter()
            .find(|player| player.cell_id == self.local_player_cell_id)
    }

    pub fn all_actions(&self) -> impl Iterator<Item = &ChampSelectAction> {
        self.actions.iter().flatten()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampSelectPlayer {
    pub cell_id: i64,
    /// The side, 1 blue and 2 red: a lone player on a custom lobby's red team reads 2
    /// (`docs/platform-notes.md`). Unlike `cell_id`, which stayed 0 there.
    pub team: i64,
    pub champion_id: i64,
    pub champion_pick_intent: i64,
    /// `top`, `jungle`, `middle`, `bottom`, `utility`, or empty outside draft.
    pub assigned_position: String,
    pub puuid: String,
    pub summoner_id: i64,
    pub game_name: String,
    pub tag_line: String,
    pub spell1_id: i64,
    pub spell2_id: i64,
    pub name_visibility_type: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampSelectAction {
    pub id: i64,
    pub actor_cell_id: i64,
    pub champion_id: i64,
    pub completed: bool,
    pub is_in_progress: bool,
    pub is_ally_action: bool,
    /// `pick`, `ban` or bookkeeping turns such as `ten_bans_reveal`.
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Bans {
    pub my_team_bans: Vec<i64>,
    pub their_team_bans: Vec<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Timer {
    /// `PLANNING`, `BAN_PICK`, `FINALIZATION` or `GAME_STARTING`.
    pub phase: String,
    pub adjusted_time_left_in_phase: i64,
    pub internal_now_in_epoch_ms: i64,
    pub total_time_in_phase: i64,
    pub is_infinite: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BenchChampion {
    pub champion_id: i64,
    pub is_priority: bool,
}

/// One row of `/lol-chat/v1/conversations`; champ select's own chat has type `championSelect`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Conversation {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// `/lol-match-history/v1/products/lol/{puuid}/matches`: the page sits one level down.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MatchList {
    pub games: MatchPage,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MatchPage {
    pub games: Vec<Game>,
    pub game_count: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Game {
    pub game_id: i64,
    /// Epoch milliseconds.
    pub game_creation: i64,
    /// Seconds.
    pub game_duration: i64,
    pub game_mode: String,
    pub game_type: String,
    pub queue_id: i64,
    pub map_id: i64,
    pub game_version: String,
    pub participants: Vec<Participant>,
    pub participant_identities: Vec<ParticipantIdentity>,
    pub teams: Vec<Team>,
}

impl Game {
    pub fn identity(&self, participant_id: i64) -> Option<&Player> {
        self.participant_identities
            .iter()
            .find(|identity| identity.participant_id == participant_id)
            .map(|identity| &identity.player)
    }

    /// The participant row belonging to `puuid`.
    pub fn participant_of(&self, puuid: &str) -> Option<&Participant> {
        let id = self
            .participant_identities
            .iter()
            .find(|identity| identity.player.puuid == puuid)?
            .participant_id;
        self.participants
            .iter()
            .find(|participant| participant.participant_id == id)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Participant {
    pub participant_id: i64,
    pub team_id: i64,
    pub champion_id: i64,
    pub spell1_id: i64,
    pub spell2_id: i64,
    pub stats: Stats,
    pub timeline: Timeline,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Stats {
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub champ_level: i64,
    pub total_minions_killed: i64,
    pub neutral_minions_killed: i64,
    pub gold_earned: i64,
    pub total_damage_dealt_to_champions: i64,
    pub total_damage_taken: i64,
    pub damage_self_mitigated: i64,
    pub damage_dealt_to_objectives: i64,
    /// The one field whose casing camelCase cannot derive.
    #[serde(rename = "timeCCingOthers")]
    pub time_ccing_others: i64,
    pub vision_score: i64,
    pub item0: i64,
    pub item1: i64,
    pub item2: i64,
    pub item3: i64,
    pub item4: i64,
    pub item5: i64,
    pub item6: i64,
    pub perk0: i64,
    pub perk_primary_style: i64,
    pub perk_sub_style: i64,
    pub largest_multi_kill: i64,
    pub penta_kills: i64,
    pub quadra_kills: i64,
    pub triple_kills: i64,
    pub double_kills: i64,
    /// Kills in a row without dying; eight or more is 超神 (Legendary).
    pub largest_killing_spree: i64,
    pub first_blood_kill: bool,
    pub turret_kills: i64,
    /// The game marked the player away: they left or idled. Only the shard's server says so.
    pub was_afk: bool,
    pub game_ended_in_early_surrender: bool,
    /// Arena placement, 1–8; zero elsewhere.
    pub subteam_placement: i64,
    /// Arena and Hextech ARAM (KIWI) augments in the order picked; zero for an empty slot.
    pub player_augment1: i64,
    pub player_augment2: i64,
    pub player_augment3: i64,
    pub player_augment4: i64,
    pub player_augment5: i64,
    pub player_augment6: i64,
}

impl Stats {
    pub fn items(&self) -> [i64; 7] {
        [
            self.item0, self.item1, self.item2, self.item3, self.item4, self.item5, self.item6,
        ]
    }

    pub fn augments(&self) -> Vec<i64> {
        [
            self.player_augment1,
            self.player_augment2,
            self.player_augment3,
            self.player_augment4,
            self.player_augment5,
            self.player_augment6,
        ]
        .into_iter()
        .filter(|&id| id > 0)
        .collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Timeline {
    pub lane: String,
    pub role: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ParticipantIdentity {
    pub participant_id: i64,
    pub player: Player,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Player {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub summoner_id: i64,
    pub profile_icon: i64,
    pub platform_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Team {
    pub team_id: i64,
    /// `Win` or `Fail`.
    pub win: String,
    pub bans: Vec<TeamBan>,
    pub baron_kills: i64,
    pub dragon_kills: i64,
    pub tower_kills: i64,
    pub inhibitor_kills: i64,
    pub rift_herald_kills: i64,
    pub horde_kills: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TeamBan {
    pub champion_id: i64,
    pub pick_turn: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Lobby {
    pub game_config: LobbyGameConfig,
    pub members: Vec<LobbyMember>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LobbyGameConfig {
    pub queue_id: i64,
    pub game_mode: String,
    pub is_custom: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LobbyMember {
    pub puuid: String,
    pub summoner_id: i64,
    pub summoner_level: i64,
    pub summoner_icon_id: i64,
    pub is_leader: bool,
    pub first_position_preference: String,
    pub second_position_preference: String,
    /// The Riot ID; chat calls the tag `gameTag`, the summoner `tagLine`.
    pub game_name: String,
    #[serde(alias = "gameTag")]
    pub tag_line: String,
    /// A custom game's bot, which has no history.
    pub is_bot: bool,
}

/// `/entitlements/v1/token`: the client's access token for its shard's own servers. Not `Debug`,
/// so the token cannot reach a log.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entitlements {
    pub access_token: String,
    /// The shard's entitlements host, `http://nj100-bcs-internal.lol.qq.com:28088`.
    pub issuer: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatMe {
    pub availability: String,
    pub status_message: String,
    pub platform_id: String,
    /// String-valued presence fields (`rankedLeagueTier`, `level`, …), all strings on the wire.
    pub lol: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampionSummary {
    pub id: i64,
    /// What the client shows: the title in zh_CN (`黑暗之女`), the name in en_US.
    pub name: String,
    /// The short name in zh_CN (`安妮`).
    pub description: String,
    /// The English key (`Annie`).
    pub alias: String,
    pub square_portrait_path: String,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub icon_path: String,
    pub price_total: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SummonerSpell {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub icon_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Perk {
    pub id: i64,
    pub name: String,
    pub icon_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PerkStyles {
    pub styles: Vec<PerkStyle>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PerkStyle {
    pub id: i64,
    pub name: String,
    pub icon_path: String,
    /// The keystones, three rows of runes and three of shards, in page order.
    pub slots: Vec<PerkSlot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PerkSlot {
    /// `kKeyStone`, `kMixedRegularSplashable` or `kStatMod`.
    #[serde(rename = "type")]
    pub kind: String,
    pub perks: Vec<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Queue {
    pub id: i64,
    pub name: String,
    pub short_name: String,
    pub description: String,
    pub game_mode: String,
    pub map_id: i64,
    pub is_ranked: bool,
    pub category: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ApexLeague {
    pub divisions: Vec<ApexDivision>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ApexDivision {
    pub standings: Vec<ApexStanding>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ApexStanding {
    pub puuid: String,
    pub position: i64,
    pub league_points: i64,
}

/// `/lol-summoner/v1/alias/lookup`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AliasLookup {
    pub puuid: String,
}

/// One row of the client's `cherry-augments.json`, which holds Arena's augments and Hextech ARAM's
/// (`ARAM_*` names) alike. Its ids are strings on 16.19.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientAugment {
    #[serde(deserialize_with = "number_or_text")]
    pub id: i64,
    #[serde(rename = "nameTRA")]
    pub name: String,
    pub augment_small_icon_path: String,
    /// `kSilver`, `kGold` or `kPrismatic`.
    pub rarity: String,
}

/// One entry of ARAM.GG's augment list: only what a player is told.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishedAugment {
    pub description: String,
    pub tooltip: String,
}

/// An id the client writes as `1004` in one file and `"1004"` in another.
fn number_or_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Number(number) => number.as_i64().unwrap_or_default(),
        serde_json::Value::String(text) => text.trim().parse().unwrap_or_default(),
        _ => 0,
    })
}
