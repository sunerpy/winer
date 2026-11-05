//! The Comebacks and Phrases pages' side of the service (`crate::comeback`): what a press of their
//! shortcuts does now, a line or a quick phrase posted in champ select's chat, and the library
//! downloaded on the user's request, kept in a file of its own beside the settings.

use std::{
    collections::VecDeque,
    future::Future,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lcu::Lcu;
use tracing::{info, warn};

use super::{CHAMP_SELECT, Client, CoreError, Service, champ_select_chat, lock, say};
use crate::{
    callout,
    comeback::{self, ComebackPress, Library, Memory, PhrasePress, TradeEvent},
    model::ChampSelectSession,
    settings::{Audience, CalloutDelivery, write_atomic},
    view::{ComebackLibrary, NoticeKind, Phase, PhraseTrigger},
};

/// What the service keeps for the Comebacks and Phrases pages: what went out last, the library,
/// and champ select's posts in their order.
pub(super) struct ComebackState {
    memory: Mutex<Memory>,
    library: LibraryStore,
    /// Downloading and removing the library, one at a time.
    library_work: tokio::sync::Mutex<()>,
    /// Champ select's posts, one at a time (`Service::post_spaced`): a spaced one checks the
    /// spacing again once it holds this, so two cannot both pass it while one is in flight.
    chat: tokio::sync::Mutex<()>,
    /// The messages trades send (`TradeRule`), in the order their events were seen, and whether a
    /// task is posting them.
    trade_queue: Mutex<(VecDeque<TradeMessage>, bool)>,
}

impl ComebackState {
    /// The library lives in `path` (`comeback-library.json` beside the settings).
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            memory: Mutex::new(Memory::default()),
            library: LibraryStore {
                path,
                loaded: Mutex::new(None),
            },
            library_work: tokio::sync::Mutex::new(()),
            chat: tokio::sync::Mutex::new(()),
            trade_queue: Mutex::new((VecDeque::new(), false)),
        }
    }
}

/// A message a trade sends, its text written when its event was seen: the champ select may have
/// moved on by the time it goes out.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TradeMessage {
    game_id: i64,
    text: String,
    trigger: PhraseTrigger,
}

/// What came of a spaced post in champ select's chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spaced {
    Sent,
    /// Within `comeback::SPACING` of the last line that went out: nothing was posted, and this
    /// much of the spacing is left.
    TooSoon(Duration),
}

/// How a post in champ select's chat counts against the spacing.
#[derive(Clone, Copy, Debug)]
enum Spacing<'a> {
    /// The board's menu: one at a time with the others, but it neither waits nor counts.
    Apart,
    /// A shortcut's or a trade's: nothing within the spacing; once it went out it starts the
    /// spacing, and a comeback's line (`Some`) is the one the next press does not repeat.
    Kept(Option<&'a str>),
}

/// The downloaded library's file, read on first use and kept in memory.
struct LibraryStore {
    path: PathBuf,
    /// `None` until the file was read; then the library, or `None` inside where there is none.
    loaded: Mutex<Option<Option<Arc<Library>>>>,
}

impl LibraryStore {
    fn get(&self) -> Option<Arc<Library>> {
        let mut loaded = lock(&self.loaded);
        if let Some(library) = &*loaded {
            return library.clone();
        }
        let library = match std::fs::read(&self.path) {
            Ok(bytes) => match serde_json::from_slice::<Library>(&bytes) {
                Ok(library) => Some(Arc::new(library)),
                Err(error) => {
                    warn!(%error, "the downloaded library is unreadable; it counts as none");
                    None
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                warn!(%error, "the downloaded library could not be read");
                None
            }
        };
        *loaded = Some(library.clone());
        library
    }

    fn put(&self, library: Library) -> Result<Arc<Library>, CoreError> {
        let bytes =
            serde_json::to_vec(&library).map_err(|error| CoreError::Io(io::Error::other(error)))?;
        let mut loaded = lock(&self.loaded);
        write_atomic(&self.path, &bytes)?;
        let library = Arc::new(library);
        *loaded = Some(Some(library.clone()));
        Ok(library)
    }

    fn remove(&self) -> Result<(), CoreError> {
        let mut loaded = lock(&self.loaded);
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        *loaded = Some(None);
        Ok(())
    }
}

/// The client's side of posting in champ select's chat; the tests use one of their own.
pub(super) trait ChampSelectChat {
    /// The game id of the champ select the client holds now; `None` outside one.
    fn game_id(&self) -> impl Future<Output = Result<Option<i64>, CoreError>> + Send;
    /// Champ select's chat room, once it is open.
    fn room(&self) -> impl Future<Output = Result<Option<String>, CoreError>> + Send;
    /// Posts `body` in `room` as one message to the team.
    fn post(&self, room: &str, body: &str) -> impl Future<Output = Result<(), CoreError>> + Send;
}

pub(super) struct LcuChat<'a>(pub &'a Lcu);

impl ChampSelectChat for LcuChat<'_> {
    async fn game_id(&self) -> Result<Option<i64>, CoreError> {
        match self.0.get::<ChampSelectSession>(CHAMP_SELECT).await {
            Ok(session) => Ok(Some(session.game_id)),
            Err(error) if error.is_not_found() => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn room(&self) -> Result<Option<String>, CoreError> {
        champ_select_chat(self.0).await
    }

    async fn post(&self, room: &str, body: &str) -> Result<(), CoreError> {
        say(
            self.0,
            room,
            &[body.to_owned()],
            Audience::Team,
            CalloutDelivery::Block,
        )
        .await
    }
}

/// Posts `body` in the chat of champ select `game_id` and nowhere else. The client is asked which
/// champ select it holds before the room is looked up and again after, so a champ select that ended
/// while the room was looked up, and one that began meanwhile, get nothing (`Invalid`).
pub(super) async fn post_to(
    chat: &impl ChampSelectChat,
    game_id: i64,
    body: &str,
) -> Result<(), CoreError> {
    let over = || CoreError::Invalid("that champ select is over".into());
    if chat.game_id().await? != Some(game_id) {
        return Err(over());
    }
    let room = chat
        .room()
        .await?
        .ok_or_else(|| CoreError::Invalid("champ select chat is not open".into()))?;
    if chat.game_id().await? != Some(game_id) {
        return Err(over());
    }
    chat.post(&room, body).await
}

/// Now, by the runtime's clock: the system's, except in a test that pauses it.
fn now() -> Instant {
    tokio::time::Instant::now().into_std()
}

/// A random number for [`comeback::pick`]; the clock stands in should the system have none.
fn random() -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_ok() {
        u64::from_le_bytes(bytes)
    } else {
        u64::try_from(super::now_ms()).unwrap_or_default()
    }
}

impl Service {
    /// What a press of the Comebacks page's team shortcut (`all` false) or all-chat shortcut does
    /// now (`comeback::press`), from the phase, champ select's game id and the settings as saved,
    /// against the last line that went out. The shell acts on it: champ select's line goes out
    /// through [`Self::send_comeback`], the game's is typed by the shell, which then tells
    /// [`Self::comeback_went_out`]. A press itself remembers nothing: one that sends nothing neither
    /// starts the spacing nor changes the line not to repeat.
    pub fn comeback_press(&self, all: bool) -> ComebackPress {
        let rule = self.settings().comeback;
        let (phase, champ_select) = {
            let state = lock(&self.inner.state);
            (
                state.phase,
                state.champ_select.as_ref().map(|view| view.game_id),
            )
        };
        let library = if rule.library.enabled {
            self.inner.comeback.library.get()
        } else {
            None
        };
        let pool = comeback::pool(&rule, library.as_deref());
        let memory = lock(&self.inner.comeback.memory).clone();
        comeback::press(
            phase,
            champ_select,
            &rule,
            all,
            &pool,
            &memory,
            Instant::now(),
            random(),
        )
    }

    /// `line` reached the chat, whole or in part, just now: the next press does not repeat it, nor
    /// follow within `comeback::SPACING`.
    pub fn comeback_went_out(&self, line: &str) {
        let mut memory = lock(&self.inner.comeback.memory);
        memory.last = Some(line.to_owned());
        memory.sent_at = Some(Instant::now());
    }

    /// Posts `line` in the chat of champ select `game_id` as one message, as the callout's 发送到队伍
    /// does, and nowhere else (`post_to`), unless the spacing has started again meanwhile (another
    /// post went out since the press was decided); a line that went out is remembered.
    pub async fn send_comeback(&self, line: String, game_id: i64) -> Result<Spaced, CoreError> {
        let client = self.client()?;
        self.post_spaced(
            &LcuChat(&client.lcu),
            game_id,
            &line,
            Spacing::Kept(Some(&line)),
        )
        .await
    }

    /// What a press of the quick phrase's shortcut does now (`comeback::phrase_press`), from the
    /// phase and champ select's session as the client last sent it, its trades among it. The shell
    /// sends the phrase through [`Self::send_phrase_by_shortcut`].
    pub fn phrase_press(&self) -> PhrasePress {
        let phase = lock(&self.inner.state).phase;
        let session = self
            .client()
            .ok()
            .and_then(|client| lock(&client.live).champ_select.clone());
        let memory = lock(&self.inner.comeback.memory).clone();
        comeback::phrase_press(phase, session.as_ref(), &memory, Instant::now())
    }

    /// Posts the first quick phrase for the teammate in cell `cell_id` of champ select `game_id`,
    /// as its shortcut does: behind the spacing, which a phrase that went out starts again.
    pub async fn send_phrase_by_shortcut(
        &self,
        game_id: i64,
        cell_id: i64,
    ) -> Result<Spaced, CoreError> {
        let client = self.client()?;
        let text = self.render_phrase(0, game_id, cell_id)?;
        let spaced = self
            .post_spaced(&LcuChat(&client.lcu), game_id, &text, Spacing::Kept(None))
            .await?;
        if spaced == Spaced::Sent {
            info!("sent a quick phrase by its shortcut");
        }
        Ok(spaced)
    }

    /// Sends quick phrase `index` (`comeback::phrases`) to champ select's chat for the teammate in
    /// cell `cell_id` of champ select `game_id`: their seat and champion filled in. Refused outside
    /// champ select (`Busy`), and for another champ select, the user's own cell or a cell not of
    /// the user's team (`Invalid`): a menu left open from an earlier champ select sends nothing.
    /// It waits for any other post to go first, and is not spaced. Returns the message sent.
    pub async fn send_phrase(
        &self,
        index: usize,
        game_id: i64,
        cell_id: i64,
    ) -> Result<String, CoreError> {
        let client = self.client()?;
        let text = self.render_phrase(index, game_id, cell_id)?;
        self.post_spaced(&LcuChat(&client.lcu), game_id, &text, Spacing::Apart)
            .await?;
        info!(index, "sent a quick phrase");
        Ok(text)
    }

    /// Posts `text` in the chat of champ select `game_id` (`post_to`), one post at a time
    /// (`ComebackState::chat`). A `Kept` post checks the spacing once it holds its turn, and only
    /// one that went out starts the spacing again (and, for a comeback, becomes the line not to
    /// repeat).
    async fn post_spaced(
        &self,
        chat: &impl ChampSelectChat,
        game_id: i64,
        text: &str,
        spacing: Spacing<'_>,
    ) -> Result<Spaced, CoreError> {
        let _turn = self.inner.comeback.chat.lock().await;
        let Spacing::Kept(line) = spacing else {
            post_to(chat, game_id, text).await?;
            return Ok(Spaced::Sent);
        };
        if let Some(left) = self.spacing_left(now()) {
            return Ok(Spaced::TooSoon(left));
        }
        post_to(chat, game_id, text).await?;
        let mut memory = lock(&self.inner.comeback.memory);
        memory.sent_at = Some(now());
        if let Some(line) = line {
            memory.last = Some(line.to_owned());
        }
        Ok(Spaced::Sent)
    }

    /// How much of `comeback::SPACING` is left at `now` since the last line that went out.
    fn spacing_left(&self, now: Instant) -> Option<Duration> {
        let sent_at = lock(&self.inner.comeback.memory).sent_at?;
        comeback::SPACING
            .checked_sub(now.saturating_duration_since(sent_at))
            .filter(|left| !left.is_zero())
    }

    /// What the trades of `session` did (`comeback::trade_events`), and the messages the rule
    /// sends for it, queued in the order seen ([`Self::trade_messages`]).
    pub(super) fn trade_phrases(&self, client: &Client, session: &ChampSelectSession) {
        for message in self.trade_messages(client, session) {
            if self.enqueue_trade_message(message) {
                let service = self.clone();
                self.spawn(async move { service.post_trade_messages().await });
            }
        }
    }

    /// The messages the trades of `session` send under the rule (`TradeRule`): each written now,
    /// from this session's view, `{champion}` the champion asked for. The events are followed
    /// whatever the switches say, so a switch turned on later sends nothing for an earlier ask.
    fn trade_messages(&self, client: &Client, session: &ChampSelectSession) -> Vec<TradeMessage> {
        let events = comeback::trade_events(&mut lock(&client.live).trade_watch, session);
        if events.is_empty() {
            return Vec::new();
        }
        let settings = self.settings();
        let (rule, language) = (&settings.comeback, settings.general.language);
        let mut messages = Vec::new();
        for event in events {
            let (trigger, template, cell_id, theirs) = match event {
                TradeEvent::Asked { cell_id, theirs } if rule.trade.on_ask => {
                    let Some(first) = comeback::phrases(rule, language).into_iter().next() else {
                        continue;
                    };
                    (PhraseTrigger::Asked, first, cell_id, theirs)
                }
                TradeEvent::Done { cell_id, theirs } if rule.trade.on_done => (
                    PhraseTrigger::Done,
                    comeback::thanks(rule, language),
                    cell_id,
                    theirs,
                ),
                _ => continue,
            };
            match self.render_for(&template, session.game_id, cell_id, Some(theirs)) {
                Ok(text) => messages.push(TradeMessage {
                    game_id: session.game_id,
                    text,
                    trigger,
                }),
                Err(error) => warn!(%error, ?trigger, "a trade's message was not written"),
            }
        }
        messages
    }

    /// Puts `message` behind the others; whether no task posts them yet, so one must start.
    fn enqueue_trade_message(&self, message: TradeMessage) -> bool {
        let mut queue = lock(&self.inner.comeback.trade_queue);
        queue.0.push_back(message);
        !std::mem::replace(&mut queue.1, true)
    }

    /// The next queued trade message; `None` once the queue is empty, which also marks that no task
    /// posts them any more.
    fn next_trade_message(&self) -> Option<TradeMessage> {
        let mut queue = lock(&self.inner.comeback.trade_queue);
        let next = queue.0.pop_front();
        if next.is_none() {
            queue.1 = false;
        }
        next
    }

    /// Posts the queued trade messages one after another, in their order, each waiting out the
    /// spacing rather than being dropped: it is the only one for that teammate. A champ select
    /// that ended meanwhile gets nothing (`post_to` checks its game id).
    async fn post_trade_messages(&self) {
        while let Some(message) = self.next_trade_message() {
            let notice = match self.client() {
                Ok(client) => {
                    self.post_trade_message(&LcuChat(&client.lcu), &message)
                        .await
                }
                Err(error) => NoticeKind::Failed {
                    action: "phrase".into(),
                    message: error.to_string(),
                },
            };
            self.report(notice);
        }
    }

    /// Posts one trade message through `chat`, waiting out the spacing; what came of it.
    async fn post_trade_message(
        &self,
        chat: &impl ChampSelectChat,
        message: &TradeMessage,
    ) -> NoticeKind {
        loop {
            match self
                .post_spaced(chat, message.game_id, &message.text, Spacing::Kept(None))
                .await
            {
                Ok(Spaced::Sent) => {
                    info!(trigger = ?message.trigger, "sent a trade's message");
                    return NoticeKind::PhraseSent {
                        trigger: message.trigger,
                    };
                }
                Ok(Spaced::TooSoon(left)) => tokio::time::sleep(left).await,
                Err(error) => {
                    return NoticeKind::Failed {
                        action: "phrase".into(),
                        message: error.to_string(),
                    };
                }
            }
        }
    }

    /// The text quick phrase `index` reads for cell `cell_id` of champ select `game_id`.
    fn render_phrase(&self, index: usize, game_id: i64, cell_id: i64) -> Result<String, CoreError> {
        let settings = self.settings();
        let phrases = comeback::phrases(&settings.comeback, settings.general.language);
        let template = phrases
            .get(index)
            .ok_or_else(|| CoreError::Invalid("no such quick phrase".into()))?;
        self.render_for(template, game_id, cell_id, None)
    }

    /// `template` filled in for the teammate in cell `cell_id` of champ select `game_id`, from the
    /// view held now: their seat and name, and `champion`'s name, or the one they hold for `None`.
    /// Refused outside champ select (`Busy`), and for another champ select, the user's own cell or
    /// a cell not of the user's team (`Invalid`).
    fn render_for(
        &self,
        template: &str,
        game_id: i64,
        cell_id: i64,
        champion: Option<i64>,
    ) -> Result<String, CoreError> {
        let (phase, view) = {
            let state = lock(&self.inner.state);
            (state.phase, state.champ_select.clone())
        };
        let view = view
            .filter(|_| phase == Phase::ChampSelect)
            .ok_or_else(|| CoreError::Busy("not in champ select".into()))?;
        if view.game_id != game_id {
            return Err(CoreError::Invalid("that champ select is over".into()));
        }
        let (number, seat) = view
            .my_team
            .iter()
            .enumerate()
            .find(|(_, seat)| seat.cell_id == Some(cell_id))
            .ok_or_else(|| CoreError::Invalid("no teammate sits in that cell".into()))?;
        if seat.is_self {
            return Err(CoreError::Invalid("that seat is the user's own".into()));
        }
        let language = self.settings().general.language;
        let champion_id = champion.unwrap_or(seat.champion_id);
        let champion = self
            .game_data()
            .and_then(|data| {
                data.champions
                    .iter()
                    .find(|champion| champion.id == champion_id)
                    .map(|champion| champion.short_name.clone())
            })
            .unwrap_or_default();
        let name = seat
            .name
            .as_ref()
            .map(|name| name.game_name.clone())
            .unwrap_or_default();
        Ok(comeback::phrase(
            template,
            &callout::seat_label(number + 1, language),
            &champion,
            &name,
        ))
    }

    /// The connected client's process id, for the shell's check of the rights a game it starts
    /// will have.
    pub fn client_pid(&self) -> Option<u32> {
        self.client().ok()?.credentials.pid
    }

    /// The downloaded library as the Comebacks page shows it; answers without a client.
    pub fn comeback_library(&self) -> ComebackLibrary {
        self.inner
            .comeback
            .library
            .get()
            .map(|library| library.status())
            .unwrap_or_default()
    }

    /// Downloads the library (`comeback::library_urls`), checks it is the pinned file and keeps its
    /// lines; only ever on the user's request, after the Comebacks page's confirmation. The switch
    /// that uses it goes off first: every download is opted into again. A failed download changes
    /// nothing: a file that cannot be written puts the switch back as it was.
    pub async fn download_comeback_library(&self) -> Result<ComebackLibrary, CoreError> {
        self.download_library(
            crate::net::web_client().map_err(CoreError::Remote)?,
            &comeback::library_urls(),
            comeback::LIBRARY_SIZE,
            comeback::LIBRARY_SHA256,
        )
        .await
    }

    /// [`Self::download_comeback_library`] from `urls`, for a file of `size` bytes and `sha256`.
    async fn download_library(
        &self,
        client: &reqwest::Client,
        urls: &[String],
        size: usize,
        sha256: &str,
    ) -> Result<ComebackLibrary, CoreError> {
        let _work = self.inner.comeback.library_work.lock().await;
        let library = comeback::download(client, urls, size, sha256, super::now_ms())
            .await
            .map_err(CoreError::Remote)?;
        let was_on = self.switch_library_off()?;
        let library = match self.inner.comeback.library.put(library) {
            Ok(library) => library,
            Err(error) => {
                warn!(%error, "the downloaded library could not be written");
                if was_on {
                    self.switch_library(true)?;
                }
                return Err(error);
            }
        };
        info!(
            mild = library.mild.len(),
            strong = library.strong.len(),
            "downloaded the library"
        );
        Ok(library.status())
    }

    /// Deletes the downloaded library, its switch turned off first; a press after it finds none.
    pub async fn remove_comeback_library(&self) -> Result<ComebackLibrary, CoreError> {
        let _work = self.inner.comeback.library_work.lock().await;
        self.switch_library_off()?;
        self.inner.comeback.library.remove()?;
        info!("removed the downloaded library");
        Ok(ComebackLibrary::default())
    }

    /// Saves the library's switch off, where it is on; whether it was.
    fn switch_library_off(&self) -> Result<bool, CoreError> {
        let was_on = self.settings().comeback.library.enabled;
        if was_on {
            self.switch_library(false)?;
        }
        Ok(was_on)
    }

    fn switch_library(&self, on: bool) -> Result<(), CoreError> {
        let mut settings = self.settings();
        settings.comeback.library.enabled = on;
        self.set_settings(settings).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::atomic::{AtomicBool, AtomicI64, Ordering},
        thread,
    };

    use lcu::{Credentials, Lcu};
    use tokio::runtime::Handle;

    use super::*;
    use crate::{
        service::{Client, Live},
        settings::TradeRule,
        view::{
            ChampSelectView, ChampionInfo, GameData, PhraseSkip, PlayerStats, RiotId, Seat,
            TimerView,
        },
    };

    /// The fixture `sqlite.rs` reads: harmless words shaped as the library is.
    const FIXTURE: &[u8] = include_bytes!("../testdata/comeback-library.db");

    fn seat(cell_id: i64, is_self: bool, champion_id: i64, name: Option<&str>) -> Seat {
        Seat {
            puuid: Some(format!("p{cell_id}")),
            name: name.and_then(|name| RiotId::new(name, "1")),
            champion_id,
            intent: false,
            position: None,
            spells: [0, 0],
            is_self,
            premade: None,
            premade_inferred: false,
            autofilled: false,
            note: None,
            stats: PlayerStats::Loading,
            rating: None,
            cell_id: Some(cell_id),
        }
    }

    /// A service in champ select `game_id`: the user in cell 0, a teammate on 龙王 in cell 1, the
    /// other team in cells 5 and 6; a client that never answers (port 1).
    fn in_champ_select(dir: &tempfile::TempDir, game_id: i64) -> Service {
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let credentials = Credentials::new(1, "t");
        let data = GameData {
            champions: vec![ChampionInfo {
                id: 136,
                name: "铸星龙王".into(),
                short_name: "龙王".into(),
                alias: "AurelionSol".into(),
                icon: String::new(),
            }],
            ..GameData::default()
        };
        *service
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::new(Mutex::new(Some(Arc::new(data)))),
            live: Arc::new(Mutex::new(Live::default())),
        });
        seat_champ_select(&service, game_id);
        service
    }

    /// Puts champ select `game_id` in the service's snapshot. Saving settings draws the views again
    /// from the client's own state, which these tests leave empty: they seat it again after.
    fn seat_champ_select(service: &Service, game_id: i64) {
        let mut state = lock(&service.inner.state);
        state.phase = Phase::ChampSelect;
        state.champ_select = Some(ChampSelectView {
            game_id,
            queue_id: 420,
            timer: TimerView::default(),
            my_team: vec![seat(0, true, 1, Some("Me")), seat(1, false, 136, None)],
            their_team: vec![seat(5, false, 2, None), seat(6, false, 3, None)],
            my_bans: Vec::new(),
            their_bans: Vec::new(),
            bench_enabled: false,
            bench: Vec::new(),
            rerolls_remaining: 0,
            callout: Vec::new(),
            side: None,
            recommendations: Vec::new(),
        });
    }

    #[tokio::test]
    async fn a_phrase_goes_only_to_a_teammate_of_this_champ_select() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        assert_eq!(
            service.render_phrase(0, 7, 1).unwrap(),
            "2L大佬，小弟想玩下龙王，给您磕一个了",
            "the teammate's seat in the list and their champion"
        );
        let refused = |result: Result<String, CoreError>| match result {
            Err(CoreError::Invalid(_)) => true,
            other => panic!("not refused as invalid: {other:?}"),
        };
        assert!(
            refused(service.render_phrase(0, 8, 1)),
            "another champ select"
        );
        assert!(
            refused(service.render_phrase(0, 7, 0)),
            "the user's own cell"
        );
        assert!(
            refused(service.render_phrase(0, 7, 5)),
            "the other team's cell"
        );
        assert!(
            refused(service.render_phrase(0, 7, 9)),
            "a cell nobody sits in"
        );
        assert!(refused(service.render_phrase(1, 7, 1)), "no second phrase");
        let mut settings = service.settings();
        settings.comeback.phrases = vec!["{seat}【{name}】，{champion}给我吗".into()];
        service.set_settings(settings).unwrap();
        seat_champ_select(&service, 7);
        assert_eq!(
            service.render_phrase(0, 7, 1).unwrap(),
            "2L，龙王给我吗",
            "a hidden name takes its brackets along"
        );
        // Past the checks, the phrase goes to the client, which here never answers.
        assert!(matches!(
            service.send_phrase(0, 7, 1).await,
            Err(CoreError::Lcu(_))
        ));
        lock(&service.inner.state).phase = Phase::InProgress;
        assert!(matches!(
            service.send_phrase(0, 7, 1).await,
            Err(CoreError::Busy(_))
        ));
    }

    #[tokio::test]
    async fn the_quick_phrases_shortcut_reads_the_trade_asked_from_the_clients_session() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        let held = |trades: serde_json::Value| {
            let session: ChampSelectSession =
                serde_json::from_value(serde_json::json!({ "gameId": 7, "trades": trades }))
                    .unwrap();
            let client = service.client().unwrap();
            lock(&client.live).champ_select = Some(session);
        };
        assert_eq!(
            service.phrase_press(),
            PhrasePress::Skip(PhraseSkip::NotChampSelect),
            "no session from the client yet"
        );
        held(serde_json::json!([{ "id": 3, "cellId": 1, "state": "AVAILABLE" }]));
        assert_eq!(
            service.phrase_press(),
            PhrasePress::Skip(PhraseSkip::NoTrade)
        );
        held(serde_json::json!([{ "id": 4, "cellId": 1, "state": "SENT" }]));
        assert_eq!(
            service.phrase_press(),
            PhrasePress::Send {
                game_id: 7,
                cell_id: 1
            }
        );
        service.comeback_went_out("a line");
        assert_eq!(
            service.phrase_press(),
            PhrasePress::Skip(PhraseSkip::TooSoon),
            "a comeback just went out"
        );
        lock(&service.inner.comeback.memory).sent_at = None;
        let chat = FakeChat::new(&[Some(7), Some(7)], Some("room"));
        assert_eq!(
            service
                .post_spaced(&chat, 7, "2L换吗", Spacing::Kept(None))
                .await
                .unwrap(),
            Spaced::Sent
        );
        let memory = lock(&service.inner.comeback.memory).clone();
        assert!(memory.sent_at.is_some(), "a phrase starts the spacing");
        assert_eq!(
            memory.last.as_deref(),
            Some("a line"),
            "and is no line to avoid"
        );
        lock(&service.inner.state).phase = Phase::InProgress;
        assert_eq!(
            service.phrase_press(),
            PhrasePress::Skip(PhraseSkip::NotChampSelect)
        );
    }

    /// A client whose champ select is each of `game_ids` in turn as it is asked, with chat room
    /// `room`; it keeps what is posted.
    struct FakeChat {
        game_ids: Mutex<std::collections::VecDeque<Option<i64>>>,
        room: Option<String>,
        posts: Mutex<Vec<(String, String)>>,
    }

    impl FakeChat {
        fn new(game_ids: &[Option<i64>], room: Option<&str>) -> Self {
            Self {
                game_ids: Mutex::new(game_ids.iter().copied().collect()),
                room: room.map(str::to_owned),
                posts: Mutex::default(),
            }
        }
    }

    impl ChampSelectChat for FakeChat {
        async fn game_id(&self) -> Result<Option<i64>, CoreError> {
            Ok(lock(&self.game_ids)
                .pop_front()
                .expect("asked no more often than set up"))
        }

        async fn room(&self) -> Result<Option<String>, CoreError> {
            Ok(self.room.clone())
        }

        async fn post(&self, room: &str, body: &str) -> Result<(), CoreError> {
            lock(&self.posts).push((room.to_owned(), body.to_owned()));
            Ok(())
        }
    }

    /// Champ select `game_id` as the client sends it: the user in cell 0, the team's champions by
    /// cell, and the trades by cell and state.
    fn trade_session(
        game_id: i64,
        team: &[(i64, i64)],
        trades: &[(i64, &str)],
    ) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "gameId": game_id,
            "localPlayerCellId": 0,
            "myTeam": team
                .iter()
                .map(|(cell, champion)| serde_json::json!({ "cellId": cell, "championId": champion }))
                .collect::<Vec<_>>(),
            "trades": trades
                .iter()
                .map(|(cell, state)| serde_json::json!({ "id": 1, "cellId": cell, "state": state }))
                .collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    fn trade_rule(service: &Service, game_id: i64, change: impl FnOnce(&mut TradeRule)) {
        let mut settings = service.settings();
        change(&mut settings.comeback.trade);
        service.set_settings(settings).unwrap();
        seat_champ_select(service, game_id);
    }

    #[tokio::test]
    async fn a_trade_writes_its_messages_when_seen_and_only_with_their_switches_on() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        let client = service.client().unwrap();
        let asked = trade_session(7, &[(0, 1), (1, 136)], &[(1, "SENT")]);
        let swapped = trade_session(7, &[(0, 136), (1, 1)], &[]);
        assert_eq!(service.trade_messages(&client, &asked), [], "both off");
        trade_rule(&service, 7, |rule| {
            rule.on_ask = true;
            rule.on_done = true;
        });
        assert_eq!(
            service.trade_messages(&client, &asked),
            [],
            "that ask was seen while off"
        );
        assert_eq!(
            service.trade_messages(&client, &swapped),
            [TradeMessage {
                game_id: 7,
                text: "谢谢2L大佬！".into(),
                trigger: PhraseTrigger::Done
            }],
            "but followed: its swap is thanked"
        );
        // Another champ select: the ask names the champion asked for, the thank-you the one got.
        seat_champ_select(&service, 8);
        assert_eq!(
            service.trade_messages(
                &client,
                &trade_session(8, &[(0, 1), (1, 136)], &[(1, "SENT")])
            ),
            [TradeMessage {
                game_id: 8,
                text: "2L大佬，小弟想玩下龙王，给您磕一个了".into(),
                trigger: PhraseTrigger::Asked
            }]
        );
        trade_rule(&service, 8, |rule| {
            rule.thanks = "{seat}【{name}】，{champion}我拿走了".into();
        });
        assert_eq!(
            service.trade_messages(&client, &trade_session(8, &[(0, 136), (1, 1)], &[])),
            [TradeMessage {
                game_id: 8,
                text: "2L，龙王我拿走了".into(),
                trigger: PhraseTrigger::Done
            }]
        );
        trade_rule(&service, 9, |rule| rule.on_ask = false);
        assert_eq!(
            service.trade_messages(
                &client,
                &trade_session(9, &[(0, 1), (1, 136)], &[(1, "SENT")])
            ),
            [],
            "the ask's switch off"
        );
    }

    #[tokio::test]
    async fn a_trades_message_goes_to_the_client_and_what_came_of_it_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        trade_rule(&service, 7, |rule| rule.on_ask = true);
        let mut events = service.subscribe();
        let client = service.client().unwrap();
        service.trade_phrases(
            &client,
            &trade_session(7, &[(0, 1), (1, 136)], &[(1, "SENT")]),
        );
        // The test client never answers: the post was tried, and the feed says it failed.
        let notice = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(crate::view::Event::Notice(notice)) = events.recv().await {
                    return notice.kind;
                }
            }
        })
        .await
        .unwrap();
        assert!(
            matches!(&notice, NoticeKind::Failed { action, .. } if action == "phrase"),
            "{notice:?}"
        );
    }

    /// A client in champ select `game_id` whose posts each wait for a permit of `gate`, or are
    /// refused; it keeps what is posted, and when.
    struct HeldChat {
        game_id: AtomicI64,
        gate: tokio::sync::Semaphore,
        refuse: AtomicBool,
        posts: Mutex<Vec<(String, tokio::time::Instant)>>,
    }

    impl HeldChat {
        fn new(game_id: i64) -> Arc<Self> {
            Arc::new(Self {
                game_id: AtomicI64::new(game_id),
                gate: tokio::sync::Semaphore::new(0),
                refuse: AtomicBool::new(false),
                posts: Mutex::default(),
            })
        }

        fn bodies(&self) -> Vec<String> {
            lock(&self.posts)
                .iter()
                .map(|(body, _)| body.clone())
                .collect()
        }
    }

    impl ChampSelectChat for HeldChat {
        async fn game_id(&self) -> Result<Option<i64>, CoreError> {
            Ok(Some(self.game_id.load(Ordering::SeqCst)))
        }

        async fn room(&self) -> Result<Option<String>, CoreError> {
            Ok(Some("room".into()))
        }

        async fn post(&self, _room: &str, body: &str) -> Result<(), CoreError> {
            self.gate.acquire().await.unwrap().forget();
            if self.refuse.load(Ordering::SeqCst) {
                return Err(CoreError::Invalid("refused".into()));
            }
            lock(&self.posts).push((body.to_owned(), tokio::time::Instant::now()));
            Ok(())
        }
    }

    /// Lets every task spawned so far run until it waits.
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn champ_selects_posts_go_one_at_a_time_and_only_one_that_went_out_counts() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        let chat = HeldChat::new(7);
        chat.refuse.store(true, Ordering::SeqCst);
        chat.gate.add_permits(1);
        assert!(
            service
                .post_spaced(&*chat, 7, "a", Spacing::Kept(None))
                .await
                .is_err()
        );
        assert_eq!(
            lock(&service.inner.comeback.memory).sent_at,
            None,
            "a refused post starts no spacing"
        );
        chat.refuse.store(false, Ordering::SeqCst);
        let post = |text: &'static str| {
            let (service, chat) = (service.clone(), chat.clone());
            tokio::spawn(async move {
                service
                    .post_spaced(&*chat, 7, text, Spacing::Kept(None))
                    .await
            })
        };
        // The first holds its turn while the client takes it; the second, decided meanwhile, finds
        // the spacing started once its turn comes.
        let first = post("first");
        settle().await;
        let second = post("second");
        settle().await;
        chat.gate.add_permits(1);
        assert_eq!(first.await.unwrap().unwrap(), Spaced::Sent);
        assert!(matches!(
            second.await.unwrap().unwrap(),
            Spaced::TooSoon(left) if left <= comeback::SPACING
        ));
        assert_eq!(chat.bodies(), ["first"]);
        // The board's menu: in turn with the others, neither held back nor counted.
        let before = lock(&service.inner.comeback.memory).sent_at;
        tokio::time::advance(Duration::from_secs(1)).await;
        chat.gate.add_permits(1);
        assert_eq!(
            service
                .post_spaced(&*chat, 7, "menu", Spacing::Apart)
                .await
                .unwrap(),
            Spaced::Sent
        );
        assert_eq!(lock(&service.inner.comeback.memory).sent_at, before);
        assert_eq!(chat.bodies(), ["first", "menu"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_trades_messages_go_out_in_order_each_after_the_spacing_as_they_were_written() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        trade_rule(&service, 7, |rule| {
            rule.on_ask = true;
            rule.on_done = true;
        });
        let client = service.client().unwrap();
        let mut messages = service.trade_messages(
            &client,
            &trade_session(7, &[(0, 1), (1, 136)], &[(1, "SENT")]),
        );
        messages
            .extend(service.trade_messages(&client, &trade_session(7, &[(0, 136), (1, 1)], &[])));
        let texts: Vec<String> = messages
            .iter()
            .map(|message| message.text.clone())
            .collect();
        assert_eq!(
            texts,
            ["2L大佬，小弟想玩下龙王，给您磕一个了", "谢谢2L大佬！"]
        );
        // The view moves on before they go out: the teammate holds another champion now.
        lock(&service.inner.state)
            .champ_select
            .as_mut()
            .unwrap()
            .my_team[1]
            .champion_id = 2;
        // Another post holds its turn while both are queued.
        let chat = HeldChat::new(7);
        let held = {
            let (service, chat) = (service.clone(), chat.clone());
            tokio::spawn(async move {
                service
                    .post_spaced(&*chat, 7, "a comeback", Spacing::Kept(Some("a comeback")))
                    .await
            })
        };
        settle().await;
        let mut queued = messages.into_iter();
        assert!(
            service.enqueue_trade_message(queued.next().unwrap()),
            "the first starts the task"
        );
        assert!(
            !service.enqueue_trade_message(queued.next().unwrap()),
            "the second waits behind it"
        );
        let drained = {
            let (service, chat) = (service.clone(), chat.clone());
            tokio::spawn(async move {
                let mut notices = Vec::new();
                while let Some(message) = service.next_trade_message() {
                    notices.push(service.post_trade_message(&*chat, &message).await);
                }
                notices
            })
        };
        settle().await;
        chat.gate.add_permits(3);
        assert_eq!(held.await.unwrap().unwrap(), Spaced::Sent);
        let notices = drained.await.unwrap();
        assert!(matches!(
            notices.as_slice(),
            [
                NoticeKind::PhraseSent {
                    trigger: PhraseTrigger::Asked
                },
                NoticeKind::PhraseSent {
                    trigger: PhraseTrigger::Done
                }
            ]
        ));
        let posts = lock(&chat.posts).clone();
        let bodies: Vec<&str> = posts.iter().map(|(body, _)| body.as_str()).collect();
        assert_eq!(
            bodies,
            ["a comeback", texts[0].as_str(), texts[1].as_str()],
            "in order, as written when seen"
        );
        assert!(
            posts[1].1 - posts[0].1 >= comeback::SPACING,
            "the ask waited out the spacing"
        );
        assert!(
            posts[2].1 - posts[1].1 >= comeback::SPACING,
            "and the thank-you after it"
        );
        assert!(
            service.enqueue_trade_message(TradeMessage {
                game_id: 7,
                text: "late".into(),
                trigger: PhraseTrigger::Done
            }),
            "drained, a new message starts a task again"
        );
        // A champ select that ended during the wait gets nothing.
        let message = service.next_trade_message().unwrap();
        chat.game_id.store(8, Ordering::SeqCst);
        assert!(matches!(
            service.post_trade_message(&*chat, &message).await,
            NoticeKind::Failed { .. }
        ));
        assert!(!chat.bodies().contains(&"late".to_owned()));
    }

    #[tokio::test]
    async fn a_line_goes_only_to_the_champ_select_it_was_meant_for() {
        let held = FakeChat::new(&[Some(7), Some(7)], Some("room"));
        post_to(&held, 7, "hi").await.unwrap();
        assert_eq!(*lock(&held.posts), [("room".to_owned(), "hi".to_owned())]);

        let over = FakeChat::new(&[Some(8)], Some("room"));
        assert!(matches!(
            post_to(&over, 7, "hi").await,
            Err(CoreError::Invalid(_))
        ));
        assert!(lock(&over.posts).is_empty(), "another champ select is on");

        // The champ select ended and the next began while the room was being looked up: the room
        // found is the new one's, and nothing goes to it.
        let switched = FakeChat::new(&[Some(7), Some(8)], Some("next room"));
        assert!(matches!(
            post_to(&switched, 7, "hi").await,
            Err(CoreError::Invalid(_))
        ));
        assert!(lock(&switched.posts).is_empty());

        let ended = FakeChat::new(&[Some(7), None], Some("room"));
        assert!(post_to(&ended, 7, "hi").await.is_err());
        assert!(lock(&ended.posts).is_empty(), "no champ select any more");

        let closed = FakeChat::new(&[Some(7)], None);
        assert!(matches!(
            post_to(&closed, 7, "hi").await,
            Err(CoreError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn only_a_line_that_went_out_starts_the_spacing_or_is_kept_from_repeating() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        let mut settings = service.settings();
        settings.comeback.lines = vec!["a".into(), "b".into()];
        service.set_settings(settings).unwrap();
        seat_champ_select(&service, 7);
        let line = |press: ComebackPress| match press {
            ComebackPress::ChampSelect { line, .. } => line,
            other => panic!("not sent: {other:?}"),
        };
        let first = line(service.comeback_press(false));
        let _ = line(service.comeback_press(false));
        // The client never answers: the line did not go out, so nothing is remembered.
        assert!(service.send_comeback(first.clone(), 7).await.is_err());
        assert!(lock(&service.inner.comeback.memory).last.is_none());
        let _ = line(service.comeback_press(false));
        service.comeback_went_out(&first);
        assert!(matches!(
            service.comeback_press(false),
            ComebackPress::Skip {
                reason: crate::view::CalloutSkip::TooSoon,
                ..
            }
        ));
        // Three seconds after it finished going out, any line but that one.
        lock(&service.inner.comeback.memory).sent_at = Some(Instant::now() - comeback::SPACING);
        for _ in 0..10 {
            assert_ne!(line(service.comeback_press(false)), first);
        }
    }

    /// Answers `routes` (a path, a status and a body each), one connection at a time, `count`
    /// connections in all; returns the base URL.
    fn serve(routes: Vec<(&'static str, u16, Vec<u8>)>, count: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for _ in 0..count {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let (status, body) = routes
                    .iter()
                    .find(|(route, ..)| *route == path)
                    .map_or((404, &[][..]), |(_, status, body)| {
                        (*status, body.as_slice())
                    });
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        base
    }

    async fn download(service: &Service, urls: &[String]) -> Result<ComebackLibrary, CoreError> {
        service
            .download_library(
                crate::net::client().unwrap(),
                urls,
                FIXTURE.len(),
                &comeback::digest(FIXTURE),
            )
            .await
    }

    fn switch_on(service: &Service) {
        let mut settings = service.settings();
        settings.comeback.library.enabled = true;
        service.set_settings(settings).unwrap();
    }

    #[tokio::test]
    async fn a_download_tries_the_next_mirror_turns_its_switch_off_and_outlives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let base = serve(
            vec![("/refuse", 503, Vec::new()), ("/ok", 200, FIXTURE.to_vec())],
            2,
        );
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(!service.comeback_library().downloaded, "none before");
        switch_on(&service);
        let status = download(&service, &[format!("{base}/refuse"), format!("{base}/ok")])
            .await
            .unwrap();
        assert!(status.downloaded);
        assert_eq!((status.mild, status.strong), (62, 73));
        assert!(status.fetched_at.is_some());
        assert!(
            !service.settings().comeback.library.enabled,
            "a download is opted into again"
        );
        assert!(dir.path().join("comeback-library.json").exists());
        let restarted = Service::new(dir.path().join("settings.json"), Handle::current());
        assert_eq!(
            restarted.comeback_library(),
            status,
            "read back after a restart"
        );
    }

    #[tokio::test]
    async fn the_library_is_used_only_switched_on_and_never_after_removal() {
        let dir = tempfile::tempdir().unwrap();
        let base = serve(vec![("/ok", 200, FIXTURE.to_vec())], 1);
        let service = in_champ_select(&dir, 7);
        download(&service, &[format!("{base}/ok")]).await.unwrap();
        let skipped = |press: ComebackPress| match press {
            ComebackPress::Skip { reason, .. } => reason,
            other => panic!("not skipped: {other:?}"),
        };
        assert_eq!(
            skipped(service.comeback_press(false)),
            crate::view::CalloutSkip::NoLines,
            "downloaded, not switched on"
        );
        switch_on(&service);
        seat_champ_select(&service, 7);
        let ComebackPress::ChampSelect { line, game_id } = service.comeback_press(false) else {
            panic!("a library line for champ select");
        };
        assert_eq!(game_id, 7, "bound to the champ select it was pressed in");
        let library = library_now(&service);
        assert!(library.mild.contains(&line), "from the chosen level");
        let removed = service.remove_comeback_library().await.unwrap();
        assert!(!removed.downloaded);
        assert!(
            !service.settings().comeback.library.enabled,
            "switched off first"
        );
        assert!(!dir.path().join("comeback-library.json").exists());
        switch_on(&service);
        seat_champ_select(&service, 7);
        assert_eq!(
            skipped(service.comeback_press(false)),
            crate::view::CalloutSkip::NoLines,
            "switched on again, nothing left to pick"
        );
    }

    fn library_now(service: &Service) -> Arc<Library> {
        service.inner.comeback.library.get().expect("a library")
    }

    #[tokio::test]
    async fn other_bytes_or_more_of_them_are_refused_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut other = FIXTURE.to_vec();
        other[2000] ^= 0xFF;
        let mut more = FIXTURE.to_vec();
        more.push(0);
        let base = serve(vec![("/other", 200, other), ("/more", 200, more)], 2);
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        switch_on(&service);
        let Err(CoreError::Remote(error)) =
            download(&service, &[format!("{base}/other"), format!("{base}/more")]).await
        else {
            panic!("refused");
        };
        assert!(error.contains("not the file winer knows"), "{error}");
        assert!(error.contains("more bytes"), "{error}");
        assert!(!dir.path().join("comeback-library.json").exists());
        assert!(!service.comeback_library().downloaded);
        assert!(
            service.settings().comeback.library.enabled,
            "a failed download leaves the switch as it was"
        );
    }

    #[tokio::test]
    async fn a_library_that_cannot_be_written_leaves_the_old_one_and_its_switch() {
        let dir = tempfile::tempdir().unwrap();
        let base = serve(vec![("/ok", 200, FIXTURE.to_vec())], 2);
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let status = download(&service, &[format!("{base}/ok")]).await.unwrap();
        switch_on(&service);
        // The file's place is taken by a directory, so the next write fails.
        let file = dir.path().join("comeback-library.json");
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert!(matches!(
            download(&service, &[format!("{base}/ok")]).await,
            Err(CoreError::Io(_))
        ));
        assert!(
            service.settings().comeback.library.enabled,
            "the switch is put back as it was"
        );
        assert_eq!(
            service.comeback_library(),
            status,
            "the library held is kept"
        );
    }

    /// The real file through the real mirrors: needs the internet, so it runs only when asked
    /// (`cargo test -p winer-core pinned_library -- --ignored`), before a pull request.
    #[tokio::test]
    #[ignore = "downloads from the internet"]
    async fn the_pinned_library_downloads_and_reads() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let status = service.download_comeback_library().await.unwrap();
        let library = library_now(&service);
        println!(
            "mild {} strong {} sha256 {} source {}",
            status.mild, status.strong, library.sha256, library.source
        );
        assert_eq!(library.sha256, comeback::LIBRARY_SHA256);
        assert!(status.mild > 600 && status.strong > 900, "{status:?}");
        assert!(
            library
                .mild
                .iter()
                .chain(&library.strong)
                .all(|line| !line.is_empty() && line.chars().count() <= 200)
        );
    }
}
