//! What the desktop window and the in-client plugin see. Every type here is exported to
//! TypeScript (`bindings.rs`), so a field added on this side is a compile error on the other.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{loadout::PageOutcome, settings::Settings};

/// Everything live, in one document. Changes after it arrive as [`Update`]s.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The revision of the last patch applied; updates at or below it are stale.
    pub rev: u64,
    pub connection: Connection,
    pub me: Option<Me>,
    pub phase: Phase,
    pub champ_select: Option<ChampSelectView>,
    pub game: Option<GameView>,
    // Social: friends' games and the lobby.
    /// `None` until the client has listed the friends once.
    pub friends: Option<FriendsView>,
    /// The party, while the client shows the lobby (in it, in queue, match found).
    pub lobby: Option<LobbyView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub rev: u64,
    pub patch: Patch,
}

/// One top-level field of [`Snapshot`], replaced whole.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "key", content = "value", rename_all = "camelCase")]
pub enum Patch {
    Connection(Connection),
    Me(Option<Me>),
    Phase(Phase),
    ChampSelect(Option<ChampSelectView>),
    Game(Option<GameView>),
    // Social.
    Friends(Option<FriendsView>),
    Lobby(Option<LobbyView>),
}

/// Everything the core pushes, to the window and to the plugin alike.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Event {
    Update(Update),
    Notice(Notice),
    Settings(Box<Settings>),
    /// The game-data catalog changed (a client connected); fetch it again.
    GameData,
    /// Someone asked from inside the client to see a player's games: the shell brings the window
    /// up and the window opens that player's history.
    OpenHistory {
        puuid: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Connection {
    /// No client process is running.
    #[default]
    Searching,
    /// The client runs elevated and winer does not, so its credentials are unreadable.
    AccessDenied,
    /// Credentials found; waiting for the LCU to answer.
    Connecting { port: u16 },
    #[serde(rename_all = "camelCase")]
    Connected { port: u16, platform_id: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Phase {
    #[default]
    None,
    Lobby,
    Matchmaking,
    CheckedIntoTournament,
    ReadyCheck,
    ChampSelect,
    GameStart,
    FailedToLaunch,
    InProgress,
    Reconnect,
    WaitingForStats,
    PreEndOfGame,
    EndOfGame,
    TerminatedInError,
    #[serde(other)]
    Unknown,
}

impl Phase {
    pub fn parse(value: &str) -> Self {
        serde_json::from_value(serde_json::Value::String(value.to_owned())).unwrap_or(Self::Unknown)
    }

    /// A game is loaded or running: the gameflow session holds both teams.
    pub fn in_game(self) -> bool {
        matches!(self, Self::GameStart | Self::InProgress | Self::Reconnect)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RiotId {
    pub game_name: String,
    pub tag_line: String,
}

impl RiotId {
    pub fn new(game_name: &str, tag_line: &str) -> Option<Self> {
        (!game_name.is_empty()).then(|| Self {
            game_name: game_name.to_owned(),
            tag_line: tag_line.to_owned(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub level: i64,
    pub icon_id: i64,
    pub ranked: Ranked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Tier {
    Iron,
    Bronze,
    Silver,
    Gold,
    Platinum,
    Emerald,
    Diamond,
    Master,
    Grandmaster,
    Challenger,
}

impl Tier {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_uppercase().as_str() {
            "IRON" => Self::Iron,
            "BRONZE" => Self::Bronze,
            "SILVER" => Self::Silver,
            "GOLD" => Self::Gold,
            "PLATINUM" => Self::Platinum,
            "EMERALD" => Self::Emerald,
            "DIAMOND" => Self::Diamond,
            "MASTER" => Self::Master,
            "GRANDMASTER" => Self::Grandmaster,
            "CHALLENGER" => Self::Challenger,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Rank {
    pub tier: Tier,
    /// `I`–`IV`; absent from Master upwards.
    pub division: Option<String>,
    pub lp: i64,
    pub wins: i64,
    pub losses: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Ranked {
    pub solo: Option<Rank>,
    pub flex: Option<Rank>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Position {
    Top,
    Jungle,
    Middle,
    Bottom,
    Utility,
}

impl Position {
    pub const ALL: [Self; 5] = [
        Self::Top,
        Self::Jungle,
        Self::Middle,
        Self::Bottom,
        Self::Utility,
    ];

    /// Champ select says `middle`, gameflow says `MIDDLE`, match timelines say `MID`.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_uppercase().as_str() {
            "TOP" => Self::Top,
            "JUNGLE" => Self::Jungle,
            "MIDDLE" | "MID" => Self::Middle,
            "BOTTOM" | "BOT" => Self::Bottom,
            "UTILITY" | "SUPPORT" => Self::Utility,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectView {
    pub game_id: i64,
    pub queue_id: i64,
    pub timer: TimerView,
    pub my_team: Vec<Seat>,
    pub their_team: Vec<Seat>,
    pub my_bans: Vec<i64>,
    pub their_bans: Vec<i64>,
    /// ARAM's shared bench, and whether this mode has one.
    pub bench_enabled: bool,
    pub bench: Vec<i64>,
    pub rerolls_remaining: i64,
    /// The callout as it would be sent now, one chat line per rated teammate.
    pub callout: Vec<String>,
    /// The local team's side; `None` where the mode has none (Arena, Swarm).
    pub side: Option<Side>,
}

/// Where a team starts on a map of two sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Blue,
    Red,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TimerView {
    pub phase: String,
    /// Epoch milliseconds at which the phase ends; zero when it never does.
    pub ends_at: i64,
    pub total_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub game_id: i64,
    pub queue_id: i64,
    /// In the client's order: on a map of two sides, blue first.
    pub teams: Vec<Vec<Seat>>,
    /// The teams are the blue and the red side; Arena's and Swarm's are not.
    pub sides: bool,
}

/// One player slot in champ select or in a running game.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Seat {
    /// Absent for hidden players (enemies before load, anonymised ranked lobbies) and bots.
    pub puuid: Option<String>,
    pub name: Option<RiotId>,
    pub champion_id: i64,
    /// The champion shown is a declared intent, not a pick.
    pub intent: bool,
    pub position: Option<Position>,
    pub spells: [i64; 2],
    pub is_self: bool,
    /// Players sharing a number came as one premade party.
    pub premade: Option<u8>,
    pub stats: PlayerStats,
    /// Recent form and the tier it earns within the team; absent until stats arrive.
    pub rating: Option<SeatRating>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SeatRating {
    /// 0–10, see `rating::form_score`.
    pub score: f64,
    /// 0 is the best of `tiers` tiers.
    pub tier: u8,
    pub tiers: u8,
    /// The tier's name in the user's words (`峡谷公务员` by default).
    pub label: String,
    /// The grade, 0 (S+) to 7 (F), when the scheme grades on fixed bands instead of ranking the
    /// team (峡谷八档); it is then the same number as `tier`.
    pub grade: Option<u8>,
    /// What recent games say beyond the tier (`版本答案`), when titles are on and one applies.
    pub title: Option<String>,
    /// A line about the tier for the chat and the tooltip (`稳得离谱，能C还能活`).
    pub quip: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum PlayerStats {
    Loading,
    Hidden,
    Failed { message: String },
    Ready(Box<PlayerSummary>),
}

/// A player at a glance: identity, rank and the last twenty games.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSummary {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub level: i64,
    pub icon_id: i64,
    /// The profile is private: rank and history are not shown to others.
    pub private: bool,
    pub ranked: Ranked,
    pub recent: RecentForm,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecentForm {
    /// Counted games: remakes are left out of every figure below.
    pub games: u32,
    pub wins: u32,
    pub kills: f64,
    pub deaths: f64,
    pub assists: f64,
    /// `+n` for n wins in a row, `-n` for n losses.
    pub streak: i32,
    /// Newest first.
    pub matches: Vec<RecentMatch>,
    /// Most played first.
    pub champions: Vec<ChampionForm>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecentMatch {
    pub game_id: i64,
    pub queue_id: i64,
    pub champion_id: i64,
    pub win: bool,
    pub remake: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    /// Epoch milliseconds.
    pub started_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampionForm {
    pub champion_id: i64,
    pub games: u32,
    pub wins: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchPage {
    pub puuid: String,
    pub begin: u32,
    pub games: Vec<MatchSummary>,
    /// A full page came back, so there may be more.
    pub has_more: bool,
    pub source: HistorySource,
}

/// Where a page of history came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum HistorySource {
    /// The shard's match-history server: the whole history, page by page.
    Server,
    /// The client's own: on a Tencent shard only its newest twenty games, as one page.
    Client,
}

/// One game from one player's point of view.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub game_id: i64,
    pub queue_id: i64,
    pub game_mode: String,
    pub started_at: i64,
    /// Seconds.
    pub duration: i64,
    pub line: PlayerLine,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetail {
    pub game_id: i64,
    pub queue_id: i64,
    pub game_mode: String,
    pub game_version: String,
    pub started_at: i64,
    pub duration: i64,
    pub teams: Vec<TeamDetail>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamDetail {
    pub team_id: i64,
    pub win: bool,
    pub bans: Vec<i64>,
    pub kills: i64,
    pub gold: i64,
    pub towers: i64,
    pub dragons: i64,
    pub barons: i64,
    pub players: Vec<PlayerLine>,
}

/// One participant's line on the scoreboard.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerLine {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub icon_id: i64,
    pub champion_id: i64,
    pub champion_level: i64,
    pub position: Option<Position>,
    pub spells: [i64; 2],
    pub items: [i64; 7],
    /// Arena and Hextech ARAM augments, in the order picked.
    pub augments: Vec<i64>,
    pub keystone: i64,
    pub sub_style: i64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub cs: i64,
    pub gold: i64,
    pub damage: i64,
    pub damage_taken: i64,
    pub vision: i64,
    pub largest_multi_kill: i64,
    pub win: bool,
    pub remake: bool,
    /// Arena placement, 1–8.
    pub placement: Option<i64>,
    /// Share of the team's damage to champions, 0–1. Only a full scoreboard knows the team.
    pub damage_share: Option<f64>,
    /// Kills plus assists over the team's kills, 0–1.
    pub kill_participation: Option<f64>,
    /// 0–10, see `rating::game_scores`; absent for remakes and single-player pages.
    pub score: Option<f64>,
    /// The score's grade on `rating::GAME_GRADES`, 0 (S+) to 7 (F).
    pub grade: Option<u8>,
    pub award: Option<Award>,
    /// What the line did that a badge names, most telling first (see [`Feat`]).
    pub feats: Vec<Feat>,
}

/// What one line did in one game that a badge names: alone (a multikill, a spree, first blood,
/// going AFK) or against everyone in the game (the most kills, gold and so on, where somebody
/// leads). Declared most telling first, the order the badges are shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Feat {
    /// The game marked the player away: they left or idled (逃兵).
    Afk,
    Penta,
    Quadra,
    /// Eight kills or more without dying (超神).
    Legendary,
    Triple,
    MostKills,
    MostDamage,
    FirstBlood,
    MostTowers,
    MostAssists,
    MostGold,
    MostTaken,
    MostCs,
    Double,
}

/// The best line of the winning side, and of the losing side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Award {
    Mvp,
    Svp,
}

/// The catalog the window needs to name ids, fetched once per client connection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub champions: Vec<ChampionInfo>,
    pub items: Vec<AssetInfo>,
    pub spells: Vec<AssetInfo>,
    /// Runes and rune styles together; their ids never collide.
    pub perks: Vec<AssetInfo>,
    /// Arena and Hextech ARAM augments.
    pub augments: Vec<AugmentInfo>,
    pub queues: Vec<QueueInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AugmentInfo {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub rarity: Option<Rarity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Silver,
    Gold,
    Prismatic,
}

impl Rarity {
    /// The client's `kSilver` / `kGold` / `kPrismatic`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim_start_matches('k').to_ascii_lowercase().as_str() {
            "silver" => Some(Self::Silver),
            "gold" => Some(Self::Gold),
            "prismatic" => Some(Self::Prismatic),
            _ => None,
        }
    }
}

/// What an augment does, in plain text, as ARAM.GG publishes it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AugmentDetail {
    pub id: i64,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampionInfo {
    pub id: i64,
    /// As the client shows it (`黑暗之女` in zh_CN).
    pub name: String,
    /// The short form (`安妮`); the same as `name` where the locale has none.
    pub short_name: String,
    pub alias: String,
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub id: i64,
    pub name: String,
    /// An LCU asset path, served to the window through the `lcu` protocol.
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QueueInfo {
    pub id: i64,
    pub name: String,
    pub game_mode: String,
    pub ranked: bool,
}

impl QueueInfo {
    pub fn mode(&self) -> crate::settings::Mode {
        crate::settings::Mode::of(&self.game_mode, self.ranked)
    }
}

/// Something the core did on the user's behalf, or failed to do.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// Epoch milliseconds.
    pub at: i64,
    pub kind: NoticeKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NoticeKind {
    Accepted,
    Declared {
        champion_id: i64,
    },
    Picked {
        champion_id: i64,
        locked: bool,
    },
    Banned {
        champion_id: i64,
    },
    PlayedAgain,
    /// Took a wishlist champion from the ARAM bench.
    Swapped {
        champion_id: i64,
    },
    /// Sent the champ-select callout.
    CalledOut {
        lines: u32,
    },
    /// Put the remembered chat status back after the client reset it.
    PresenceRestored {
        availability: String,
    },
    /// The client kept undoing the remembered status or the disguised rank; winer stopped trying
    /// until the rule changes or the client reconnects.
    PresenceRefused,
    Failed {
        action: String,
        message: String,
    },
    // Runes, spells and item sets (`loadout`).
    /// Set up the runes and summoner spells for the champion just taken.
    LoadoutApplied {
        champion_id: i64,
        /// The client's own recommendation: nothing was remembered for the champion.
        recommended: bool,
        /// What became of the rune page; absent where there were no runes to set up.
        runes: Option<PageOutcome>,
        /// The two summoner spells are the ones set up.
        spells: bool,
    },
    /// Wrote winer's item set for the champion just taken.
    ItemSetWritten {
        champion_id: i64,
    },
}

/// A player found by Riot ID.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub level: i64,
    pub icon_id: i64,
    pub private: bool,
    pub ranked: Ranked,
}

/// The chat presence the client shows to friends.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    /// `chat`, `away`, `dnd`, `mobile` or `offline`.
    pub availability: String,
    pub status_message: String,
}

/// The desktop shell itself: version, privileges and where its files are.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// The process runs elevated; required to read an elevated client's credentials.
    pub elevated: bool,
    pub log_dir: String,
    pub settings_path: String,
    /// The licences of the third-party components shipped inside winer (`THIRD_PARTY_NOTICES.md`).
    pub notices: String,
}

/// The updater's progress, owned by the shell and broadcast to the window.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate {
        version: String,
        checked_at: i64,
    },
    Available {
        version: String,
        current: String,
        notes: Option<String>,
        date: Option<String>,
    },
    Downloading {
        version: String,
        received: u64,
        total: Option<u64>,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
    },
}

/// Every command failure, in a shape the window can branch on.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    NotConnected,
    NotFound,
    Invalid,
    Busy,
    Client,
    Internal,
}

// ---- Social: friends' games, the lobby, the hotkey (`friends.rs`, `live.rs`, the shell) ----

/// The friends signed in to chat and what each is playing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FriendsView {
    /// In game first (the longest-running game first), then champ select, in queue, the rest.
    /// Offline friends are left out: nothing shows them, and a long list would ride along with
    /// every patch.
    pub friends: Vec<FriendView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FriendView {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub icon_id: i64,
    /// `chat`, `away`, `dnd` or `mobile`, as the client shows it beside the name.
    pub availability: String,
    pub status: FriendStatus,
    /// Friends in one game, or one party, share a number from 1, which picks the colour they are
    /// drawn in; a friend playing without other friends has none.
    pub group: Option<u8>,
}

/// What a friend is doing, from the presence their client publishes.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FriendStatus {
    /// Signed in, not queued or playing: the home screen, a lobby, the collection.
    OutOfGame,
    /// `mode` is the queue's name in the client's catalog, else the presence's own words for it;
    /// `since` is epoch milliseconds, zero when the presence does not say.
    InQueue {
        mode: String,
        queue_id: i64,
        since: i64,
    },
    ChampSelect {
        mode: String,
        queue_id: i64,
        since: i64,
    },
    InGame {
        mode: String,
        queue_id: i64,
        /// When the game started, epoch milliseconds; zero when the presence does not say.
        started_at: i64,
        /// The game can be spectated.
        observable: bool,
    },
}

/// The party in the lobby.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyView {
    pub queue_id: i64,
    /// A custom game's lobby, where everyone in it plays, on both teams.
    pub custom: bool,
    /// In the lobby's order, the local player among them; bots are left out.
    pub members: Vec<LobbyMember>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyMember {
    pub puuid: String,
    pub name: Option<RiotId>,
    pub icon_id: i64,
    pub is_self: bool,
    pub leader: bool,
    /// The lanes asked for, first choice first; empty in queues without positions.
    pub positions: Vec<LanePreference>,
    pub stats: PlayerStats,
    /// Recent form, 0–10 (`rating::form_score`), once the stats are in.
    pub score: Option<f64>,
}

/// A lane a lobby member asked for: one of the five, or any (补位).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LanePreference {
    Top,
    Jungle,
    Middle,
    Bottom,
    Utility,
    Fill,
}

impl LanePreference {
    /// The lobby's `firstPositionPreference` words: `TOP` … `UTILITY`, `FILL`; `UNSELECTED` and
    /// anything else is no preference.
    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("FILL") {
            return Some(Self::Fill);
        }
        Position::parse(value).map(|position| match position {
            Position::Top => Self::Top,
            Position::Jungle => Self::Jungle,
            Position::Middle => Self::Middle,
            Position::Bottom => Self::Bottom,
            Position::Utility => Self::Utility,
        })
    }
}

/// The global shortcut that summons the window, owned by the shell and broadcast to the window.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    /// The combination the settings name (`Alt+Backquote`); `None` while the shortcut is off.
    pub shortcut: Option<String>,
    /// The system has it registered for winer right now.
    pub active: bool,
    /// Let go while the settings record a new combination.
    pub suspended: bool,
    /// Why the system refused it, in its own words; usually another program holds the combination.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_parse_from_the_wire_and_unknown_values_survive() {
        assert_eq!(Phase::parse("ChampSelect"), Phase::ChampSelect);
        assert_eq!(Phase::parse("None"), Phase::None);
        assert_eq!(Phase::parse("SomethingNew"), Phase::Unknown);
    }

    #[test]
    fn positions_parse_from_every_vocabulary() {
        assert_eq!(Position::parse("middle"), Some(Position::Middle));
        assert_eq!(Position::parse("BOTTOM"), Some(Position::Bottom));
        assert_eq!(Position::parse("MID"), Some(Position::Middle));
        assert_eq!(Position::parse(""), None);
        assert_eq!(Position::parse("NONE"), None);
    }

    #[test]
    fn lobby_lanes_parse_from_the_lobby_and_fill_is_one_of_them() {
        assert_eq!(
            LanePreference::parse("MIDDLE"),
            Some(LanePreference::Middle)
        );
        assert_eq!(
            LanePreference::parse("UTILITY"),
            Some(LanePreference::Utility)
        );
        assert_eq!(LanePreference::parse("FILL"), Some(LanePreference::Fill));
        assert_eq!(LanePreference::parse("UNSELECTED"), None);
        assert_eq!(LanePreference::parse(""), None);
    }

    #[test]
    fn a_history_request_names_the_player_in_its_data() {
        assert_eq!(
            serde_json::to_value(Event::OpenHistory { puuid: "p".into() }).unwrap(),
            serde_json::json!({"type": "openHistory", "data": {"puuid": "p"}})
        );
        let status = FriendStatus::InGame {
            mode: "极地大乱斗".into(),
            queue_id: 450,
            started_at: 5,
            observable: true,
        };
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::json!({"state": "inGame", "mode": "极地大乱斗", "queueId": 450, "startedAt": 5, "observable": true})
        );
    }

    #[test]
    fn patches_serialize_as_key_and_value() {
        let update = Update {
            rev: 3,
            patch: Patch::Phase(Phase::Lobby),
        };
        assert_eq!(
            serde_json::to_value(update).unwrap(),
            serde_json::json!({"rev": 3, "patch": {"key": "phase", "value": "Lobby"}})
        );
    }
}
