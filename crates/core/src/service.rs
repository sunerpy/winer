//! The connection to the client and everything that follows from it: one loop that finds the
//! client, mirrors its state into a [`Snapshot`] and acts on it for the user.

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::{Duration, Instant},
};

use lcu::{Credentials, DiscoverError, EventKind, Lcu};
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::{
    runtime::Handle,
    sync::{Semaphore, broadcast},
    time::sleep,
};
use tracing::{debug, info, warn};

use crate::{
    analysis, augments,
    automation::{self, Availability, ChampAction, Step},
    callout, catalog,
    friends::{self, FRIENDS, Friend},
    live,
    model::{
        ChampSelectSession, ChatMe, Conversation, Entitlements, GameQueue, GameflowSession, Lobby,
        MatchList, RankedStats, ReadyCheck, Summoner,
    },
    settings::{Audience, CalloutRule, General, Language, Mode, Scoped, Settings, SettingsStore},
    sgp,
    view::{
        AugmentDetail, Connection, ErrorCode, Event, GameData, HistorySource, IpcError,
        MatchDetail, MatchPage, Me, Notice, NoticeKind, Patch, Phase, PlayerProfile, PlayerStats,
        PlayerSummary, Presence, QueueInfo, Snapshot, Update,
    },
};

const PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
const GAMEFLOW: &str = "/lol-gameflow/v1/session";
const READY_CHECK: &str = "/lol-matchmaking/v1/ready-check";
const CHAMP_SELECT: &str = "/lol-champ-select/v1/session";
const SUMMONER: &str = "/lol-summoner/v1/current-summoner";
const RANKED: &str = "/lol-ranked/v1/current-ranked-stats";
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
const FAILED_PLAYER_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("the League client is not connected")]
    NotConnected,
    #[error("no player is called {0}")]
    PlayerNotFound(String),
    #[error("{0}")]
    Invalid(String),
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
    assets: Mutex<HashMap<String, Arc<Asset>>>,
    /// Augment descriptions, per language, fetched at most once per run.
    augment_details: tokio::sync::Mutex<HashMap<Language, Arc<Vec<AugmentDetail>>>>,
}

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
    // Social.
    /// The friends as the client last listed them; `None` until it has once.
    friends: Option<Vec<Friend>>,
    /// A rebuild of the friends view is due once the events settle.
    friends_due: bool,
    /// The lobby while there is one.
    lobby: Option<Lobby>,
    /// The local player's party as the last lobby had it, for champ select's premade marks.
    party: Vec<String>,
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
    Ready(Arc<PlayerSummary>, Instant),
    Failed(String, Instant),
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
        Self {
            inner: Arc::new(Inner {
                settings: SettingsStore::open(settings_path),
                state: Mutex::new(Snapshot::default()),
                events,
                runtime,
                client: RwLock::new(None),
                players: Mutex::new(HashMap::new()),
                player_slots: Semaphore::new(4),
                assets: Mutex::new(HashMap::new()),
                augment_details: tokio::sync::Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn start(&self) {
        let service = self.clone();
        self.inner.runtime.spawn(async move { service.run().await });
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
        let saved = self.inner.settings.set(settings)?;
        let _ = self
            .inner
            .events
            .send(Event::Settings(Box::new(saved.clone())));
        // Tier names and the callout's lines come from the settings.
        if let Ok(client) = self.client() {
            self.render(&client);
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
    /// it answers every page with the same newest twenty (`sgp`).
    pub async fn match_history(
        &self,
        puuid: &str,
        begin: u32,
        count: u32,
    ) -> Result<MatchPage, CoreError> {
        segment(puuid)?;
        let count = count.clamp(1, 50);
        let client = self.client()?;
        let summaries = |games: &[crate::model::Game]| -> Vec<_> {
            games
                .iter()
                .filter_map(|game| analysis::match_summary(puuid, game))
                .collect()
        };
        match server_history(&client, puuid, begin, count).await {
            Some(Ok(page)) => {
                return Ok(MatchPage {
                    puuid: puuid.to_owned(),
                    begin,
                    games: summaries(&page.games),
                    has_more: page.returned as u32 >= count,
                    source: HistorySource::Server,
                });
            }
            Some(Err(error)) if begin > 0 => return Err(error),
            Some(Err(error)) => warn!(%error, "no match-history server, showing the client's own"),
            None => {}
        }
        let me = lock(&client.live).me.clone();
        let tencent = sgp::base(&client.platform_id).is_some() || client.platform_id.is_empty();
        if tencent && begin > 0 {
            // The client's window is the first page, whatever range is asked.
            return Ok(MatchPage {
                puuid: puuid.to_owned(),
                begin,
                games: Vec::new(),
                has_more: false,
                source: HistorySource::Client,
            });
        }
        let list = history(&client.lcu, puuid, &me, begin, count).await?;
        let games = summaries(&list.games.games);
        Ok(MatchPage {
            puuid: puuid.to_owned(),
            begin,
            has_more: !tencent && games.len() as u32 >= count,
            games,
            source: HistorySource::Client,
        })
    }

    pub async fn match_detail(&self, game_id: i64) -> Result<MatchDetail, CoreError> {
        let game = self
            .client()?
            .lcu
            .get(&format!("/lol-match-history/v1/games/{game_id}"))
            .await?;
        Ok(analysis::match_detail(&game))
    }

    /// Looks a player up by Riot ID, `name#tag`.
    pub async fn find_player(&self, riot_id: &str) -> Result<PlayerProfile, CoreError> {
        let riot_id = riot_id.trim();
        let (name, tag) = riot_id.rsplit_once('#').unwrap_or((riot_id, ""));
        if name.trim().is_empty() {
            return Err(CoreError::Invalid(
                "enter a Riot ID such as name#tag".into(),
            ));
        }
        let lcu = self.client()?.lcu;
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
        Ok(analysis::profile(&summoner, ranked.as_ref()))
    }

    pub async fn player_summary(&self, puuid: &str) -> Result<PlayerSummary, CoreError> {
        segment(puuid)?;
        if let Some(PlayerEntry::Ready(summary, at)) = lock(&self.inner.players).get(puuid)
            && at.elapsed() < PLAYER_TTL
        {
            return Ok((**summary).clone());
        }
        let client = self.client()?;
        let me = lock(&client.live).me.clone();
        let summary = load_summary(&client.lcu, puuid, &me).await?;
        lock(&self.inner.players).insert(
            puuid.to_owned(),
            PlayerEntry::Ready(Arc::new(summary.clone()), Instant::now()),
        );
        Ok(summary)
    }

    /// A game-data asset such as a champion icon. Only `/lol-game-data/assets/` is served: the
    /// window must not reach the rest of the LCU through the asset protocol.
    pub async fn asset(&self, path: &str) -> Result<Arc<Asset>, CoreError> {
        if !path.starts_with("/lol-game-data/assets/") || path.contains("..") {
            return Err(CoreError::Invalid(format!("not a game-data asset: {path}")));
        }
        if let Some(asset) = lock(&self.inner.assets).get(path) {
            return Ok(asset.clone());
        }
        let (content_type, bytes) = self.client()?.lcu.bytes(path).await?;
        let asset = Arc::new(Asset {
            content_type: content_type.unwrap_or_else(|| "application/octet-stream".into()),
            bytes,
        });
        let mut assets = lock(&self.inner.assets);
        if assets.len() >= 1024 {
            assets.clear();
        }
        assets.insert(path.to_owned(), asset.clone());
        Ok(asset)
    }

    /// `chat`, `away` or `offline`: the states the client offers itself. A request for `dnd` is
    /// ignored (the client marks games on its own), and `mobile` is the phone app's, which the
    /// desktop client shows as 在线分组 (measured on 16.19, `docs/platform-notes.md`).
    pub async fn set_availability(&self, availability: &str) -> Result<(), CoreError> {
        if !matches!(availability, "chat" | "away" | "offline") {
            return Err(CoreError::Invalid(format!(
                "unknown availability {availability}"
            )));
        }
        let lcu = self.client()?.lcu;
        lcu.put("/lol-chat/v1/me", &json!({ "availability": availability }))
            .await?;
        // The client answers 201 to a state it then declines; only reading back tells.
        let me: ChatMe = lcu.get("/lol-chat/v1/me").await?;
        if me.availability != availability {
            return Err(CoreError::Invalid(format!(
                "the client kept {} instead of {availability}",
                me.availability
            )));
        }
        Ok(())
    }

    pub async fn set_status_message(&self, message: &str) -> Result<(), CoreError> {
        self.client()?
            .lcu
            .put("/lol-chat/v1/me", &json!({ "statusMessage": message }))
            .await?;
        Ok(())
    }

    pub async fn presence(&self) -> Result<Presence, CoreError> {
        let me: ChatMe = self.client()?.lcu.get("/lol-chat/v1/me").await?;
        Ok(Presence {
            availability: me.availability,
            status_message: me.status_message,
        })
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

    /// What each Hextech ARAM augment does, from ARAM.GG, once per run and language; nothing while
    /// the user has switched it off. A failure is not cached, so the next view tries again.
    pub async fn augment_details(&self) -> Result<Arc<Vec<AugmentDetail>>, CoreError> {
        let general = self.settings().general;
        if !general.augment_details {
            return Ok(Arc::default());
        }
        // Held across the fetch: two views asking at once make one request.
        let mut cache = self.inner.augment_details.lock().await;
        if let Some(details) = cache.get(&general.language) {
            return Ok(details.clone());
        }
        let details = Arc::new(
            augments::fetch(general.language)
                .await
                .map_err(CoreError::Remote)?,
        );
        info!(count = details.len(), "augment descriptions loaded");
        cache.insert(general.language, details.clone());
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
    /// session are untouched.
    pub async fn restart_client_ui(&self) -> Result<(), CoreError> {
        self.client()?
            .lcu
            .post("/riotclient/kill-and-restart-ux", &json!({}))
            .await?;
        Ok(())
    }

    /// Restarts the client's UI only while the player is idle in it, outside any lobby, queue,
    /// champ select or game, so a newly linked loader starts without interrupting anything. The
    /// phase is asked of the client itself: right after connecting, the snapshot may not have it
    /// yet. Returns whether it restarted.
    pub async fn restart_client_ui_when_idle(&self) -> Result<bool, CoreError> {
        let client = self.client()?;
        let phase: String = client.lcu.get(PHASE).await?;
        if Phase::parse(&phase) != Phase::None {
            return Ok(false);
        }
        client
            .lcu
            .post("/riotclient/kill-and-restart-ux", &json!({}))
            .await?;
        Ok(true)
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
            .get::<ChatMe>("/lol-chat/v1/me")
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
                if complete {
                    return;
                }
                sleep(Duration::from_secs(3 * attempt)).await;
            }
        });
    }

    async fn on_phase(&self, client: &Client, phase: Phase) {
        lock(&client.live).phase = phase;
        self.patch(Patch::Phase(phase));
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
        for puuid in live::champ_select_puuids(&session) {
            self.ensure_player(client, puuid);
        }
        self.render(client);
        self.automate(client);
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
        for puuid in session.iter().flat_map(live::game_puuids) {
            self.ensure_player(client, puuid);
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
        let stats = |puuid: &str| self.player_stats(puuid);
        let mut mode = None;
        let view = champ_select.map(|session| {
            let data = lock(&client.data).clone();
            // The catalog says what kind of game the queue is.
            let queue = queue_info(data.as_deref(), session.queue_id);
            mode = queue.map(QueueInfo::mode);
            let game_mode = queue.map_or("", |queue| queue.game_mode.as_str());
            let mut view = live::champ_select_view(&session, stats, &ranking, game_mode);
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
            self.patch(Patch::Game(live::game_view(&session, &me, stats, &ranking)));
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

    fn player_stats(&self, puuid: &str) -> PlayerStats {
        match lock(&self.inner.players).get(puuid) {
            Some(PlayerEntry::Ready(summary, _)) => {
                PlayerStats::Ready(Box::new((**summary).clone()))
            }
            Some(PlayerEntry::Failed(message, _)) => PlayerStats::Failed {
                message: message.clone(),
            },
            Some(PlayerEntry::Loading) | None => PlayerStats::Loading,
        }
    }

    fn ensure_player(&self, client: &Client, puuid: String) {
        {
            let mut players = lock(&self.inner.players);
            let fresh = match players.get(&puuid) {
                Some(PlayerEntry::Loading) => true,
                Some(PlayerEntry::Ready(_, at)) => at.elapsed() < PLAYER_TTL,
                Some(PlayerEntry::Failed(_, at)) => at.elapsed() < FAILED_PLAYER_TTL,
                None => false,
            };
            if fresh {
                return;
            }
            if players.len() > 500 {
                players.retain(|_, entry| matches!(entry, PlayerEntry::Ready(_, at) if at.elapsed() < PLAYER_TTL));
            }
            players.insert(puuid.clone(), PlayerEntry::Loading);
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let entry = {
                let _slot = service.inner.player_slots.acquire().await;
                let me = lock(&client.live).me.clone();
                match load_summary(&client.lcu, &puuid, &me).await {
                    Ok(summary) => PlayerEntry::Ready(Arc::new(summary), Instant::now()),
                    Err(error) => {
                        debug!(%error, "player stats unavailable");
                        PlayerEntry::Failed(error.to_string(), Instant::now())
                    }
                }
            };
            lock(&service.inner.players).insert(puuid, entry);
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
        for puuid in lobby.iter().flat_map(live::lobby_puuids) {
            self.ensure_player(client, puuid);
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
        let view = lobby
            .filter(|_| live::shows_lobby(phase))
            .map(|lobby| live::lobby_view(&lobby, &me, |puuid| self.player_stats(puuid)));
        self.patch(Patch::Lobby(view));
    }
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

async fn load_summary(lcu: &Lcu, puuid: &str, me: &str) -> Result<PlayerSummary, CoreError> {
    let summoner_path = format!("/lol-summoner/v2/summoners/puuid/{}", segment(puuid)?);
    let ranked_path = format!("/lol-ranked/v1/ranked-stats/{puuid}");
    let (summoner, ranked, history) = tokio::join!(
        lcu.get::<Summoner>(&summoner_path),
        lcu.get_optional::<RankedStats>(&ranked_path),
        // Wider than the form's twenty, so custom games (left out of form) do not shrink it.
        history(lcu, puuid, me, 0, 30),
    );
    let summoner = summoner?;
    if let Err(error) = &history {
        debug!(%error, "match history unavailable");
    }
    // A private profile refuses history and rank; the identity alone is still worth showing.
    let games = history.map(|list| list.games.games).unwrap_or_default();
    Ok(analysis::summary(
        &summoner,
        ranked.ok().flatten().as_ref(),
        &games,
    ))
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
    }
}
