//! The connection to the client and everything that follows from it: one loop that finds the
//! client, mirrors its state into a [`Snapshot`] and acts on it for the user.

mod caches;
mod loadout;
mod ux;

pub use ux::{Readiness, UiRestart};

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock},
    time::{Duration, Instant},
};

use lcu::{Credentials, DiscoverError, EventKind, Lcu};
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::{
    runtime::Handle,
    sync::{Semaphore, broadcast, watch},
    time::sleep,
};
use tracing::{debug, info, warn};

use crate::{
    analysis, augments,
    automation::{self, Availability, ChampAction, Step},
    backup::{self, BackupChannel, BackupFile, BackupInfo},
    cache::Lru,
    callout, catalog,
    friends::{self, FRIENDS, Friend},
    history::{HistoryCache, NEWEST_TTL, Page as HistoryPage, rate_alone},
    live,
    model::{
        ChampSelectSession, ChatMe, Conversation, Entitlements, Game, GameQueue, GameflowSession,
        Lobby, MatchList, RankedStats, ReadyCheck, Summoner,
    },
    profile::{
        self, Admit, ChallengeProfile, ChallengeSummary, ClientBanner, ClientChallenge,
        ClientTitle, Fix, Regalia, SkinChoice, SummonerProfile,
    },
    settings::{
        self, Audience, CalloutRule, General, Language, Mode, PresenceRule, ProfileSettings,
        Scoped, Settings, SettingsStore,
    },
    sgp,
    view::{
        AugmentDetail, Connection, ErrorCode, Event, GameData, HistorySource, IpcError,
        MatchDetail, MatchPage, Me, Notice, NoticeKind, Patch, Phase, PlayerProfile,
        PlayerStanding, PlayerStats, PlayerSummary, Presence, QueueInfo, Snapshot, Update,
    },
};

const PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
const GAMEFLOW: &str = "/lol-gameflow/v1/session";
const READY_CHECK: &str = "/lol-matchmaking/v1/ready-check";
const CHAMP_SELECT: &str = "/lol-champ-select/v1/session";
const SUMMONER: &str = "/lol-summoner/v1/current-summoner";
const RANKED: &str = "/lol-ranked/v1/current-ranked-stats";
/// The chat presence friends see: the status, the message and the rank (`profile`).
const CHAT_ME: &str = "/lol-chat/v1/me";
const LOBBY: &str = "/lol-lobby/v2/lobby";
/// A change to the members alone may come as an event of its own: the lobby is read again then.
const LOBBY_MEMBERS: &str = "/lol-lobby/v2/lobby/members";
const SUBSCRIPTIONS: &[&str] = &[
    PHASE,
    GAMEFLOW,
    READY_CHECK,
    CHAMP_SELECT,
    SUMMONER,
    RANKED,
    CHAT_ME,
    // Social.
    FRIENDS,
    LOBBY,
];

/// Friends' events come in bursts (a client signing in lists them one by one): the view is
/// rebuilt once they settle.
const FRIENDS_SETTLE: Duration = Duration::from_millis(300);
/// The whole friends list is read again this often, in case an event went missing.
const FRIENDS_REFRESH: Duration = Duration::from_secs(60);
/// Sooner until it has been read once: chat can still be signing in when the client connects.
const FRIENDS_RETRY: Duration = Duration::from_secs(5);

/// Between two callout lines: the chat service throttles a burst from one client.
const MESSAGE_GAP: Duration = Duration::from_millis(350);
/// Champ select's chat room opens a moment after champ select itself.
const CHAT_WAIT_ATTEMPTS: u32 = 10;

/// How long a player's stats are reused before they are fetched again.
const PLAYER_TTL: Duration = Duration::from_secs(600);
/// The pages of the shard server's history a player's record reads, newest first, until they hold
/// the form's window (`analysis::window_filled`): twenty games, then forty at a time, a hundred at
/// most. Twenty games are about 2.2 MB and 0.7 s (41 players measured, 2026-10-06).
const RECORD_PAGES: [(u32, u32); 3] = [(0, 20), (20, 40), (60, 40)];
const FAILED_PLAYER_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("the League client is not connected")]
    NotConnected,
    #[error("no player is called {0}")]
    PlayerNotFound(String),
    #[error("{0}")]
    Invalid(String),
    /// Not now: the client is in a state the request must wait out, such as a game.
    #[error("{0}")]
    Busy(String),
    #[error(transparent)]
    Lcu(#[from] lcu::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    /// A host on the internet did not answer as expected.
    #[error("{0}")]
    Remote(String),
}

impl From<CoreError> for IpcError {
    fn from(error: CoreError) -> Self {
        let code = match &error {
            CoreError::NotConnected => ErrorCode::NotConnected,
            CoreError::PlayerNotFound(_) => ErrorCode::NotFound,
            CoreError::Invalid(_) => ErrorCode::Invalid,
            CoreError::Busy(_) => ErrorCode::Busy,
            CoreError::Lcu(error) if error.is_not_found() => ErrorCode::NotFound,
            CoreError::Lcu(error) if error.is_unreachable() => ErrorCode::NotConnected,
            CoreError::Lcu(_) | CoreError::Remote(_) => ErrorCode::Client,
            CoreError::Io(_) => ErrorCode::Internal,
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

/// A game-data file served to the window, as the client sent it.
#[derive(Debug)]
pub struct Asset {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct Service {
    inner: Arc<Inner>,
}

struct Inner {
    settings: SettingsStore,
    state: Mutex<Snapshot>,
    events: broadcast::Sender<Event>,
    runtime: Handle,
    client: RwLock<Option<Client>>,
    players: Mutex<HashMap<String, PlayerEntry>>,
    /// Each player costs three requests; four players at a time keeps the client responsive.
    player_slots: Semaphore,
    /// Pictures the window has drawn, within `caches::ASSETS`.
    assets: Mutex<Lru<String, Arc<Asset>>>,
    /// Augment descriptions, per language, with when they were fetched (`caches`).
    augment_details: tokio::sync::Mutex<AugmentDetails>,
    /// Where snapshots of the game's settings are kept; the shell names it (`set_backup_dir`).
    backups: OnceLock<PathBuf>,
    /// Remembered runes and spells, and the build panel's numbers (`loadout`).
    loadout: loadout::LoadoutState,
    /// History: pages, whole games and Riot ID lookups, for the signed-in account (`history`).
    history: Mutex<HistoryCache>,
    /// Counts the plugins' hellos on the bridge: a restarted interface is back once it changes
    /// (`ux`).
    plugin_hellos: watch::Sender<u64>,
}

/// Augment descriptions per language, with when they were fetched.
type AugmentDetails = HashMap<Language, (Instant, Arc<Vec<AugmentDetail>>)>;

/// One live connection to one client process.
#[derive(Clone)]
struct Client {
    lcu: Lcu,
    credentials: Credentials,
    /// The shard, `NJ100`; empty when chat did not say.
    platform_id: String,
    data: Arc<Mutex<Option<Arc<GameData>>>>,
    live: Arc<Mutex<Live>>,
}

/// What the loop knows about the client between events.
#[derive(Default)]
struct Live {
    phase: Phase,
    me: String,
    champ_select: Option<ChampSelectSession>,
    gameflow: Option<GameflowSession>,
    /// The availability lists of one game; `None` inside while they are being fetched.
    available: Option<(i64, Option<Availability>)>,
    attempts: HashMap<(i64, Step), u8>,
    acting: bool,
    accepting: bool,
    /// The automatic callout went out for this champ select.
    callout_sent: bool,
    /// Bench swaps tried per champion, and one in flight.
    swaps: HashMap<i64, u8>,
    swapping: bool,
    /// What winer keeps in the chat presence: the disguised rank and the remembered status.
    presence: profile::Keeper,
    // Social.
    /// The friends as the client last listed them; `None` until it has once.
    friends: Option<Vec<Friend>>,
    /// A rebuild of the friends view is due once the events settle.
    friends_due: bool,
    /// The lobby while there is one.
    lobby: Option<Lobby>,
    /// The local player's party as the last lobby had it, for champ select's premade marks.
    party: Vec<String>,
    /// What was set up for the champion in hand, and the champ select to remember (`loadout`).
    loadout: loadout::LoadoutLive,
}

impl Live {
    /// Forgets everything tied to one champ select.
    fn reset_champ_select(&mut self) {
        self.attempts.clear();
        self.available = None;
        self.callout_sent = false;
        self.swaps.clear();
    }
}

enum PlayerEntry {
    Loading,
    /// When it was fetched; `None` once the player has finished a game since.
    Ready(Arc<PlayerRecord>, Option<Instant>),
    Failed(String, Instant),
}

/// What a player's summary is worked out from, as the client sent it. The summary is read from it
/// each time it is shown, so it follows the catalog: which queues are played against the computer
/// is known only once the catalog has arrived, which can be after the record.
struct PlayerRecord {
    summoner: Summoner,
    ranked: Option<RankedStats>,
    games: Vec<Game>,
    /// The kind of game the games were fetched to fill a form of (`None`: every kind).
    focus: Option<analysis::Focus>,
    /// The games are all there are: the history ended, or the client's own list stood in.
    complete: bool,
}

impl PlayerRecord {
    /// Whether the record holds what a form read within `focus` needs: fetched for it, holding its
    /// whole window anyway, or all the history there is.
    fn serves(&self, focus: Option<&analysis::Focus>, kinds: &analysis::QueueKinds) -> bool {
        self.complete
            || self.focus.as_ref() == focus
            || analysis::window_filled(&self.games, kinds, focus)
    }
}

impl PlayerEntry {
    /// A record fetched within `max_age`, and no game of the player's has ended since.
    fn fresh(&self, max_age: Duration) -> Option<&Arc<PlayerRecord>> {
        match self {
            Self::Ready(record, Some(at)) if at.elapsed() < max_age => Some(record),
            _ => None,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

/// An event's payload, or `None` when the resource was deleted (`null`) or did not decode.
fn decode<T: DeserializeOwned>(value: serde_json::Value) -> Option<T> {
    if value.is_null() {
        return None;
    }
    serde_json::from_value(value)
        .map_err(|error| debug!(%error, "event payload did not decode"))
        .ok()
}

impl Service {
    /// A service whose background work runs on `runtime`. Nothing happens until [`Self::start`].
    pub fn new(settings_path: impl Into<PathBuf>, runtime: Handle) -> Self {
        let (events, _) = broadcast::channel(256);
        let settings_path = settings_path.into();
        // Remembered setups live in a file of their own beside the settings.
        let loadouts = settings_path.with_file_name("loadouts.json");
        Self {
            inner: Arc::new(Inner {
                settings: SettingsStore::open(settings_path),
                state: Mutex::new(Snapshot::default()),
                events,
                runtime,
                client: RwLock::new(None),
                players: Mutex::new(HashMap::new()),
                player_slots: Semaphore::new(4),
                assets: Mutex::new(Lru::new(caches::ASSETS)),
                augment_details: tokio::sync::Mutex::new(HashMap::new()),
                backups: OnceLock::new(),
                loadout: loadout::LoadoutState::new(loadouts),
                history: Mutex::new(HistoryCache::default()),
                plugin_hellos: watch::Sender::new(0),
            }),
        }
    }

    pub fn start(&self) {
        let service = self.clone();
        self.inner.runtime.spawn(async move { service.run().await });
        // Storage: the caches let go of what has expired.
        self.start_sweeping();
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.inner.state).clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings.get()
    }

    pub fn set_settings(&self, settings: Settings) -> Result<Settings, CoreError> {
        let before = self.settings().profile;
        let saved = self.inner.settings.set(settings)?;
        let _ = self
            .inner
            .events
            .send(Event::Settings(Box::new(saved.clone())));
        // Tier names and the callout's lines come from the settings.
        if let Ok(client) = self.client() {
            self.render(&client);
            self.on_profile_settings(&client, &before, &saved.profile);
        }
        Ok(saved)
    }

    pub fn game_data(&self) -> Option<Arc<GameData>> {
        lock(&self.client().ok()?.data).clone()
    }

    /// The connected client's install directory (`…\LeagueClient`).
    pub fn client_dir(&self) -> Option<PathBuf> {
        self.client().ok()?.credentials.client_dir
    }

    /// `count` of `puuid`'s games from the `begin`-th newest. On a Tencent shard they come from its
    /// match-history server; the client's own history only stands in for the first page, because
    /// it answers every page with the same newest twenty (`sgp`). Pages are kept for a while
    /// (`history`), so going back to a player, paging back and the overview's short list ask for
    /// nothing again.
    pub async fn match_history(
        &self,
        puuid: &str,
        begin: u32,
        count: u32,
    ) -> Result<MatchPage, CoreError> {
        segment(puuid)?;
        let count = count.clamp(1, 50);
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        self.scope_account(&me);
        let kept = lock(&self.inner.history).page(&me, puuid, begin, count, Instant::now());
        let page = match kept {
            Some(page) => page,
            None => {
                let (page, keep) = fetch_history(&client, puuid, &me, begin, count).await?;
                if keep {
                    lock(&self.inner.history).put_page(
                        &me,
                        puuid,
                        begin,
                        page.clone(),
                        Instant::now(),
                    );
                }
                page
            }
        };
        self.same_account(&client, &me)?;
        let data = lock(&client.data).clone().unwrap_or_default();
        Ok(MatchPage {
            puuid: puuid.to_owned(),
            begin,
            games: page
                .entries
                .iter()
                .flatten()
                .filter_map(|game| analysis::match_summary(puuid, game, &data.roles, &data.kinds))
                .collect(),
            has_more: page.more,
            source: page.source,
        })
    }

    /// One game's scoreboard. A game that came with a page of the shard's server is drawn from that
    /// page; any other is asked of the client once (`history`).
    pub async fn match_detail(&self, game_id: i64) -> Result<MatchDetail, CoreError> {
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        self.scope_account(&me);
        let kept = lock(&self.inner.history).game(&me, game_id);
        let game = match kept {
            Some(game) => game,
            None => {
                let game: Arc<Game> = Arc::new(
                    client
                        .lcu
                        .get(&format!("/lol-match-history/v1/games/{game_id}"))
                        .await?,
                );
                lock(&self.inner.history).put_game(&me, game.clone(), Instant::now());
                game
            }
        };
        self.same_account(&client, &me)?;
        let data = lock(&client.data).clone();
        let no_roles = crate::rating::Roles::new();
        Ok(analysis::match_detail(
            &game,
            data.as_deref().map_or(&no_roles, |data| &data.roles),
        ))
    }

    /// Looks a player up by Riot ID, `name#tag`; the same ID again within a few minutes is not
    /// asked for again (`history::FOUND_TTL`).
    pub async fn find_player(&self, riot_id: &str) -> Result<PlayerProfile, CoreError> {
        let riot_id = riot_id.trim();
        let (name, tag) = riot_id.rsplit_once('#').unwrap_or((riot_id, ""));
        if name.trim().is_empty() {
            return Err(CoreError::Invalid(
                "enter a Riot ID such as name#tag".into(),
            ));
        }
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        self.scope_account(&me);
        if let Some(found) = lock(&self.inner.history).found(&me, riot_id, Instant::now()) {
            return Ok(found);
        }
        let lcu = client.lcu.clone();
        let query = query_value(&format!("{}#{}", name.trim(), tag.trim()));
        let summoner: Summoner = match lcu
            .get(&format!("/lol-summoner/v1/summoners?name={query}"))
            .await
        {
            Ok(summoner) => summoner,
            Err(error) if matches!(error.status(), Some(404 | 422)) => {
                return Err(CoreError::PlayerNotFound(riot_id.to_owned()));
            }
            Err(error) => return Err(error.into()),
        };
        if summoner.puuid.is_empty() {
            return Err(CoreError::PlayerNotFound(riot_id.to_owned()));
        }
        let ranked = lcu
            .get_optional::<RankedStats>(&format!("/lol-ranked/v1/ranked-stats/{}", summoner.puuid))
            .await
            .ok()
            .flatten();
        let profile = analysis::profile(&summoner, ranked.as_ref());
        self.same_account(&client, &me)?;
        lock(&self.inner.history).put_found(&me, riot_id, profile.clone(), Instant::now());
        Ok(profile)
    }

    pub async fn player_summary(&self, puuid: &str) -> Result<PlayerSummary, CoreError> {
        let record = self.player_record(puuid).await?;
        Ok(self.summarize(&record, None))
    }

    /// A game-data asset such as a champion icon. Only `/lol-game-data/assets/` is served: the
    /// window must not reach the rest of the LCU through the asset protocol.
    pub async fn asset(&self, path: &str) -> Result<Arc<Asset>, CoreError> {
        if !path.starts_with("/lol-game-data/assets/") || path.contains("..") {
            return Err(CoreError::Invalid(format!("not a game-data asset: {path}")));
        }
        if let Some(asset) = lock(&self.inner.assets).get(path, Instant::now()) {
            return Ok(asset.clone());
        }
        let (content_type, bytes) = self.client()?.lcu.bytes(path).await?;
        let asset = Arc::new(Asset {
            content_type: content_type.unwrap_or_else(|| "application/octet-stream".into()),
            bytes,
        });
        let size = asset.bytes.len();
        lock(&self.inner.assets).insert(path.to_owned(), asset.clone(), size, Instant::now());
        Ok(asset)
    }

    /// `chat`, `away` or `offline`, the states the client offers itself, or `mobile`, the phone
    /// app's, which the desktop client keeps and shows as 在线分组. A request for `dnd` is ignored:
    /// the client marks games on its own (measured on 16.19, `docs/platform-notes.md`). With the
    /// mobile message on, the status message follows the state (`profile::mobile_message_for`).
    /// Returns the presence afterwards.
    pub async fn set_availability(&self, availability: &str) -> Result<Presence, CoreError> {
        if !PresenceRule::AVAILABILITIES.contains(&availability) {
            return Err(CoreError::Invalid(format!(
                "unknown availability {availability}"
            )));
        }
        let lcu = self.client()?.lcu;
        lcu.put(CHAT_ME, &json!({ "availability": availability }))
            .await?;
        // The client answers 201 to a state it then declines; only reading back tells.
        let me: ChatMe = lcu.get(CHAT_ME).await?;
        if me.availability != availability {
            return Err(CoreError::Invalid(format!(
                "the client kept {} instead of {availability}",
                me.availability
            )));
        }
        if !self.settings().profile.presence.mobile_message {
            return Ok(presence_of(me));
        }
        follow_mobile_message(&lcu, me, true).await
    }

    /// Brings the status message in line with the mobile-message switch as it is now: the message
    /// for the mobile state where the client shows that state with none, nothing where winer's no
    /// longer belongs. Returns the presence afterwards.
    pub async fn apply_mobile_message(&self) -> Result<Presence, CoreError> {
        let on = self.settings().profile.presence.mobile_message;
        let lcu = self.client()?.lcu;
        let me: ChatMe = lcu.get(CHAT_ME).await?;
        follow_mobile_message(&lcu, me, on).await
    }

    pub async fn set_status_message(&self, message: &str) -> Result<(), CoreError> {
        self.client()?
            .lcu
            .put(CHAT_ME, &json!({ "statusMessage": message }))
            .await?;
        Ok(())
    }

    pub async fn presence(&self) -> Result<Presence, CoreError> {
        let me: ChatMe = self.client()?.lcu.get(CHAT_ME).await?;
        Ok(presence_of(me))
    }

    /// Sends the champ-select callout now, to `audience` or the configured one. Returns the number
    /// of lines sent.
    pub async fn send_callout(&self, audience: Option<Audience>) -> Result<u32, CoreError> {
        let client = self.client()?;
        let lines = lock(&self.inner.state)
            .champ_select
            .as_ref()
            .map(|view| view.callout.clone())
            .unwrap_or_default();
        if lines.is_empty() {
            return Err(CoreError::Invalid(
                "nobody in champ select has been rated yet".into(),
            ));
        }
        let audience = audience.unwrap_or(self.settings().automation.callout.audience);
        let chat = champ_select_chat(&client.lcu)
            .await?
            .ok_or_else(|| CoreError::Invalid("champ select chat is not open".into()))?;
        say(&client.lcu, &chat, &lines, audience).await?;
        Ok(lines.len() as u32)
    }

    /// What each Hextech ARAM augment does, from ARAM.GG, once per language and half a day
    /// (`caches::AUGMENT_DETAILS_TTL`); nothing while the user has switched it off. A failure is
    /// not cached, so the next view tries again.
    pub async fn augment_details(&self) -> Result<Arc<Vec<AugmentDetail>>, CoreError> {
        let general = self.settings().general;
        if !general.augment_details {
            return Ok(Arc::default());
        }
        // Held across the fetch: two views asking at once make one request.
        let mut cache = self.inner.augment_details.lock().await;
        if let Some((at, details)) = cache.get(&general.language)
            && at.elapsed() < caches::AUGMENT_DETAILS_TTL
        {
            return Ok(details.clone());
        }
        let details = Arc::new(
            augments::fetch(general.language)
                .await
                .map_err(CoreError::Remote)?,
        );
        info!(count = details.len(), "augment descriptions loaded");
        cache.insert(general.language, (Instant::now(), details.clone()));
        Ok(details)
    }

    /// The lines `rule` would send, with the user's own form in each tier.
    /// What `rule` would send under `general` (its language and titles): the window's own
    /// settings, which may be newer than the ones saved.
    pub async fn preview_callout(
        &self,
        rule: &CalloutRule,
        general: &General,
    ) -> Result<Vec<String>, CoreError> {
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        let summary = self.player_summary(&me).await?;
        let data = lock(&client.data).clone();
        let champion = |id: i64| {
            data.as_ref()?
                .champions
                .iter()
                .find(|champion| champion.id == id)
                .map(|champion| champion.short_name.clone())
        };
        Ok(callout::preview(&summary, rule, general, champion))
    }

    /// Takes `champion_id` from the ARAM bench, without waiting out the client's swap cooldown.
    pub async fn bench_swap(&self, champion_id: i64) -> Result<(), CoreError> {
        let client = self.client()?;
        let on_bench = lock(&client.live)
            .champ_select
            .as_ref()
            .is_some_and(|session| {
                session
                    .bench_champions
                    .iter()
                    .any(|bench| bench.champion_id == champion_id)
            });
        if !on_bench {
            return Err(CoreError::Invalid(format!(
                "champion {champion_id} is not on the bench"
            )));
        }
        client
            .lcu
            .post(
                &format!("/lol-champ-select/v1/session/bench/swap/{champion_id}"),
                &json!({}),
            )
            .await?;
        Ok(())
    }

    /// ARAM's reroll: a random champion for the one held, which goes to the bench.
    pub async fn reroll(&self) -> Result<(), CoreError> {
        let client = self.client()?;
        let left = lock(&client.live)
            .champ_select
            .as_ref()
            .map_or(0, |session| session.rerolls_remaining);
        if left <= 0 {
            return Err(CoreError::Invalid("no reroll left".into()));
        }
        client
            .lcu
            .post(
                "/lol-champ-select/v1/session/my-selection/reroll",
                &json!({}),
            )
            .await?;
        Ok(())
    }

    /// Restarts the client's UI process, which reloads injected plugins; the game and the login
    /// session are untouched. The new interface is brought up once it is back, as after the restart
    /// for a new loader ([`Self::restart_client_ui_when_idle`]).
    pub async fn restart_client_ui(&self) -> Result<(), CoreError> {
        let mut hellos = self.plugin_hellos();
        // Only a hello from the new interface counts.
        hellos.mark_unchanged();
        self.client()?
            .lcu
            .post("/riotclient/kill-and-restart-ux", &json!({}))
            .await?;
        self.show_client_ui_when_back(hellos);
        Ok(())
    }

    fn client(&self) -> Result<Client, CoreError> {
        self.inner
            .client
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or(CoreError::NotConnected)
    }

    fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        self.inner.runtime.spawn(task);
    }

    fn notice(&self, kind: NoticeKind) {
        info!(?kind, "notice");
        let _ = self
            .inner
            .events
            .send(Event::Notice(Notice { at: now_ms(), kind }));
    }

    /// Replaces one field of the snapshot and announces it, unless it did not change.
    fn patch(&self, patch: Patch) {
        fn replace<T: PartialEq + Clone>(field: &mut T, value: &T) -> bool {
            let changed = field != value;
            if changed {
                *field = value.clone();
            }
            changed
        }
        let mut state = lock(&self.inner.state);
        let changed = match &patch {
            Patch::Connection(value) => replace(&mut state.connection, value),
            Patch::Me(value) => replace(&mut state.me, value),
            Patch::Phase(value) => replace(&mut state.phase, value),
            Patch::ChampSelect(value) => replace(&mut state.champ_select, value),
            Patch::Game(value) => replace(&mut state.game, value),
            Patch::Friends(value) => replace(&mut state.friends, value),
            Patch::Lobby(value) => replace(&mut state.lobby, value),
        };
        if changed {
            state.rev += 1;
            // Sent under the lock, so revisions reach every subscriber in order.
            let _ = self.inner.events.send(Event::Update(Update {
                rev: state.rev,
                patch,
            }));
        }
    }

    async fn run(self) {
        loop {
            let credentials = self.find_client().await;
            info!(port = credentials.port, pid = ?credentials.pid, "client found");
            self.patch(Patch::Connection(Connection::Connecting {
                port: credentials.port,
            }));
            match self.serve(credentials).await {
                Ok(()) => info!("client closed the event socket"),
                Err(error) => warn!(%error, "client connection ended"),
            }
            *self
                .inner
                .client
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
            for patch in [
                Patch::Me(None),
                Patch::Phase(Phase::None),
                Patch::ChampSelect(None),
                Patch::Game(None),
                Patch::Friends(None),
                Patch::Lobby(None),
            ] {
                self.patch(patch);
            }
            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn find_client(&self) -> Credentials {
        let mut reported = false;
        loop {
            match tokio::task::spawn_blocking(lcu::discover).await {
                Ok(Ok(credentials)) => return credentials,
                Ok(Err(DiscoverError::AccessDenied)) => {
                    if !reported {
                        warn!(
                            "the client runs elevated; its credentials need winer to run as administrator"
                        );
                        reported = true;
                    }
                    self.patch(Patch::Connection(Connection::AccessDenied));
                }
                Ok(Err(error)) => {
                    if let DiscoverError::Io(error) = &error {
                        debug!(%error, "client discovery failed");
                    }
                    self.patch(Patch::Connection(Connection::Searching));
                }
                Err(error) => warn!(%error, "client discovery panicked"),
            }
            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn serve(
        &self,
        credentials: Credentials,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lcu = Lcu::new(&credentials)?;
        let summoner = wait_until_ready(&lcu).await?;
        let mut events = lcu::subscribe(&credentials, SUBSCRIPTIONS).await?;
        let platform_id = lcu
            .get::<ChatMe>(CHAT_ME)
            .await
            .map(|me| me.platform_id)
            .unwrap_or_default();
        let client = Client {
            lcu,
            credentials,
            platform_id: platform_id.clone(),
            data: Arc::default(),
            live: Arc::new(Mutex::new(Live {
                me: summoner.puuid.clone(),
                ..Live::default()
            })),
        };
        *self
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(client.clone());

        self.set_me(&client, summoner).await;
        info!(%platform_id, "connected to the client");
        self.patch(Patch::Connection(Connection::Connected {
            port: client.credentials.port,
            platform_id,
        }));
        self.load_game_data(&client);
        if let Ok(phase) = client.lcu.get::<String>(PHASE).await {
            self.on_phase(&client, Phase::parse(&phase)).await;
        }
        // A client starting up resets the status; winer's disguise is not there yet either.
        self.open_presence_window(&client);
        self.follow_friends(&client);

        while let Some(event) = events.next().await {
            let event = event?;
            let data = if event.kind == EventKind::Delete {
                serde_json::Value::Null
            } else {
                event.data
            };
            match event.uri.as_str() {
                // Social: a friend's presence, or the list itself.
                uri if uri.starts_with(FRIENDS) => self.on_friends_event(&client, uri, data),
                LOBBY => self.on_lobby(&client, decode(data)),
                uri if uri.starts_with(LOBBY_MEMBERS) => self.reload_lobby(&client),
                PHASE => {
                    self.on_phase(&client, data.as_str().map_or(Phase::None, Phase::parse))
                        .await
                }
                GAMEFLOW => self.on_gameflow(&client, decode(data)),
                READY_CHECK => self.on_ready_check(&client, decode(data)),
                CHAMP_SELECT => self.on_champ_select(&client, decode(data)),
                SUMMONER => {
                    if let Some(summoner) = decode::<Summoner>(data) {
                        self.set_me(&client, summoner).await;
                    }
                }
                RANKED => {
                    if let Ok(summoner) = client.lcu.get::<Summoner>(SUMMONER).await {
                        self.set_me(&client, summoner).await;
                    }
                }
                CHAT_ME => self.on_chat_me(&client, decode(data)),
                _ => {}
            }
        }
        Ok(())
    }

    async fn set_me(&self, client: &Client, summoner: Summoner) {
        let ranked = client
            .lcu
            .get_optional::<RankedStats>(&format!("/lol-ranked/v1/ranked-stats/{}", summoner.puuid))
            .await
            .ok()
            .flatten();
        lock(&client.live).me = summoner.puuid.clone();
        self.scope_account(&summoner.puuid);
        self.patch(Patch::Me(Some(Me {
            name: analysis::riot_id(&summoner),
            puuid: summoner.puuid,
            level: summoner.summoner_level,
            icon_id: summoner.profile_icon_id,
            ranked: ranked.as_ref().map(analysis::ranked).unwrap_or_default(),
        })));
    }

    /// Fetches the catalog in the background; a client still starting up may answer with empty
    /// lists, so those are retried a few times.
    fn load_game_data(&self, client: &Client) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            for attempt in 1..=5 {
                let data = catalog::load(&client.lcu).await;
                let complete = !data.champions.is_empty() && !data.queues.is_empty();
                *lock(&client.data) = Some(Arc::new(data));
                let _ = service.inner.events.send(Event::GameData);
                // Friends' modes are named from the catalog's queues.
                service.render_friends(&client);
                // History: so are the games form leaves out, which ratings shown before need.
                service.render(&client);
                if complete {
                    return;
                }
                sleep(Duration::from_secs(3 * attempt)).await;
            }
        });
    }

    async fn on_phase(&self, client: &Client, phase: Phase) {
        let previous = std::mem::replace(&mut lock(&client.live).phase, phase);
        if game_over(previous, phase) {
            self.forget_newest(client);
        }
        self.patch(Patch::Phase(phase));
        if profile::after_a_game(previous, phase) {
            self.open_presence_window(client);
        }
        self.loadout_phase(client, phase);
        if phase != Phase::ChampSelect {
            self.on_champ_select(client, None);
        }
        match phase {
            // The session may have started before the subscription did.
            Phase::ChampSelect => {
                if let Ok(Some(session)) = client
                    .lcu
                    .get_optional::<ChampSelectSession>(CHAMP_SELECT)
                    .await
                {
                    self.on_champ_select(client, Some(session));
                }
            }
            Phase::GameStart | Phase::InProgress | Phase::Reconnect => {
                if let Ok(session) = client.lcu.get_optional::<GameflowSession>(GAMEFLOW).await {
                    self.on_gameflow(client, session);
                }
            }
            Phase::EndOfGame => self.play_again(client),
            Phase::None | Phase::Lobby | Phase::Matchmaking | Phase::ReadyCheck => {
                lock(&client.live).gameflow = None;
                self.patch(Patch::Game(None));
            }
            _ => {}
        }
        // The lobby shows only in some phases; it may also have formed before the subscription.
        if live::shows_lobby(phase) {
            if let Ok(lobby) = client.lcu.get_optional::<Lobby>(LOBBY).await {
                self.on_lobby(client, lobby);
            }
        } else {
            self.render_lobby(client);
        }
    }

    fn on_ready_check(&self, client: &Client, check: Option<ReadyCheck>) {
        let rule = self.settings().automation.accept;
        if !rule.enabled || !check.as_ref().is_some_and(automation::should_accept) {
            return;
        }
        if std::mem::replace(&mut lock(&client.live).accepting, true) {
            return;
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            sleep(Duration::from_millis(u64::from(rule.delay_ms))).await;
            // Re-read rather than trust the old event: the user may have answered meanwhile.
            let still = client
                .lcu
                .get_optional::<ReadyCheck>(READY_CHECK)
                .await
                .ok()
                .flatten();
            // What was found: the gameflow session names the queue being searched.
            let mode = client
                .lcu
                .get_optional::<GameflowSession>(GAMEFLOW)
                .await
                .ok()
                .flatten()
                .and_then(|session| gameflow_mode(&session.game_data.queue));
            let automation = service.settings().automation;
            if !automation.scopes.covers(Scoped::Accept, mode) {
                debug!(?mode, "match found in a mode auto-accept leaves alone");
            } else if automation.accept.enabled
                && still.as_ref().is_some_and(automation::should_accept)
            {
                match client
                    .lcu
                    .post("/lol-matchmaking/v1/ready-check/accept", &json!({}))
                    .await
                {
                    Ok(()) => service.notice(NoticeKind::Accepted),
                    Err(error) => service.notice(NoticeKind::Failed {
                        action: "accept".into(),
                        message: error.to_string(),
                    }),
                }
            }
            lock(&client.live).accepting = false;
        });
    }

    fn on_champ_select(&self, client: &Client, session: Option<ChampSelectSession>) {
        let Some(session) = session else {
            let mut live = lock(&client.live);
            if live.champ_select.take().is_some() {
                live.reset_champ_select();
            }
            drop(live);
            self.patch(Patch::ChampSelect(None));
            return;
        };
        {
            let mut live = lock(&client.live);
            if live
                .champ_select
                .as_ref()
                .is_some_and(|previous| previous.game_id != session.game_id)
            {
                live.reset_champ_select();
            }
            live.champ_select = Some(session.clone());
        }
        let focus = queue_focus(lock(&client.data).as_deref(), session.queue_id, "");
        for puuid in live::champ_select_puuids(&session) {
            self.ensure_player(client, puuid, focus.clone());
        }
        self.render(client);
        self.automate(client);
        self.loadout_champ_select(client, &session);
    }

    fn on_gameflow(&self, client: &Client, session: Option<GameflowSession>) {
        let mut live = lock(&client.live);
        if !(live.phase.in_game()
            || matches!(
                live.phase,
                Phase::WaitingForStats | Phase::PreEndOfGame | Phase::EndOfGame
            ))
        {
            return;
        }
        let session = session.filter(|session| {
            !session.game_data.team_one.is_empty() || !session.game_data.team_two.is_empty()
        });
        if live.gameflow == session {
            return;
        }
        live.gameflow = session.clone();
        drop(live);
        let focus = session.as_ref().and_then(|session| {
            let queue = &session.game_data.queue;
            queue_focus(lock(&client.data).as_deref(), queue.id, &queue.game_mode)
        });
        for puuid in session.iter().flat_map(live::game_puuids) {
            self.ensure_player(client, puuid, focus.clone());
        }
        self.render(client);
    }

    /// Rebuilds the champ select and game views from the last sessions and the stats known now,
    /// and sends the automatic callout once its lines are final.
    fn render(&self, client: &Client) {
        let (champ_select, gameflow, me, party) = {
            let live = lock(&client.live);
            (
                live.champ_select.clone(),
                live.gameflow.clone(),
                live.me.clone(),
                live.party.clone(),
            )
        };
        let settings = self.settings();
        let (rule, language) = (&settings.automation.callout, settings.general.language);
        let ranking = callout::ranking(rule, &settings.general);
        // Each player's form is read within the kind of game being played.
        let stats = |focus: Option<analysis::Focus>| {
            move |puuid: &str| self.player_stats(puuid, focus.as_ref())
        };
        let mut mode = None;
        let view = champ_select.map(|session| {
            let data = lock(&client.data).clone();
            // The catalog says what kind of game the queue is.
            let queue = queue_info(data.as_deref(), session.queue_id);
            mode = queue.map(QueueInfo::mode);
            let game_mode = queue.map_or("", |queue| queue.game_mode.as_str());
            let focus = queue_focus(data.as_deref(), session.queue_id, "");
            let mut view = live::champ_select_view(&session, stats(focus), &ranking, game_mode);
            live::mark_party(&mut view.my_team, &party);
            let champion = |id: i64| {
                data.as_ref()?
                    .champions
                    .iter()
                    .find(|champion| champion.id == id)
                    .map(|champion| champion.short_name.clone())
            };
            view.callout = callout::lines(&view, rule, language, champion);
            view
        });
        let auto = rule.auto && settings.automation.scopes.covers(Scoped::Callout, mode);
        let lines = view
            .as_ref()
            .filter(|view| auto && callout::ready(view))
            .map(|view| view.callout.clone())
            .filter(|lines| !lines.is_empty());
        self.patch(Patch::ChampSelect(view));
        if let Some(lines) = lines
            && !std::mem::replace(&mut lock(&client.live).callout_sent, true)
        {
            self.call_out(client, lines, rule.audience);
        }
        if let Some(session) = gameflow {
            let queue = &session.game_data.queue;
            let focus = queue_focus(lock(&client.data).as_deref(), queue.id, &queue.game_mode);
            let mut view = live::game_view(&session, &me, stats(focus), &ranking);
            // Callout: the lines the shortcut types into the game's chat, both teams by champion.
            if let Some(view) = view.as_mut() {
                let data = lock(&client.data).clone();
                let champion = |id: i64| {
                    data.as_ref()?
                        .champions
                        .iter()
                        .find(|champion| champion.id == id)
                        .map(|champion| champion.short_name.clone())
                };
                view.callout = callout::game_lines(view, rule, language, champion);
                // The team's own lines, by champion.
                view.ally_callout = callout::ally_lines(view, rule, language, champion);
            }
            self.patch(Patch::Game(view));
        }
        // The lobby's members wait for the same stats.
        self.render_lobby(client);
    }

    fn call_out(&self, client: &Client, lines: Vec<String>, audience: Audience) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let mut chat = None;
            for _ in 0..CHAT_WAIT_ATTEMPTS {
                match champ_select_chat(&client.lcu).await {
                    Ok(Some(found)) => {
                        chat = Some(found);
                        break;
                    }
                    Ok(None) => sleep(Duration::from_secs(1)).await,
                    Err(error) => {
                        debug!(%error, "conversations unavailable");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            let result = match &chat {
                Some(chat) => say(&client.lcu, chat, &lines, audience).await,
                None => Err(CoreError::Invalid("champ select chat never opened".into())),
            };
            match result {
                Ok(()) => service.notice(NoticeKind::CalledOut {
                    lines: lines.len() as u32,
                }),
                Err(error) => service.notice(NoticeKind::Failed {
                    action: "callout".into(),
                    message: error.to_string(),
                }),
            }
        });
    }

    /// A player's stats as known now, the form read within `focus` ([`analysis::recent_form`]).
    fn player_stats(&self, puuid: &str, focus: Option<&analysis::Focus>) -> PlayerStats {
        let record = match lock(&self.inner.players).get(puuid) {
            Some(PlayerEntry::Ready(record, _)) => record.clone(),
            Some(PlayerEntry::Failed(message, _)) => {
                return PlayerStats::Failed {
                    message: message.clone(),
                };
            }
            Some(PlayerEntry::Loading) | None => return PlayerStats::Loading,
        };
        PlayerStats::Ready(Box::new(self.summarize(&record, focus)))
    }

    /// Fetches `puuid`'s record unless a fresh one holds what a form read within `focus` needs.
    fn ensure_player(&self, client: &Client, puuid: String, focus: Option<analysis::Focus>) {
        let me = lock(&client.live).me.clone();
        self.scope_account(&me);
        let shown = caches::players_shown(client);
        let kinds = catalog_kinds(client);
        {
            let mut players = lock(&self.inner.players);
            let fresh = match players.get(&puuid) {
                Some(PlayerEntry::Loading) => true,
                Some(entry @ PlayerEntry::Ready(record, _)) => {
                    entry.fresh(PLAYER_TTL).is_some() && record.serves(focus.as_ref(), &kinds)
                }
                Some(PlayerEntry::Failed(_, at)) => at.elapsed() < FAILED_PLAYER_TTL,
                None => false,
            };
            if fresh {
                return;
            }
            players.insert(puuid.clone(), PlayerEntry::Loading);
            caches::trim_players(&mut players, &shown, Instant::now());
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let entry = {
                let _slot = service.inner.player_slots.acquire().await;
                match load_record(&client, &puuid, &me, focus).await {
                    Ok(record) => PlayerEntry::Ready(Arc::new(record), Some(Instant::now())),
                    Err(error) => {
                        debug!(%error, "player stats unavailable");
                        PlayerEntry::Failed(error.to_string(), Instant::now())
                    }
                }
            };
            let current = service.same_account(&client, &me).is_ok();
            let shown = caches::players_shown(&client);
            {
                let mut players = lock(&service.inner.players);
                if current {
                    players.insert(puuid, entry);
                    caches::trim_players(&mut players, &shown, Instant::now());
                } else if matches!(players.get(&puuid), Some(PlayerEntry::Loading)) {
                    // Fetched for an account that has signed out since: the next one asks again.
                    players.remove(&puuid);
                }
            }
            service.render(&client);
        });
    }

    fn automate(&self, client: &Client) {
        let mut rules = self.settings().automation;
        let queue_id = lock(&client.live)
            .champ_select
            .as_ref()
            .map(|session| session.queue_id);
        let mode = queue_id.and_then(|id| queue_mode(client, id));
        // A rule switched on still acts only in the kinds of game it is scoped to.
        let scopes = &rules.scopes;
        rules.pick.enabled &= scopes.covers(Scoped::Pick, mode);
        rules.ban.enabled &= scopes.covers(Scoped::Ban, mode);
        rules.bench.enabled &= scopes.covers(Scoped::Bench, mode);
        if rules.bench.enabled {
            self.grab_from_bench(client, &rules.bench.champions);
        }
        if !rules.pick.enabled && !rules.ban.enabled {
            return;
        }
        let mut live = lock(&client.live);
        let Some(session) = live.champ_select.clone() else {
            return;
        };
        let available = match &live.available {
            Some((game, Some(available))) if *game == session.game_id => available.clone(),
            Some((game, None)) if *game == session.game_id => return,
            _ => {
                // Owned and bannable champions first: acting without them risks a refused pick.
                live.available = Some((session.game_id, None));
                drop(live);
                self.fetch_availability(client, session.game_id);
                return;
            }
        };
        if live.acting {
            return;
        }
        let Some(action) = automation::decide(&session, &rules, &available) else {
            return;
        };
        let attempts = live
            .attempts
            .entry((action.action_id, action.step))
            .or_default();
        if *attempts >= 2 {
            return;
        }
        *attempts += 1;
        live.acting = true;
        drop(live);

        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let result = execute(&client.lcu, action).await;
            {
                let mut live = lock(&client.live);
                live.acting = false;
                // Done for good: an event sent before the change landed must not repeat it.
                if result.is_ok() {
                    live.attempts
                        .insert((action.action_id, action.step), u8::MAX);
                }
            }
            match result {
                Ok(()) => service.notice(match action.step {
                    Step::Declare => NoticeKind::Declared {
                        champion_id: action.champion_id,
                    },
                    Step::Hover => NoticeKind::Picked {
                        champion_id: action.champion_id,
                        locked: false,
                    },
                    Step::Lock => NoticeKind::Picked {
                        champion_id: action.champion_id,
                        locked: true,
                    },
                    Step::Ban => NoticeKind::Banned {
                        champion_id: action.champion_id,
                    },
                }),
                Err(error) => {
                    let action = if action.step == Step::Ban {
                        "ban"
                    } else {
                        "pick"
                    };
                    service.notice(NoticeKind::Failed {
                        action: action.into(),
                        message: error.to_string(),
                    });
                }
            }
        });
    }

    /// Takes a better wishlist champion off the ARAM bench, twice at most per champion.
    fn grab_from_bench(&self, client: &Client, wishlist: &[i64]) {
        let mut live = lock(&client.live);
        let Some(champion) = live
            .champ_select
            .as_ref()
            .and_then(|session| automation::bench_pick(session, wishlist))
        else {
            return;
        };
        if live.swapping {
            return;
        }
        let tries = live.swaps.entry(champion).or_default();
        if *tries >= 2 {
            return;
        }
        *tries += 1;
        live.swapping = true;
        drop(live);

        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let result = client
                .lcu
                .post(
                    &format!("/lol-champ-select/v1/session/bench/swap/{champion}"),
                    &json!({}),
                )
                .await;
            {
                let mut live = lock(&client.live);
                live.swapping = false;
                if result.is_ok() {
                    live.swaps.insert(champion, u8::MAX);
                }
            }
            match result {
                Ok(()) => service.notice(NoticeKind::Swapped {
                    champion_id: champion,
                }),
                Err(error) => service.notice(NoticeKind::Failed {
                    action: "swap".into(),
                    message: error.to_string(),
                }),
            }
        });
    }

    fn fetch_availability(&self, client: &Client, game_id: i64) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let ids = |path: &'static str| {
                let lcu = client.lcu.clone();
                async move {
                    lcu.get::<Vec<i64>>(path)
                        .await
                        .ok()
                        .map(|ids| ids.into_iter().collect())
                }
            };
            let (pickable, bannable) = tokio::join!(
                ids("/lol-champ-select/v1/pickable-champion-ids"),
                ids("/lol-champ-select/v1/bannable-champion-ids")
            );
            if let Some((game, slot)) = &mut lock(&client.live).available
                && *game == game_id
            {
                *slot = Some(Availability { pickable, bannable });
            }
            service.automate(&client);
        });
    }

    fn play_again(&self, client: &Client) {
        if !self.wants_play_again(client) {
            return;
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            // Give the post-game screen a moment; the user may be reading it.
            sleep(Duration::from_secs(3)).await;
            // Asked again: meanwhile the user may have left, or narrowed the rule.
            if !service.wants_play_again(&client) {
                return;
            }
            match client
                .lcu
                .post("/lol-lobby/v2/play-again", &json!({}))
                .await
            {
                Ok(()) => service.notice(NoticeKind::PlayedAgain),
                Err(error) => service.notice(NoticeKind::Failed {
                    action: "playAgain".into(),
                    message: error.to_string(),
                }),
            }
        });
    }
}

impl Service {
    /// Whether to go back to the lobby now: still on the post-game screen, the rule on as the
    /// settings say at this moment, and the game one of the kinds it is scoped to.
    fn wants_play_again(&self, client: &Client) -> bool {
        let live = lock(&client.live);
        let mode = live
            .gameflow
            .as_ref()
            .and_then(|session| gameflow_mode(&session.game_data.queue));
        live.phase == Phase::EndOfGame && self.settings().automation.plays_again(mode)
    }
}

// ---- Profile: background, challenges, the presence winer keeps, game-settings backups --------

const PROFILE: &str = "/lol-summoner/v1/current-summoner/summoner-profile";
const CHALLENGES: &str = "/lol-challenges/v1/challenges/local-player";
const CHALLENGE_SUMMARY: &str = "/lol-challenges/v1/summary-player-data/local-player";
const TITLES: &str = "/lol-challenges/v2/titles/local-player";
/// The tokens, the title and the banner (`bannerAccent`) the profile shows.
const PREFERENCES: &str = "/lol-challenges/v1/update-player-preferences";
/// Every banner there is, owned or not.
const BANNERS: &str = "/lol-regalia/v3/inventory/REGALIA_BANNER";
/// How the banner is drawn: plain, or in the tier of last season's rank.
const REGALIA: &str = "/lol-regalia/v2/current-summoner/regalia";
/// A change to the profile is the server's to accept: winer reads it back this many times, this
/// far apart, before taking the answer as final.
const READ_BACK_ATTEMPTS: u32 = 4;
const READ_BACK_GAP: Duration = Duration::from_millis(400);
/// The client sends a burst of presence changes when it resets one; a correction waits it out.
const PRESENCE_SETTLE: Duration = Duration::from_millis(1500);

impl Service {
    /// Every skin of every champion, owned or not, for the background picker.
    pub async fn skins(&self) -> Result<Vec<SkinChoice>, CoreError> {
        let lcu = self.client()?.lcu;
        let summoner: Summoner = lcu.get(SUMMONER).await?;
        let listed = lcu
            .get(&format!(
                "/lol-champions/v1/inventories/{}/skins-minimal",
                summoner.summoner_id
            ))
            .await?;
        Ok(profile::skins(listed))
    }

    /// The skin behind the profile; `None` while the player has chosen none.
    pub async fn profile_background(&self) -> Result<Option<i64>, CoreError> {
        let profile: SummonerProfile = self.client()?.lcu.get(PROFILE).await?;
        Ok(profile.background())
    }

    /// Sets the profile background and returns what the client reports afterwards, which is still
    /// the old one when the server refused the skin (one the player does not own, say).
    pub async fn set_profile_background(&self, skin_id: i64) -> Result<Option<i64>, CoreError> {
        if skin_id <= 0 {
            return Err(CoreError::Invalid(format!("{skin_id} is not a skin")));
        }
        let lcu = self.client()?.lcu;
        lcu.post(PROFILE, &profile::background_request(skin_id))
            .await?;
        let mut shown = None;
        for attempt in 0..READ_BACK_ATTEMPTS {
            if attempt > 0 {
                sleep(READ_BACK_GAP).await;
            }
            shown = lcu.get::<SummonerProfile>(PROFILE).await?.background();
            if shown == Some(skin_id) {
                break;
            }
        }
        Ok(shown)
    }

    /// The tokens, title and banner the profile shows, and every one it could.
    pub async fn challenge_profile(&self) -> Result<ChallengeProfile, CoreError> {
        let lcu = self.client()?.lcu;
        let choices = challenge_choices(&lcu).await?;
        let (summary, regalia) = tokio::join!(
            lcu.get::<ChallengeSummary>(CHALLENGE_SUMMARY),
            regalia(&lcu)
        );
        Ok(choices.profile(&summary?, &regalia))
    }

    /// Shows `tokens` in the profile's slots, left to right, `title` when given and `banner` when
    /// given (empty for the default), then returns what the client reports, which is the request
    /// only where the server took it. The banner goes out as the client's own customizer sends it:
    /// in the preferences, and in the regalia where it changes how the banner is drawn.
    pub async fn set_challenge_profile(
        &self,
        tokens: Vec<i64>,
        title: Option<i64>,
        banner: Option<String>,
    ) -> Result<ChallengeProfile, CoreError> {
        profile::check_tokens(&tokens).map_err(CoreError::Invalid)?;
        if title.is_some_and(|id| id <= 0) {
            return Err(CoreError::Invalid("not a title".into()));
        }
        if let Some(banner) = &banner {
            profile::check_banner(banner).map_err(CoreError::Invalid)?;
        }
        let lcu = self.client()?.lcu;
        lcu.post(
            PREFERENCES,
            &profile::preferences_request(&tokens, title, banner.as_deref()),
        )
        .await?;
        if let Some(banner) = &banner
            && let Some(body) = profile::regalia_request(&regalia(&lcu).await, banner)
        {
            lcu.put(REGALIA, &body).await?;
        }
        let choices = challenge_choices(&lcu).await?;
        let mut shown = None;
        for attempt in 0..READ_BACK_ATTEMPTS {
            if attempt > 0 {
                sleep(READ_BACK_GAP).await;
            }
            let (summary, regalia) = tokio::join!(
                lcu.get::<ChallengeSummary>(CHALLENGE_SUMMARY),
                regalia(&lcu)
            );
            let profile = choices.profile(&summary?, &regalia);
            let done = profile::shows(&profile, &tokens, title, banner.as_deref());
            shown = Some(profile);
            if done {
                break;
            }
        }
        Ok(shown.expect("read at least once"))
    }

    /// Acts on a change to what winer keeps in the chat presence. A disguise switched off comes
    /// off at once, anything still on is checked again, and the client's refusals are counted
    /// afresh.
    fn on_profile_settings(
        &self,
        client: &Client,
        before: &ProfileSettings,
        after: &ProfileSettings,
    ) {
        if before == after {
            return;
        }
        lock(&client.live).presence.reset();
        let (was, now) = (&before.rank_disguise, &after.rank_disguise);
        if was.enabled && !now.enabled {
            let (service, client, was) = (self.clone(), client.clone(), was.clone());
            self.spawn(async move {
                if let Err(error) = service.undisguise(&client, &was).await {
                    service.notice(NoticeKind::Failed {
                        action: "rankDisguise".into(),
                        message: error.to_string(),
                    });
                }
            });
        } else if now.enabled || after.presence.remember {
            // A changed rule is tried afresh, also after winer gave up on the old one.
            self.keep_presence(client);
        }
    }

    /// Takes the disguise `was` off the presence: the client's own rank back where winer saw it,
    /// the keys taken out where it never did. A client that merges `lol` keeps a key taken out,
    /// so where the disguise is still there afterwards the keys are emptied instead.
    async fn undisguise(
        &self,
        client: &Client,
        was: &settings::RankDisguise,
    ) -> Result<(), CoreError> {
        let me: ChatMe = client.lcu.get(CHAT_ME).await?;
        let (ours, real) = {
            let live = lock(&client.live);
            (live.presence.ours(was), live.presence.real().cloned())
        };
        let Some(body) = profile::undisguise(&me.lol, &ours, real.as_ref()) else {
            return Ok(());
        };
        client.lcu.put(CHAT_ME, &body).await?;
        let after: ChatMe = client.lcu.get(CHAT_ME).await?;
        if profile::undisguise(&after.lol, &ours, None).is_some() {
            debug!("the client kept the disguised rank keys; emptying them");
            client
                .lcu
                .put(CHAT_ME, &profile::blank_rank(&after.lol))
                .await?;
        }
        Ok(())
    }

    /// The client starts up and comes out of a game with its own status: for a while, put the
    /// remembered one back, and the disguise whenever it is missing.
    fn open_presence_window(&self, client: &Client) {
        lock(&client.live).presence.open_window(Instant::now());
        let profile = self.settings().profile;
        if profile.presence.remember || profile.rank_disguise.enabled {
            self.keep_presence(client);
        }
    }

    /// The chat presence changed: learns the client's own rank from it, and corrects it if the
    /// client reset what winer keeps there.
    fn on_chat_me(&self, client: &Client, me: Option<ChatMe>) {
        let Some(me) = me else {
            return;
        };
        let settings = self.settings().profile;
        let disguise = settings
            .rank_disguise
            .enabled
            .then_some(&settings.rank_disguise);
        let needed = {
            let mut live = lock(&client.live);
            let phase = live.phase;
            live.presence.observe(&me.lol, disguise);
            live.presence
                .plan(&me, &settings, phase, Instant::now())
                .is_some()
        };
        if needed {
            self.keep_presence(client);
        }
    }

    /// Corrects the presence once the client has settled, from a fresh reading, one correction
    /// at a time.
    fn keep_presence(&self, client: &Client) {
        if std::mem::replace(&mut lock(&client.live).presence.scheduled, true) {
            return;
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            sleep(PRESENCE_SETTLE).await;
            lock(&client.live).presence.scheduled = false;
            if let Err(error) = service.correct_presence(&client).await {
                debug!(%error, "chat presence not corrected");
            }
        });
    }

    async fn correct_presence(&self, client: &Client) -> Result<(), CoreError> {
        let me: ChatMe = client.lcu.get(CHAT_ME).await?;
        let settings = self.settings().profile;
        let disguise = settings
            .rank_disguise
            .enabled
            .then_some(&settings.rank_disguise);
        let outcome: Option<(Admit, Fix)> = {
            let mut live = lock(&client.live);
            let phase = live.phase;
            let keeper = &mut live.presence;
            keeper.observe(&me.lol, disguise);
            keeper
                .plan(&me, &settings, phase, Instant::now())
                .map(|fix| {
                    let admit = keeper.admit(Instant::now());
                    if admit == Admit::Send {
                        keeper.sent(&fix);
                    }
                    (admit, fix)
                })
        };
        match outcome {
            Some((Admit::Send, fix)) => {
                client.lcu.put(CHAT_ME, &fix.body).await?;
                if let Some(availability) = fix.availability {
                    self.notice(NoticeKind::PresenceRestored { availability });
                }
            }
            Some((Admit::GiveUp, _)) => {
                warn!("the client keeps undoing the chat presence winer keeps; stopped");
                self.notice(NoticeKind::PresenceRefused);
            }
            Some((Admit::Stopped, _)) | None => {}
        }
        Ok(())
    }

    /// Names the folder snapshots of the game's settings are kept in. Once; later calls are ignored.
    pub fn set_backup_dir(&self, dir: impl Into<PathBuf>) {
        let _ = self.inner.backups.set(dir.into());
    }

    fn backup_dir(&self) -> Result<&Path, CoreError> {
        self.inner
            .backups
            .get()
            .map(PathBuf::as_path)
            .ok_or_else(|| CoreError::Invalid("there is no folder for settings backups".into()))
    }

    /// The snapshots kept, the newest first. Reads files: call it off the async runtime.
    pub fn game_settings_backups(&self) -> Result<Vec<BackupInfo>, CoreError> {
        list_backups(self.backup_dir()?)
    }

    /// Reads both halves of the game's settings into a new snapshot; the oldest beyond
    /// [`backup::KEEP`] go.
    pub async fn back_up_game_settings(&self) -> Result<BackupInfo, CoreError> {
        let dir = self.backup_dir()?.to_owned();
        let lcu = self.client()?.lcu;
        game_settings_ready(&lcu).await?;
        let (general, hotkeys) = tokio::join!(
            lcu.get::<serde_json::Value>(backup::GAME_SETTINGS),
            lcu.get::<serde_json::Value>(backup::INPUT_SETTINGS),
        );
        let (general, hotkeys) = (general?, hotkeys?);
        let read =
            |document: &serde_json::Value| document.as_object().is_some_and(|map| !map.is_empty());
        if !read(&general) || !read(&hotkeys) {
            return Err(CoreError::Invalid(
                "the client has not loaded the game's settings yet".into(),
            ));
        }
        let file = BackupFile::new(now_ms(), general, hotkeys);
        off_runtime(move || store_backup(&dir, now_ms(), &file)).await
    }

    /// Puts `channels` of snapshot `id` back and has the client write them to the game's files.
    /// Only outside a game, from the lobby or the home screen: the game reads its settings as it
    /// starts, and its own settings screen would write over these.
    pub async fn restore_game_settings(
        &self,
        id: i64,
        channels: Vec<BackupChannel>,
    ) -> Result<(), CoreError> {
        let path = self.backup_dir()?.join(backup::file_name(id));
        let lcu = self.client()?.lcu;
        let phase: String = lcu.get(PHASE).await?;
        if !matches!(Phase::parse(&phase), Phase::None | Phase::Lobby) {
            return Err(CoreError::Busy(
                "game settings are restored only outside a game: from the lobby or the home screen"
                    .into(),
            ));
        }
        let text = off_runtime(move || match fs::read_to_string(&path) {
            Ok(text) => Ok(text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(CoreError::Invalid(format!("there is no backup {id}")))
            }
            Err(error) => Err(error.into()),
        })
        .await?;
        let file = backup::parse(&text).map_err(CoreError::Invalid)?;
        game_settings_ready(&lcu).await?;
        for (path, document) in file.patches(&channels).map_err(CoreError::Invalid)? {
            lcu.patch(path, document).await?;
        }
        lcu.post(backup::SAVE, &json!({})).await?;
        Ok(())
    }

    /// Removes snapshot `id`; one already gone is no error. Writes files: call it off the runtime.
    pub fn delete_game_settings_backup(&self, id: i64) -> Result<(), CoreError> {
        let path = self.backup_dir()?.join(backup::file_name(id));
        match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
    }

    /// Keeps a snapshot brought in from a file as the newest one. Writes files: call it off the
    /// runtime.
    pub fn import_game_settings_backup(&self, text: &str) -> Result<BackupInfo, CoreError> {
        let dir = self.backup_dir()?;
        let file = backup::parse(text).map_err(CoreError::Invalid)?;
        store_backup(dir, now_ms(), &file)
    }

    /// The file snapshot `id` is kept in, for the shell to show in the file manager.
    pub fn game_settings_backup_path(&self, id: i64) -> Result<PathBuf, CoreError> {
        let path = self.backup_dir()?.join(backup::file_name(id));
        if path.is_file() {
            Ok(path)
        } else {
            Err(CoreError::Invalid(format!("there is no backup {id}")))
        }
    }
}

fn presence_of(me: ChatMe) -> Presence {
    Presence {
        availability: me.availability,
        status_message: me.status_message,
    }
}

/// Puts up or takes down the mobile state's message on `me`, the presence just read, as
/// `profile::mobile_message_for` says under `on`; returns the presence the client reports.
async fn follow_mobile_message(lcu: &Lcu, me: ChatMe, on: bool) -> Result<Presence, CoreError> {
    let Some(message) = profile::mobile_message_for(&me.availability, &me.status_message, on)
    else {
        return Ok(presence_of(me));
    };
    lcu.put(CHAT_ME, &json!({ "statusMessage": message }))
        .await?;
    Ok(presence_of(lcu.get(CHAT_ME).await?))
}

/// What the profile can show of challenges: the challenges with their levels, the titles and the
/// banners. Titles and banners are left out where the client does not list them.
async fn challenge_choices(lcu: &Lcu) -> Result<ChallengeChoices, CoreError> {
    let (challenges, titles, banners) = tokio::join!(
        lcu.get::<HashMap<String, ClientChallenge>>(CHALLENGES),
        lcu.get::<Vec<ClientTitle>>(TITLES),
        lcu.get::<HashMap<String, ClientBanner>>(BANNERS),
    );
    let titles = titles.unwrap_or_else(|error| {
        debug!(%error, "titles unavailable");
        Vec::new()
    });
    let banners = banners.unwrap_or_else(|error| {
        debug!(%error, "banners unavailable");
        HashMap::new()
    });
    Ok(ChallengeChoices {
        challenges: challenges?,
        titles,
        banners,
    })
}

struct ChallengeChoices {
    challenges: HashMap<String, ClientChallenge>,
    titles: Vec<ClientTitle>,
    banners: HashMap<String, ClientBanner>,
}

impl ChallengeChoices {
    fn profile(&self, summary: &ChallengeSummary, regalia: &Regalia) -> ChallengeProfile {
        profile::challenge_profile(
            &self.challenges,
            summary,
            &self.titles,
            &self.banners,
            regalia,
        )
    }
}

/// The regalia, or nothing known of it where the client does not say: the banner is then read
/// from the challenge summary alone.
async fn regalia(lcu: &Lcu) -> Regalia {
    lcu.get(REGALIA).await.unwrap_or_else(|error| {
        debug!(%error, "regalia unavailable");
        Regalia::default()
    })
}

/// Refuses while the client has not read the game's settings yet, as it may not have just after it
/// started. A client without the endpoint is taken as ready. Not `Busy`: the window words that one
/// as "not during a game".
async fn game_settings_ready(lcu: &Lcu) -> Result<(), CoreError> {
    match lcu.get::<bool>(backup::READY).await {
        Ok(false) => Err(CoreError::Invalid(
            "the client has not loaded the game's settings yet; try again in a moment".into(),
        )),
        Ok(true) => Ok(()),
        Err(error) => {
            debug!(%error, "no game-settings readiness; going ahead");
            Ok(())
        }
    }
}

/// File work, on the blocking pool.
async fn off_runtime<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CoreError> + Send + 'static,
) -> Result<T, CoreError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| CoreError::Io(io::Error::other(error)))?
}

/// The snapshots in `dir`, by file name alone.
fn backup_ids(dir: &Path) -> Result<Vec<i64>, CoreError> {
    match fs::read_dir(dir) {
        Ok(entries) => Ok(entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str().and_then(backup::id_of))
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// The snapshots in `dir`, the newest first. A file that is not one is passed over.
fn list_backups(dir: &Path) -> Result<Vec<BackupInfo>, CoreError> {
    let mut found = Vec::new();
    for id in backup_ids(dir)? {
        let path = dir.join(backup::file_name(id));
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        match backup::parse(&text) {
            Ok(file) => found.push(file.info(id, text.len() as u64)),
            Err(error) => debug!(%error, path = %path.display(), "not a settings backup"),
        }
    }
    found.sort_by_key(|info| std::cmp::Reverse(info.id));
    Ok(found)
}

/// Writes `file` as the newest snapshot in `dir` and removes the oldest beyond [`backup::KEEP`].
fn store_backup(dir: &Path, now: i64, file: &BackupFile) -> Result<BackupInfo, CoreError> {
    let mut ids = backup_ids(dir)?;
    let id = backup::fresh_id(now, &ids);
    let text = serde_json::to_string_pretty(file).expect("a backup serializes");
    settings::write_atomic(&dir.join(backup::file_name(id)), text.as_bytes())?;
    ids.push(id);
    for old in backup::surplus(ids) {
        if let Err(error) = fs::remove_file(dir.join(backup::file_name(old)))
            && error.kind() != io::ErrorKind::NotFound
        {
            warn!(%error, old, "an old settings backup was not removed");
        }
    }
    Ok(file.info(id, text.len() as u64))
}

// ---- Social: friends' games, the lobby, history asked for from the client ----

impl Service {
    /// The client's phase now, without copying the whole snapshot (the shell's hotkey asks it).
    pub fn phase(&self) -> Phase {
        lock(&self.inner.state).phase
    }

    /// Asks the shell to bring the window up on `puuid`'s history, for a click in the client.
    pub fn open_history(&self, puuid: &str) -> Result<(), CoreError> {
        segment(puuid)?;
        let _ = self.inner.events.send(Event::OpenHistory {
            puuid: puuid.to_owned(),
        });
        Ok(())
    }

    /// Whether `client` is still the connected one: background loops end with their connection.
    fn is_current(&self, client: &Client) -> bool {
        self.client()
            .is_ok_and(|current| Arc::ptr_eq(&current.live, &client.live))
    }

    /// Reads the friends list now and again every [`FRIENDS_REFRESH`] while `client` stays
    /// connected, every [`FRIENDS_RETRY`] until a read has succeeded; events keep it current in
    /// between.
    fn follow_friends(&self, client: &Client) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            loop {
                match client.lcu.get::<Vec<Friend>>(FRIENDS).await {
                    Ok(list) => {
                        lock(&client.live).friends = Some(list);
                        service.render_friends(&client);
                    }
                    Err(error) => debug!(%error, "friends list unavailable"),
                }
                let pause = match lock(&client.live).friends {
                    Some(_) => FRIENDS_REFRESH,
                    None => FRIENDS_RETRY,
                };
                sleep(pause).await;
                if !service.is_current(&client) {
                    return;
                }
            }
        });
    }

    fn on_friends_event(&self, client: &Client, uri: &str, data: serde_json::Value) {
        let due = {
            let mut live = lock(&client.live);
            let Some(list) = live.friends.as_mut() else {
                // Before the first list: it is on its way and will hold this change too.
                return;
            };
            if !friends::apply(list, uri, data) {
                return;
            }
            !std::mem::replace(&mut live.friends_due, true)
        };
        if due {
            let (service, client) = (self.clone(), client.clone());
            self.spawn(async move {
                sleep(FRIENDS_SETTLE).await;
                lock(&client.live).friends_due = false;
                service.render_friends(&client);
            });
        }
    }

    fn render_friends(&self, client: &Client) {
        let Some(list) = lock(&client.live).friends.clone() else {
            return;
        };
        let data = lock(&client.data).clone();
        let queues = data
            .as_deref()
            .map_or(&[][..], |data| data.queues.as_slice());
        self.patch(Patch::Friends(Some(friends::view(&list, queues))));
    }

    fn on_lobby(&self, client: &Client, lobby: Option<Lobby>) {
        {
            let mut live = lock(&client.live);
            // A lobby gone keeps its party: champ select, which follows it, still needs it.
            if let Some(lobby) = &lobby {
                live.party = live::party_of(lobby);
            }
            live.lobby = lobby.clone();
        }
        let focus = lobby.as_ref().and_then(|lobby| {
            let config = &lobby.game_config;
            queue_focus(
                lock(&client.data).as_deref(),
                config.queue_id,
                &config.game_mode,
            )
        });
        for puuid in lobby.iter().flat_map(live::lobby_puuids) {
            self.ensure_player(client, puuid, focus.clone());
        }
        self.render_lobby(client);
    }

    fn reload_lobby(&self, client: &Client) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            if let Ok(lobby) = client.lcu.get_optional::<Lobby>(LOBBY).await {
                service.on_lobby(&client, lobby);
            }
        });
    }

    /// The lobby view, while the phase shows the lobby; the members' stats as known now.
    fn render_lobby(&self, client: &Client) {
        let (lobby, phase, me) = {
            let live = lock(&client.live);
            (live.lobby.clone(), live.phase, live.me.clone())
        };
        let view = lobby.filter(|_| live::shows_lobby(phase)).map(|lobby| {
            // The queue the lobby is set up for decides the kind of game its members are read in.
            let config = &lobby.game_config;
            let focus = queue_focus(
                lock(&client.data).as_deref(),
                config.queue_id,
                &config.game_mode,
            );
            live::lobby_view(&lobby, &me, |puuid| {
                self.player_stats(puuid, focus.as_ref())
            })
        });
        self.patch(Patch::Lobby(view));
    }
}

// ---- The callout's shortcut: what a press does, and what the shell made of it ----

impl Service {
    /// What the callout's shortcut does now (`callout::press`), from the views as drawn and the
    /// settings as saved. The shell acts on it: champ select's lines go out through
    /// [`Self::send_callout`], the game's are typed into the game's chat by the shell itself.
    pub fn callout_press(&self) -> callout::Press {
        let rule = self.settings().automation.callout;
        let state = lock(&self.inner.state);
        callout::press(
            state.phase,
            state.champ_select.as_ref(),
            state.game.as_ref(),
            &rule,
        )
    }

    /// Puts what the shell did for the user, or could not do, in the activity feed: the shortcut
    /// acts while the window may be hidden behind the game.
    pub fn report(&self, kind: NoticeKind) {
        self.notice(kind);
    }

    /// What one press of the callout's shortcut would type in the game under `rule` and `general`,
    /// with the user's own form in every line (`callout::game_preview`); for the settings page.
    pub async fn preview_game_callout(
        &self,
        rule: &CalloutRule,
        general: &General,
    ) -> Result<Vec<String>, CoreError> {
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        let summary = self.player_summary(&me).await?;
        let data = lock(&client.data).clone();
        let champion = |id: i64| {
            data.as_ref()?
                .champions
                .iter()
                .find(|champion| champion.id == id)
                .map(|champion| champion.short_name.clone())
        };
        Ok(callout::game_preview(&summary, rule, general, champion))
    }
}

// ---- History: a player's record and standing, and the caches behind the history pages ----

impl Service {
    /// What `puuid`'s form counts, and the tier, title and quip it earns on its own under the
    /// rating settings (`history::rate_alone`).
    pub async fn player_standing(&self, puuid: &str) -> Result<PlayerStanding, CoreError> {
        let record = self.player_record(puuid).await?;
        let summary = self.summarize(&record, None);
        let data = self.game_data().unwrap_or_default();
        let settings = self.settings();
        let rated = rate_alone(
            &summary,
            &callout::ranking(&settings.automation.callout, &settings.general),
        );
        let catalog = analysis::Catalog {
            kinds: &data.kinds,
            roles: &data.roles,
        };
        Ok(PlayerStanding {
            scope: analysis::form_scope(&record.summoner.puuid, &record.games, &catalog),
            band: rated.as_ref().map(|(_, band)| *band),
            rating: rated.map(|(rating, _)| rating),
        })
    }

    /// `puuid`'s record, the one champ select and the window share. The window takes it while it
    /// is as fresh as the newest page of games beside it (`history::NEWEST_TTL`), else fetches it,
    /// so the form above a list never lags the list.
    async fn player_record(&self, puuid: &str) -> Result<Arc<PlayerRecord>, CoreError> {
        segment(puuid)?;
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        self.scope_account(&me);
        let kinds = catalog_kinds(&client);
        if let Some(record) = lock(&self.inner.players)
            .get(puuid)
            .and_then(|entry| entry.fresh(NEWEST_TTL))
            .filter(|record| record.serves(None, &kinds))
        {
            return Ok(record.clone());
        }
        let record = Arc::new(load_record(&client, puuid, &me, None).await?);
        self.same_account(&client, &me)?;
        let shown = caches::players_shown(&client);
        let mut players = lock(&self.inner.players);
        players.insert(
            puuid.to_owned(),
            PlayerEntry::Ready(record.clone(), Some(Instant::now())),
        );
        caches::trim_players(&mut players, &shown, Instant::now());
        Ok(record)
    }

    /// A record's summary under the catalog as it is now, the form read within `focus`.
    fn summarize(&self, record: &PlayerRecord, focus: Option<&analysis::Focus>) -> PlayerSummary {
        let data = self.game_data().unwrap_or_default();
        analysis::summary(
            &record.summoner,
            record.ranked.as_ref(),
            &record.games,
            &analysis::Catalog {
                kinds: &data.kinds,
                roles: &data.roles,
            },
            focus,
        )
    }

    /// Makes every cache the signed-in account's: another account's pages, games, lookups and
    /// players' records go, so nothing fetched for one is shown to the next.
    fn scope_account(&self, me: &str) {
        if lock(&self.inner.history).scope(me) {
            lock(&self.inner.players).clear();
        }
    }

    /// Refuses an answer fetched for an account that has signed out since.
    fn same_account(&self, client: &Client, me: &str) -> Result<(), CoreError> {
        if self.is_current(client) && lock(&client.live).me == me {
            Ok(())
        } else {
            Err(CoreError::Busy("the signed-in account changed".into()))
        }
    }

    /// A game just ended: the local player and everyone in it have a new game in their history, so
    /// their newest pages and records are asked for again. A view showing them keeps the record it
    /// has until the new one arrives.
    fn forget_newest(&self, client: &Client) {
        let played: Vec<String> = {
            let live = lock(&client.live);
            std::iter::once(live.me.clone())
                .chain(live.gameflow.iter().flat_map(live::game_puuids))
                .collect()
        };
        lock(&self.inner.history).expire_newest(played.iter().map(String::as_str));
        let mut players = lock(&self.inner.players);
        for puuid in &played {
            if let Some(PlayerEntry::Ready(_, at)) = players.get_mut(puuid) {
                *at = None;
            }
        }
    }
}

/// A step out of a game that was played, its stats and its end screen included: somewhere along
/// it the game lands in its players' histories.
fn game_over(previous: Phase, phase: Phase) -> bool {
    let played = |phase: Phase| {
        phase.in_game()
            || matches!(
                phase,
                Phase::WaitingForStats | Phase::PreEndOfGame | Phase::EndOfGame
            )
    };
    previous != phase && played(previous) && !phase.in_game()
}

/// The id of champ select's chat room, once it is open.
async fn champ_select_chat(lcu: &Lcu) -> Result<Option<String>, CoreError> {
    let conversations: Vec<Conversation> = lcu.get("/lol-chat/v1/conversations").await?;
    Ok(conversations
        .into_iter()
        .find(|conversation| conversation.kind.eq_ignore_ascii_case("championSelect"))
        .map(|conversation| conversation.id))
}

/// Posts `lines` to the chat room `chat`. `Me` uses a message type the client shows only locally
/// (`celebration`), so nobody else in the room sees it.
async fn say(lcu: &Lcu, chat: &str, lines: &[String], audience: Audience) -> Result<(), CoreError> {
    let path = format!("/lol-chat/v1/conversations/{}/messages", query_value(chat));
    let kind = match audience {
        Audience::Team => "chat",
        Audience::Me => "celebration",
    };
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            sleep(MESSAGE_GAP).await;
        }
        lcu.post(&path, &json!({ "body": line, "type": kind }))
            .await?;
    }
    Ok(())
}

async fn execute(lcu: &Lcu, action: ChampAction) -> Result<(), lcu::Error> {
    let path = format!("/lol-champ-select/v1/session/actions/{}", action.action_id);
    lcu.patch(&path, &json!({ "championId": action.champion_id }))
        .await?;
    if action.step.completes() {
        lcu.post(&format!("{path}/complete"), &json!({})).await?;
    }
    Ok(())
}

/// The LCU accepts connections before its plugins are up; the summoner answering is the sign
/// that the client is usable. Gives up after two minutes, by which time it was not starting.
async fn wait_until_ready(lcu: &Lcu) -> Result<Summoner, lcu::Error> {
    let mut last = None;
    for _ in 0..60 {
        match lcu.get::<Summoner>(SUMMONER).await {
            Ok(summoner) if !summoner.puuid.is_empty() => return Ok(summoner),
            Ok(_) => {}
            Err(error) => last = Some(error),
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(last.unwrap_or_else(|| {
        lcu::Error::Io(io::Error::other("the client never reported a summoner"))
    }))
}

/// The catalog's entry for `queue_id`.
fn queue_info(data: Option<&GameData>, queue_id: i64) -> Option<&QueueInfo> {
    data?.queues.iter().find(|queue| queue.id == queue_id)
}

/// The kind of game a queue is; `None` before the catalog arrives or for a queue it does not list.
fn queue_mode(client: &Client, queue_id: i64) -> Option<Mode> {
    let data = lock(&client.data).clone();
    queue_info(data.as_deref(), queue_id).map(QueueInfo::mode)
}

/// The kind of game a gameflow session's queue is; `None` while it names none.
fn gameflow_mode(queue: &GameQueue) -> Option<Mode> {
    (!queue.game_mode.is_empty()).then(|| Mode::of(&queue.game_mode, queue.is_ranked))
}

/// One page of `puuid`'s history from wherever the shard keeps it, and whether it is worth keeping:
/// a Tencent client standing in for its shard's server is asked again next time, in case the
/// server is back.
async fn fetch_history(
    client: &Client,
    puuid: &str,
    me: &str,
    begin: u32,
    count: u32,
) -> Result<(HistoryPage, bool), CoreError> {
    match server_history(client, puuid, begin, count).await {
        Some(Ok(page)) => {
            let more = page.entries.len() as u32 >= count;
            let entries = page
                .entries
                .into_iter()
                .map(|entry| entry.map(Arc::new))
                .collect();
            let page = HistoryPage {
                entries,
                more,
                source: HistorySource::Server,
            };
            return Ok((page, true));
        }
        Some(Err(error)) if begin > 0 => return Err(error),
        Some(Err(error)) => warn!(%error, "no match-history server, showing the client's own"),
        None => {}
    }
    let tencent = sgp::base(&client.platform_id).is_some() || client.platform_id.is_empty();
    if tencent && begin > 0 {
        // The client's window is the first page, whatever range is asked.
        let page = HistoryPage {
            entries: Vec::new(),
            more: false,
            source: HistorySource::Client,
        };
        return Ok((page, false));
    }
    let games = history(&client.lcu, puuid, me, begin, count)
        .await?
        .games
        .games;
    let page = HistoryPage {
        more: !tencent && games.len() as u32 >= count,
        entries: games.into_iter().map(|game| Some(Arc::new(game))).collect(),
        source: HistorySource::Client,
    };
    Ok((page, !tencent))
}

/// One page from the shard's match-history server, with the client's own access token for it;
/// `None` where the shard has no server (`sgp::base` knows the Tencent ones only).
async fn server_history(
    client: &Client,
    puuid: &str,
    begin: u32,
    count: u32,
) -> Option<Result<sgp::Page, CoreError>> {
    let (entitlements, version) = tokio::join!(
        client.lcu.get::<Entitlements>("/entitlements/v1/token"),
        client.lcu.get::<String>("/lol-patch/v1/game-version"),
    );
    let entitlements = match entitlements {
        Ok(entitlements) => entitlements,
        // Without a token there is no telling whether the shard has a server.
        Err(error) => return sgp::base(&client.platform_id).map(|_| Err(error.into())),
    };
    let platform = if client.platform_id.is_empty() {
        sgp::platform_of_issuer(&entitlements.issuer).unwrap_or_default()
    } else {
        client.platform_id.clone()
    };
    let base = sgp::base(&platform)?;
    if entitlements.access_token.is_empty() {
        return Some(Err(CoreError::Remote(
            "the client has no access token".into(),
        )));
    }
    let user_agent = sgp::user_agent(&version.unwrap_or_default());
    Some(
        sgp::history(
            base,
            &entitlements.access_token,
            &user_agent,
            puuid,
            begin,
            count,
        )
        .await
        .map_err(CoreError::Remote),
    )
}

/// One page of `puuid`'s games. For the local player either path form has been measured answering
/// HTTP 400 while the other worked (one client, a few hours apart), so a refusal gets one more try.
async fn history(
    lcu: &Lcu,
    puuid: &str,
    me: &str,
    begin: u32,
    count: u32,
) -> Result<MatchList, CoreError> {
    let (path, fallback) = history_paths(puuid, me, begin, count)?;
    match (lcu.get(&path).await, fallback) {
        (Err(error), Some(fallback)) if error.status() == Some(400) => {
            debug!(%error, "match history refused, trying current-summoner");
            Ok(lcu.get(&fallback).await?)
        }
        (result, _) => Ok(result?),
    }
}

/// The path for one page of `puuid`'s games, and for the local player the other form of it.
fn history_paths(
    puuid: &str,
    me: &str,
    begin: u32,
    count: u32,
) -> Result<(String, Option<String>), CoreError> {
    let page = |who: &str| {
        format!(
            "/lol-match-history/v1/products/lol/{who}/matches?begIndex={begin}&endIndex={}",
            begin + count - 1
        )
    };
    let fallback = (!me.is_empty() && puuid == me).then(|| page("current-summoner"));
    Ok((page(segment(puuid)?), fallback))
}

/// `puuid`'s identity, rank and newest games, as many as a form read within `focus` needs. The games
/// come from the shard's match-history server where it has one: all ten players of each, so the form
/// scores every game against its own players, and a Tencent client's own list holds as few as five
/// at first. Elsewhere, or when the server fails, they come from the client's list, the player's
/// own row only.
async fn load_record(
    client: &Client,
    puuid: &str,
    me: &str,
    focus: Option<analysis::Focus>,
) -> Result<PlayerRecord, CoreError> {
    let lcu = &client.lcu;
    let summoner_path = format!("/lol-summoner/v2/summoners/puuid/{}", segment(puuid)?);
    let ranked_path = format!("/lol-ranked/v1/ranked-stats/{puuid}");
    let (summoner, ranked, (games, complete)) = tokio::join!(
        lcu.get::<Summoner>(&summoner_path),
        lcu.get_optional::<RankedStats>(&ranked_path),
        record_games(client, puuid, me, focus.as_ref()),
    );
    // A private profile refuses history and rank; the identity alone is still worth showing.
    Ok(PlayerRecord {
        summoner: summoner?,
        ranked: ranked.ok().flatten(),
        games,
        focus,
        complete,
    })
}

/// The games of a player's record, and whether they are all there are: the shard server's pages
/// until they hold the form's window ([`RECORD_PAGES`]), else the client's own list; none when
/// neither answers.
async fn record_games(
    client: &Client,
    puuid: &str,
    me: &str,
    focus: Option<&analysis::Focus>,
) -> (Vec<Game>, bool) {
    let kinds = catalog_kinds(client);
    let paged = page_through(
        |begin, count| server_history(client, puuid, begin, count),
        |games| analysis::window_filled(games, &kinds, focus),
    )
    .await;
    if let Paged::Games(games, ended) = paged {
        return (games, ended);
    }
    // Wider than the form's twenty, so games form leaves out do not shrink it where the client has
    // more (a Tencent client answers with what it holds whatever the range). It is all there is.
    match history(&client.lcu, puuid, me, 0, 30).await {
        Ok(list) => (list.games.games, true),
        Err(error) => {
            debug!(%error, "match history unavailable");
            (Vec::new(), true)
        }
    }
}

/// What paging the server's history for a record came to.
#[derive(Debug, PartialEq)]
enum Paged {
    /// The server answered: the games, and whether the history ended among them.
    Games(Vec<Game>, bool),
    /// The shard has no server, or its first page failed: the client's own list stands in.
    NoServer,
}

/// Reads `fetch`'s pages ([`RECORD_PAGES`]) until `filled` says the games hold the form's window,
/// the history ends (a short page) or the pages run out. A page failing after the first keeps what
/// came before it.
async fn page_through<F, Fut>(mut fetch: F, filled: impl Fn(&[Game]) -> bool) -> Paged
where
    F: FnMut(u32, u32) -> Fut,
    Fut: Future<Output = Option<Result<sgp::Page, CoreError>>>,
{
    let mut games = Vec::new();
    for (begin, count) in RECORD_PAGES {
        match fetch(begin, count).await {
            Some(Ok(page)) => {
                let ended = (page.entries.len() as u32) < count;
                games.extend(page.entries.into_iter().flatten());
                if ended || filled(&games) {
                    return Paged::Games(games, ended);
                }
            }
            Some(Err(error)) if begin > 0 => {
                debug!(%error, "a further page of history failed; keeping the games before it");
                return Paged::Games(games, false);
            }
            Some(Err(error)) => {
                debug!(%error, "no match-history server, reading the client's own list");
                return Paged::NoServer;
            }
            None => return Paged::NoServer,
        }
    }
    Paged::Games(games, false)
}

/// The kind of game a queue is played as, for the form of the players in it: the catalog's word on
/// `queue_id`, else the mode the session names; `None` while neither says.
fn queue_focus(data: Option<&GameData>, queue_id: i64, game_mode: &str) -> Option<analysis::Focus> {
    let mode = queue_info(data, queue_id).map_or(game_mode, |queue| queue.game_mode.as_str());
    analysis::Focus::of(mode)
}

/// Each queue's kind of game in the connected client's catalog; empty before it arrives.
fn catalog_kinds(client: &Client) -> analysis::QueueKinds {
    lock(&client.data)
        .as_ref()
        .map(|data| data.kinds.clone())
        .unwrap_or_default()
}

/// Rejects anything that could change the meaning of the URL it is placed in.
fn segment(value: &str) -> Result<&str, CoreError> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(value)
    } else {
        Err(CoreError::Invalid(format!("invalid id {value}")))
    }
}

/// Percent-encodes a query value (Riot IDs carry `#`, spaces and CJK).
fn query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queue_says_what_kind_of_game_it_is_once_it_names_a_mode() {
        let queue = |game_mode: &str, ranked: bool| GameQueue {
            game_mode: game_mode.into(),
            is_ranked: ranked,
            ..GameQueue::default()
        };
        assert_eq!(gameflow_mode(&queue("", false)), None);
        assert_eq!(gameflow_mode(&queue("KIWI", false)), Some(Mode::Hextech));
        assert_eq!(gameflow_mode(&queue("CLASSIC", true)), Some(Mode::Ranked));
        let data = GameData {
            queues: vec![QueueInfo {
                id: 2400,
                name: "海克斯大乱斗".into(),
                game_mode: "KIWI".into(),
                ranked: false,
            }],
            ..GameData::default()
        };
        assert_eq!(
            queue_info(Some(&data), 2400).map(QueueInfo::mode),
            Some(Mode::Hextech)
        );
        assert_eq!(queue_info(Some(&data), 420), None);
    }

    #[test]
    fn only_the_local_player_has_a_second_history_path() {
        let (path, fallback) = history_paths("me", "me", 0, 20).unwrap();
        assert_eq!(
            path,
            "/lol-match-history/v1/products/lol/me/matches?begIndex=0&endIndex=19"
        );
        assert_eq!(
            fallback.as_deref(),
            Some(
                "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex=19"
            )
        );
        let other = history_paths("other", "me", 20, 20).unwrap();
        assert_eq!(
            other,
            (
                "/lol-match-history/v1/products/lol/other/matches?begIndex=20&endIndex=39"
                    .to_owned(),
                None
            )
        );
        assert_eq!(history_paths("p", "", 0, 5).unwrap().1, None);
        assert!(history_paths("../x", "me", 0, 1).is_err());
    }

    #[test]
    fn ids_and_queries_cannot_escape_their_url_slot() {
        assert!(segment("0f1e-2d3c_4b").is_ok());
        assert!(segment("../lol-login").is_err());
        assert!(segment("a/b").is_err());
        assert!(segment("").is_err());
        assert_eq!(
            query_value("失了你#65740"),
            "%E5%A4%B1%E4%BA%86%E4%BD%A0%2365740"
        );
        assert_eq!(query_value("a b&c"), "a%20b%26c");
    }

    #[tokio::test]
    async fn patches_bump_the_revision_only_when_something_changed() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let mut events = service.subscribe();
        service.patch(Patch::Phase(Phase::Lobby));
        service.patch(Patch::Phase(Phase::Lobby));
        service.patch(Patch::Connection(Connection::AccessDenied));
        assert_eq!(service.snapshot().rev, 2);
        let Event::Update(first) = events.recv().await.unwrap() else {
            panic!("an update")
        };
        assert_eq!((first.rev, first.patch), (1, Patch::Phase(Phase::Lobby)));
        let Event::Update(second) = events.recv().await.unwrap() else {
            panic!("an update")
        };
        assert_eq!(second.rev, 2);
    }

    #[tokio::test]
    async fn settings_changes_are_saved_and_announced() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let mut events = service.subscribe();
        let mut settings = service.settings();
        settings.automation.play_again = true;
        service.set_settings(settings.clone()).unwrap();
        assert_eq!(
            events.recv().await.unwrap(),
            Event::Settings(Box::new(settings.clone()))
        );
        assert_eq!(
            Service::new(dir.path().join("settings.json"), Handle::current()).settings(),
            settings
        );
    }

    #[tokio::test]
    async fn going_back_to_the_lobby_follows_the_rule_as_it_stands_when_the_wait_ends() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let credentials = Credentials::new(1, "t");
        let gameflow = GameflowSession {
            phase: "EndOfGame".into(),
            game_data: crate::model::GameData {
                queue: GameQueue {
                    id: 2400,
                    game_mode: "KIWI".into(),
                    ..GameQueue::default()
                },
                ..crate::model::GameData::default()
            },
        };
        let client = Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::default(),
            live: Arc::new(Mutex::new(Live {
                phase: Phase::EndOfGame,
                gameflow: Some(gameflow),
                ..Live::default()
            })),
        };
        let mut settings = service.settings();
        settings.automation.play_again = true;
        service.set_settings(settings.clone()).unwrap();
        assert!(service.wants_play_again(&client));

        // Hextech ARAM taken out of the rule while the post-game screen waits.
        settings.automation.scopes.play_again = vec![Mode::Ranked, Mode::Normal];
        service.set_settings(settings.clone()).unwrap();
        assert!(!service.wants_play_again(&client));

        settings.automation.scopes.play_again.push(Mode::Hextech);
        service.set_settings(settings).unwrap();
        assert!(service.wants_play_again(&client));
        lock(&client.live).phase = Phase::Lobby;
        assert!(
            !service.wants_play_again(&client),
            "the user went back first"
        );
    }

    /// A connection whose client never answers (port 1): enough for what the views do with the
    /// state they are handed.
    fn quiet_client(phase: Phase) -> Client {
        let credentials = Credentials::new(1, "t");
        Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::default(),
            live: Arc::new(Mutex::new(Live {
                phase,
                me: "me".into(),
                ..Live::default()
            })),
        }
    }

    #[tokio::test]
    async fn a_burst_of_friend_events_becomes_one_view_once_they_settle() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let client = quiet_client(Phase::None);
        let playing = |puuid: &str| {
            json!({"id": format!("{puuid}@h"), "puuid": puuid, "gameName": puuid, "availability": "dnd",
                   "lol": {"gameStatus": "inProgress", "gameId": "42", "queueId": "450", "timeStamp": "1"}})
        };
        service.on_friends_event(&client, "/lol-chat/v1/friends/a@h", playing("a"));
        assert_eq!(
            service.snapshot().friends,
            None,
            "nothing is drawn before the first list"
        );

        lock(&client.live).friends = Some(Vec::new());
        let mut events = service.subscribe();
        service.on_friends_event(&client, "/lol-chat/v1/friends/a@h", playing("a"));
        service.on_friends_event(&client, "/lol-chat/v1/friends/b%40h", playing("b"));
        let Event::Update(update) = events.recv().await.unwrap() else {
            panic!("an update")
        };
        let Patch::Friends(Some(view)) = update.patch else {
            panic!("the friends view")
        };
        let groups: Vec<(String, Option<u8>)> = view
            .friends
            .iter()
            .map(|friend| (friend.puuid.clone(), friend.group))
            .collect();
        assert_eq!(
            groups,
            [("a".to_owned(), Some(1)), ("b".to_owned(), Some(1))]
        );
        assert!(
            events.try_recv().is_err(),
            "two events, one view: the second came in while the first settled"
        );
    }

    #[tokio::test]
    async fn the_lobby_shows_while_the_client_shows_it_and_leaves_its_party_for_champ_select() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let client = quiet_client(Phase::Lobby);
        let lobby: Lobby = serde_json::from_value(json!({
            "gameConfig": {"queueId": 430},
            "members": [
                {"puuid": "me", "gameName": "Me", "isLeader": true, "firstPositionPreference": "TOP"},
                {"puuid": "mate", "gameName": "Mate"}
            ]
        }))
        .unwrap();
        service.on_lobby(&client, Some(lobby));
        let view = service.snapshot().lobby.expect("a lobby view in the lobby");
        assert_eq!(view.members.len(), 2);
        assert!(view.members[0].is_self && view.members[0].leader);

        lock(&client.live).phase = Phase::ChampSelect;
        service.on_lobby(&client, None);
        assert_eq!(service.snapshot().lobby, None);
        let session: ChampSelectSession = serde_json::from_value(json!({
            "localPlayerCellId": 0,
            "myTeam": [
                {"cellId": 0, "puuid": "me", "gameName": "Me"},
                {"cellId": 1, "puuid": "mate", "gameName": "Mate"},
                {"cellId": 2, "puuid": "stranger", "gameName": "S"}
            ]
        }))
        .unwrap();
        lock(&client.live).champ_select = Some(session);
        service.render(&client);
        let marks: Vec<Option<u8>> = service
            .snapshot()
            .champ_select
            .expect("champ select")
            .my_team
            .iter()
            .map(|seat| seat.premade)
            .collect();
        assert_eq!(
            marks,
            [Some(1), Some(1), None],
            "the lobby's party, gone with the lobby, still marks champ select"
        );
    }

    #[tokio::test]
    async fn requests_without_a_client_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(matches!(
            service.match_detail(1).await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.asset("/lol-login/v1/session").await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.match_history("bad/id", 0, 20).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.set_availability("busy").await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.set_availability("mobile").await,
            Err(CoreError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn the_profile_tools_need_a_client_and_refuse_what_cannot_be_shown() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(matches!(
            service.skins().await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.profile_background().await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.set_profile_background(103015).await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.set_profile_background(0).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.challenge_profile().await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service
                .set_challenge_profile(vec![1, 2, 3, 4], None, None)
                .await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service
                .set_challenge_profile(vec![101304], Some(-1), None)
                .await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service
                .set_challenge_profile(vec![101304], None, Some("24\"}".into()))
                .await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service
                .set_challenge_profile(vec![101304], Some(1436), Some("24".into()))
                .await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.apply_mobile_message().await,
            Err(CoreError::NotConnected)
        ));
        // Switching the disguise on and off with no client changes the settings alone.
        let mut settings = service.settings();
        settings.profile.rank_disguise.enabled = true;
        service.set_settings(settings.clone()).unwrap();
        settings.profile.rank_disguise.enabled = false;
        assert_eq!(service.set_settings(settings.clone()).unwrap(), settings);
    }

    fn snapshot_text(taken_at: i64) -> String {
        serde_json::to_string(&BackupFile::new(
            taken_at,
            json!({ "General": { "WindowMode": 0 } }),
            json!({ "GameEvents": { "evtCastSpell1": "[q]" } }),
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn backups_are_kept_newest_first_ten_at_most() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(
            matches!(service.game_settings_backups(), Err(CoreError::Invalid(_))),
            "the shell has not named a folder"
        );
        let folder = dir.path().join("game-settings");
        service.set_backup_dir(&folder);
        assert_eq!(service.game_settings_backups().unwrap(), Vec::new());

        let first = service
            .import_game_settings_backup(&snapshot_text(1_000))
            .unwrap();
        assert_eq!(
            first.taken_at, 1_000,
            "an import keeps the time it was taken"
        );
        assert_eq!(
            first.channels,
            vec![BackupChannel::General, BackupChannel::Hotkeys]
        );
        let mut ids = vec![first.id];
        for taken_at in 2..=12 {
            ids.push(
                service
                    .import_game_settings_backup(&snapshot_text(taken_at))
                    .unwrap()
                    .id,
            );
        }
        fs::write(folder.join("notes.txt"), "not a backup").unwrap();
        fs::write(folder.join(backup::file_name(1)), "{ broken").unwrap();

        let kept = service.game_settings_backups().unwrap();
        assert_eq!(
            kept.iter().map(|info| info.id).collect::<Vec<_>>(),
            ids.iter()
                .rev()
                .take(backup::KEEP)
                .copied()
                .collect::<Vec<_>>(),
            "the two oldest went, and nothing else is listed"
        );
        assert!(kept.iter().all(|info| info.size > 0));

        let newest = kept[0].id;
        assert!(service.game_settings_backup_path(newest).unwrap().is_file());
        service.delete_game_settings_backup(newest).unwrap();
        service.delete_game_settings_backup(newest).unwrap();
        assert!(matches!(
            service.game_settings_backup_path(newest),
            Err(CoreError::Invalid(_))
        ));
        assert_eq!(
            service.game_settings_backups().unwrap().len(),
            backup::KEEP - 1
        );

        assert!(matches!(
            service.import_game_settings_backup(r#"{"General": {}}"#),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.back_up_game_settings().await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service
                .restore_game_settings(kept[1].id, vec![BackupChannel::General])
                .await,
            Err(CoreError::NotConnected)
        ));
    }

    #[test]
    fn a_restore_refused_in_a_game_says_busy() {
        let error = IpcError::from(CoreError::Busy("in a game".into()));
        assert_eq!(error.code, ErrorCode::Busy);
    }

    // ---- The callout's shortcut ----

    #[tokio::test]
    async fn a_running_game_carries_both_teams_lines_which_the_shortcut_types_only_when_asked() {
        use crate::{settings::GameTeams, view::CalloutSkip};
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let client = quiet_client(Phase::InProgress);
        for (puuid, wins) in [("me", 10), ("strong", 16), ("weak", 4)] {
            // Twenty Rift games of five kills, five deaths and five assists, `wins` of them won: the
            // cache keeps what the client sent, and the form is worked out from it when shown.
            let games = (0..20)
                .map(|index| {
                    serde_json::from_value(json!({
                        "gameId": index + 1, "gameCreation": index + 1, "gameDuration": 1800,
                        "gameMode": "CLASSIC",
                        "participantIdentities": [{"participantId": 1, "player": {"puuid": puuid}}],
                        "participants": [{"participantId": 1, "championId": 1, "stats": {
                            "win": index < wins, "kills": 5, "deaths": 5, "assists": 5
                        }}]
                    }))
                    .unwrap()
                })
                .collect();
            lock(&service.inner.players).insert(
                puuid.into(),
                PlayerEntry::Ready(
                    Arc::new(PlayerRecord {
                        summoner: Summoner {
                            puuid: puuid.into(),
                            summoner_level: 30,
                            ..Summoner::default()
                        },
                        ranked: None,
                        games,
                        focus: None,
                        complete: true,
                    }),
                    Some(Instant::now()),
                ),
            );
        }
        lock(&client.live).gameflow = Some(
            serde_json::from_value(json!({
                "phase": "InProgress",
                "gameData": {
                    "gameId": 9, "queue": {"id": 420, "gameMode": "CLASSIC", "isRanked": true},
                    "teamOne": [{"puuid": "me", "championId": 1, "gameName": "Me", "tagLine": "1"}],
                    "teamTwo": [
                        {"puuid": "strong", "championId": 157, "gameName": "Strong", "tagLine": "2"},
                        {"puuid": "weak", "championId": 86, "gameName": "Weak", "tagLine": "3"},
                        {"puuid": "", "championId": 22, "gameName": "Bot"}
                    ]
                }
            }))
            .unwrap(),
        );
        service.patch(Patch::Phase(Phase::InProgress));
        service.render(&client);
        let game = service.snapshot().game.expect("the game's view");
        let lines = game.callout;
        assert_eq!(
            lines,
            [
                "【敌方·红色方】winer 战绩鉴定",
                "小心【Strong】：人形防御塔，近20场胜率80%，KDA 2.0【常胜将军】",
                "对面【Weak】：移动眼位，近20场胜率20%，可以多抓",
            ],
            "two rated of three, second and fourth of five tiers; no catalog, so the name stands \
             in for the champion; the title is the one Strong's twenty games earn"
        );
        // Callout: the team's own lines, by champion (here the name, for want of a catalog).
        let allies = game.ally_callout;
        assert_eq!(allies.len(), 2, "{allies:?}");
        assert_eq!(allies[0], "【我方·蓝色方】winer 战绩鉴定");
        assert!(
            allies[1].starts_with("峡谷公务员【Me】，近20场胜率50%，KDA 2.0，战力"),
            "rated alone, the middle of five: {allies:?}"
        );
        assert_eq!(
            service.callout_press(),
            callout::Press::Skip(CalloutSkip::InGameOff),
            "nothing is typed into the game by default"
        );
        let mut settings = service.settings();
        settings.automation.callout.in_game = true;
        service.set_settings(settings).unwrap();
        assert_eq!(
            service.callout_press(),
            callout::Press::Game(lines.clone()),
            "the enemy lines by default"
        );
        let mut settings = service.settings();
        settings.automation.callout.game_teams = GameTeams::Allies;
        service.set_settings(settings).unwrap();
        assert_eq!(
            service.callout_press(),
            callout::Press::Game(allies.clone())
        );
        let mut settings = service.settings();
        settings.automation.callout.game_teams = GameTeams::Both;
        service.set_settings(settings).unwrap();
        assert_eq!(
            service.callout_press(),
            callout::Press::Game([lines, allies].concat()),
            "five lines, within the limit"
        );

        service.patch(Patch::Phase(Phase::EndOfGame));
        assert_eq!(
            service.callout_press(),
            callout::Press::Skip(CalloutSkip::NotNow)
        );
        let mut events = service.subscribe();
        service.report(NoticeKind::TypedInGame { lines: 3 });
        assert!(matches!(
            events.recv().await.unwrap(),
            Event::Notice(Notice {
                kind: NoticeKind::TypedInGame { lines: 3 },
                ..
            })
        ));
        assert!(matches!(
            service
                .preview_game_callout(&CalloutRule::default(), &General::default())
                .await,
            Err(CoreError::NotConnected)
        ));
    }

    // History.

    fn record(puuid: &str) -> PlayerEntry {
        PlayerEntry::Ready(
            Arc::new(PlayerRecord {
                summoner: Summoner {
                    puuid: puuid.into(),
                    ..Summoner::default()
                },
                ranked: None,
                games: Vec::new(),
                focus: None,
                complete: true,
            }),
            Some(Instant::now()),
        )
    }

    /// A page of `count` games of `game_mode` from the `begin`-th newest, the history `total` long.
    fn server_page(begin: u32, count: u32, total: u32, game_mode: &str) -> sgp::Page {
        sgp::Page {
            entries: (begin..(begin + count).min(total))
                .map(|index| {
                    Some(Game {
                        game_id: i64::from(index) + 1,
                        game_mode: game_mode.into(),
                        ..Game::default()
                    })
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn a_record_pages_back_until_it_holds_the_forms_window() {
        let kinds = analysis::QueueKinds::new();
        // The newest twenty are Rift games, ARAM ones after them.
        let mode = |index: u32| if index < 20 { "CLASSIC" } else { "KIWI" };
        let asked = std::sync::Mutex::new(Vec::new());
        let fetch = |begin: u32, count: u32| {
            asked.lock().unwrap().push((begin, count));
            let mut page = server_page(begin, count, 500, "");
            for (offset, entry) in page.entries.iter_mut().enumerate() {
                entry.as_mut().unwrap().game_mode = mode(begin + offset as u32).into();
            }
            async move { Some(Ok(page)) }
        };
        let rift = page_through(&fetch, |games| {
            analysis::window_filled(games, &kinds, analysis::Focus::of("CLASSIC").as_ref())
        })
        .await;
        assert!(matches!(&rift, Paged::Games(games, false) if games.len() == 20));
        assert_eq!(*asked.lock().unwrap(), [(0, 20)], "the first page holds it");

        asked.lock().unwrap().clear();
        let aram = page_through(&fetch, |games| {
            analysis::window_filled(games, &kinds, analysis::Focus::of("KIWI").as_ref())
        })
        .await;
        assert!(matches!(&aram, Paged::Games(games, false) if games.len() == 60));
        assert_eq!(*asked.lock().unwrap(), [(0, 20), (20, 40)]);

        // Never more than a hundred games.
        asked.lock().unwrap().clear();
        let never = page_through(&fetch, |_| false).await;
        assert!(matches!(&never, Paged::Games(games, false) if games.len() == 100));
        assert_eq!(*asked.lock().unwrap(), [(0, 20), (20, 40), (60, 40)]);
    }

    #[tokio::test]
    async fn a_short_history_ends_the_paging_and_a_failed_first_page_hands_over_to_the_client() {
        let short = page_through(
            |begin, count| async move { Some(Ok(server_page(begin, count, 30, "KIWI"))) },
            |_| false,
        )
        .await;
        assert!(
            matches!(&short, Paged::Games(games, true) if games.len() == 30),
            "the history ended: the record holds all there is"
        );
        let failed = page_through(
            |_, _| async { Some(Err(CoreError::Remote("401".into()))) },
            |_| false,
        )
        .await;
        assert_eq!(failed, Paged::NoServer);
        let elsewhere = page_through(|_, _| async { None }, |_| false).await;
        assert_eq!(elsewhere, Paged::NoServer, "no server for the shard");
        let later = page_through(
            |begin, count| async move {
                if begin == 0 {
                    Some(Ok(server_page(begin, count, 500, "KIWI")))
                } else {
                    Some(Err(CoreError::Remote("timeout".into())))
                }
            },
            |_| false,
        )
        .await;
        assert!(
            matches!(&later, Paged::Games(games, false) if games.len() == 20),
            "a later page failing keeps the first"
        );
    }

    #[test]
    fn a_record_serves_the_form_it_was_fetched_for_or_one_it_holds_anyway() {
        let kinds = analysis::QueueKinds::new();
        let record = |focus, complete, games: Vec<Game>| PlayerRecord {
            summoner: Summoner::default(),
            ranked: None,
            games,
            focus,
            complete,
        };
        let aram = |count: u32| {
            server_page(0, count, count, "KIWI")
                .entries
                .into_iter()
                .flatten()
                .collect()
        };
        let fetched = record(analysis::Focus::of("KIWI"), false, aram(20));
        assert!(fetched.serves(analysis::Focus::of("ARAM").as_ref(), &kinds));
        assert!(
            fetched.serves(None, &kinds),
            "twenty games against players fill every-kind's window"
        );
        assert!(
            !fetched.serves(analysis::Focus::of("CLASSIC").as_ref(), &kinds),
            "a Rift form pages again"
        );
        let few = record(None, false, aram(8));
        assert!(!few.serves(analysis::Focus::of("KIWI").as_ref(), &kinds));
        let all = record(None, true, aram(8));
        assert!(
            all.serves(analysis::Focus::of("CLASSIC").as_ref(), &kinds),
            "there is no more to fetch"
        );
    }

    #[test]
    fn every_step_out_of_a_played_game_counts_and_nothing_before_it() {
        use Phase::*;
        for (previous, phase) in [
            (InProgress, WaitingForStats),
            (WaitingForStats, PreEndOfGame),
            (PreEndOfGame, EndOfGame),
            (EndOfGame, Lobby),
            (Reconnect, None),
        ] {
            assert!(game_over(previous, phase), "{previous:?} → {phase:?}");
        }
        for (previous, phase) in [
            (ChampSelect, Lobby),
            (GameStart, InProgress),
            (InProgress, Reconnect),
            (Lobby, Matchmaking),
            (EndOfGame, EndOfGame),
        ] {
            assert!(!game_over(previous, phase), "{previous:?} → {phase:?}");
        }
    }

    #[tokio::test]
    async fn a_game_that_ended_makes_its_players_due_and_another_account_sees_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let client = quiet_client(Phase::EndOfGame);
        let player = |puuid: &str| crate::model::GamePlayer {
            puuid: puuid.into(),
            ..crate::model::GamePlayer::default()
        };
        lock(&client.live).gameflow = Some(GameflowSession {
            phase: "EndOfGame".into(),
            game_data: crate::model::GameData {
                team_one: vec![player("me"), player("mate")],
                team_two: vec![player("foe")],
                ..crate::model::GameData::default()
            },
        });
        service.scope_account("me");
        for puuid in ["me", "mate", "foe", "stranger"] {
            lock(&service.inner.players).insert(puuid.into(), record(puuid));
        }
        service.forget_newest(&client);
        let fresh = |puuid: &str| {
            lock(&service.inner.players)
                .get(puuid)
                .and_then(|entry| entry.fresh(PLAYER_TTL))
                .is_some()
        };
        assert!(!fresh("me") && !fresh("mate") && !fresh("foe"));
        assert!(fresh("stranger"), "not in the game");
        assert!(
            matches!(service.player_stats("mate", None), PlayerStats::Ready(_)),
            "a view keeps what it has until the new record arrives"
        );

        service.scope_account("someone-else");
        assert!(matches!(
            service.player_stats("stranger", None),
            PlayerStats::Loading
        ));
        assert_eq!(lock(&service.inner.history).viewer(), "someone-else");
    }
}
