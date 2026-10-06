//! The loopback socket the in-client plugin talks to.
//!
//! The plugin runs inside the client page and cannot read files, so the desktop app writes the
//! port and a fresh token into `bootstrap.json` beside the plugin's `index.js` (see
//! [`crate::plugin`]). The plugin fetches that sibling file, connects to
//! `ws://127.0.0.1:<port>/?token=<token>` and from then on receives the same [`Event`]s as the
//! window. A connection with any other token is refused during the HTTP upgrade.
//!
//! The plugin can also ask for a player's latest games ([`PluginMessage::History`]); the answer
//! comes back on the same socket ([`BridgeMessage::HistoryResult`]), for the history panel the
//! plugin draws over the client page.

use std::{
    net::Ipv4Addr,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use futures_util::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast::error::RecvError, mpsc},
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{ErrorResponse, Request, Response},
        http::StatusCode,
    },
};
use tracing::{debug, info, warn};
use ts_rs::TS;

use crate::{
    CoreError, Service,
    settings::Settings,
    view::{Award, Event, GameData, GameKind, IpcError, MatchPage, Snapshot},
};

/// Core → plugin. Built, serialized and dropped at once, so the variants' sizes do not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BridgeMessage {
    /// The whole state, first on every connection and again whenever the plugin fell behind.
    Hello {
        version: String,
        snapshot: Snapshot,
        settings: Settings,
    },
    Event {
        event: Event,
    },
    // The history panel in the client.
    /// The answer to [`PluginMessage::History`] under its `request_id`: the player's latest games,
    /// or why there are none. Exactly one of `page` and `error` is set.
    HistoryResult {
        request_id: u32,
        page: Option<PanelHistory>,
        error: Option<IpcError>,
    },
}

/// Plugin → core.
#[derive(Clone, Debug, PartialEq, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PluginMessage {
    Hello {
        version: String,
        context: String,
    },
    /// Lands in winer's own log, which is how the in-client half is diagnosed.
    Log {
        level: LogLevel,
        message: String,
    },
    /// The user clicked an ARAM bench champion in the client. Carried out only while the
    /// `plugin.benchNoCooldown` setting is on.
    BenchSwap {
        champion_id: i64,
    },
    /// The user clicked a lobby member in the client: the window comes up on their history.
    OpenHistory {
        puuid: String,
    },
    // The history panel in the client.
    /// The user clicked a player in the client's lobby or champ select while the history panel is
    /// on: winer looks up their latest games and answers with a [`BridgeMessage::HistoryResult`]
    /// carrying the same `request_id`.
    History {
        puuid: String,
        request_id: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone)]
pub struct Bridge {
    port: u16,
    token: Arc<str>,
    connected: Arc<AtomicU32>,
}

impl Bridge {
    /// Binds a random loopback port and serves until the process exits.
    pub async fn start(service: Service, version: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let bridge = Self {
            port: listener.local_addr()?.port(),
            token: token()?.into(),
            connected: Arc::default(),
        };
        info!(port = bridge.port, "plugin bridge listening");
        let (accepting, version) = (bridge.clone(), version.to_owned());
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let (bridge, service, version) =
                            (accepting.clone(), service.clone(), version.clone());
                        tokio::spawn(async move { bridge.serve(stream, service, version).await });
                    }
                    Err(error) => warn!(%error, "plugin bridge accept failed"),
                }
            }
        });
        Ok(bridge)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Plugin contexts connected right now. The client runs two per page load.
    pub fn connected(&self) -> u32 {
        self.connected.load(Ordering::Relaxed)
    }

    // tungstenite fixes the refusal type of the upgrade callback.
    #[allow(clippy::result_large_err)]
    async fn serve(&self, stream: TcpStream, service: Service, version: String) {
        let authorize = |request: &Request, response: Response| {
            if request.uri().query().is_some_and(|query| {
                query_token(query).is_some_and(|token| same(token, &self.token))
            }) {
                Ok(response)
            } else {
                let mut refusal = ErrorResponse::new(Some("unauthorized".into()));
                *refusal.status_mut() = StatusCode::UNAUTHORIZED;
                Err(refusal)
            }
        };
        let socket = match accept_hdr_async(stream, authorize).await {
            Ok(socket) => socket,
            Err(error) => return debug!(%error, "plugin connection refused"),
        };
        self.connected.fetch_add(1, Ordering::Relaxed);
        let (mut sink, mut incoming) = socket.split();
        let mut events = service.subscribe();
        let hello = || BridgeMessage::Hello {
            version: version.clone(),
            snapshot: service.snapshot(),
            settings: service.settings(),
        };
        let mut outgoing = Some(hello());
        let mut context = String::from("?");
        // The history panel: lookups run off the loop and hand their answers back through here.
        let (answers, mut answered) = mpsc::channel::<BridgeMessage>(PANEL_LOOKUPS);
        let mut lookups: usize = 0;

        loop {
            if let Some(message) = outgoing.take() {
                let text = serde_json::to_string(&message).expect("bridge messages serialize");
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => outgoing = Some(BridgeMessage::Event { event }),
                    // Too slow to follow the patches: start it over from the whole state.
                    Err(RecvError::Lagged(_)) => outgoing = Some(hello()),
                    Err(RecvError::Closed) => break,
                },
                Some(answer) = answered.recv() => {
                    lookups = lookups.saturating_sub(1);
                    outgoing = Some(answer);
                }
                message = incoming.next() => match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<PluginMessage>(&text) {
                        Ok(PluginMessage::Hello { version, context: id }) => {
                            info!(%version, context = %id, "plugin connected");
                            context = id;
                            // A restarted interface is back (`Service::restart_client_ui_when_idle`).
                            service.plugin_connected();
                        }
                        Ok(PluginMessage::Log { level, message }) => log(&context, level, &message),
                        Ok(PluginMessage::BenchSwap { champion_id }) => swap(&service, champion_id),
                        Ok(PluginMessage::OpenHistory { puuid }) => {
                            if let Err(error) = service.open_history(&puuid) {
                                warn!(%error, "history asked for from the client not opened");
                            }
                        }
                        Ok(PluginMessage::History { puuid, request_id }) if lookups < PANEL_LOOKUPS => {
                            lookups += 1;
                            answer_history(&service, puuid, request_id, answers.clone());
                        }
                        Ok(PluginMessage::History { request_id, .. }) => {
                            outgoing = Some(history_busy(request_id));
                        }
                        Err(error) => debug!(%error, "unreadable plugin message"),
                    },
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                },
            }
        }
        self.connected.fetch_sub(1, Ordering::Relaxed);
        info!(%context, "plugin disconnected");
    }
}

/// A bench swap asked for from inside the client, carried out off the socket's loop.
fn swap(service: &Service, champion_id: i64) {
    if !service.settings().plugin.bench_no_cooldown {
        return debug!(
            champion_id,
            "in-client bench swap ignored: the setting is off"
        );
    }
    let service = service.clone();
    tokio::spawn(async move {
        match service.bench_swap(champion_id).await {
            Ok(()) => info!(champion_id, "bench swap from the client"),
            Err(error) => warn!(%error, champion_id, "bench swap from the client failed"),
        }
    });
}

fn log(context: &str, level: LogLevel, message: &str) {
    let message: String = message.chars().take(2000).collect();
    match level {
        LogLevel::Debug => debug!(target: "plugin", %context, "{message}"),
        LogLevel::Info => info!(target: "plugin", %context, "{message}"),
        LogLevel::Warn => warn!(target: "plugin", %context, "{message}"),
        LogLevel::Error => tracing::error!(target: "plugin", %context, "{message}"),
    }
}

// ---- The history panel in the client ----

/// Games the history panel lists: the newest of the History page's first page.
pub const PANEL_GAMES: u32 = 10;
/// Lookups one connection may have under way at once; one more is answered as busy.
const PANEL_LOOKUPS: usize = 4;
/// The longest one lookup may take: the shard's server (20 s at most) and then the client's own
/// history.
const PANEL_TIMEOUT: Duration = Duration::from_secs(30);
/// A puuid is 78 characters; nothing much longer is one.
const PUUID_MAX: usize = 128;

/// A player's latest games, as the history panel in the client lists them.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PanelHistory {
    pub puuid: String,
    /// Newest first, at most [`PANEL_GAMES`].
    pub games: Vec<PanelGame>,
}

/// One game in the history panel, from the player's side.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PanelGame {
    pub game_id: i64,
    pub queue_id: i64,
    /// The queue's name in the client's catalog (`极地大乱斗`); empty where the catalog has none.
    pub queue: String,
    pub champion_id: i64,
    pub win: bool,
    pub remake: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    /// Epoch milliseconds.
    pub started_at: i64,
    /// Seconds.
    pub duration: i64,
    pub award: Option<Award>,
    /// Arena placement, 1–8.
    pub placement: Option<i64>,
}

impl BridgeMessage {
    /// The answer to the history request `request_id`.
    fn history_result(request_id: u32, result: Result<PanelHistory, IpcError>) -> Self {
        let (page, error) = match result {
            Ok(page) => (Some(page), None),
            Err(error) => (None, Some(error)),
        };
        Self::HistoryResult {
            request_id,
            page,
            error,
        }
    }
}

/// The panel's games from a page of history: the newest [`PANEL_GAMES`], each queue named as the
/// client's catalog names it; with `hide_custom`, the newest that are not custom games, as the
/// History page shows them.
pub fn panel_history(page: MatchPage, data: Option<&GameData>, hide_custom: bool) -> PanelHistory {
    let queue = |id: i64| {
        data.and_then(|data| data.queues.iter().find(|queue| queue.id == id))
            .map(|queue| queue.name.trim().to_owned())
            .unwrap_or_default()
    };
    PanelHistory {
        puuid: page.puuid,
        games: page
            .games
            .into_iter()
            .filter(|game| !(hide_custom && game.kind == GameKind::Custom))
            .take(PANEL_GAMES as usize)
            .map(|game| PanelGame {
                game_id: game.game_id,
                queue_id: game.queue_id,
                queue: queue(game.queue_id),
                champion_id: game.line.champion_id,
                win: game.line.win,
                remake: game.line.remake,
                kills: game.line.kills,
                deaths: game.line.deaths,
                assists: game.line.assists,
                started_at: game.started_at,
                duration: game.duration,
                award: game.line.award,
                placement: game.line.placement,
            })
            .collect(),
    }
}

/// `puuid`'s latest games, looked up as the History page looks up its first page. While the History
/// page hides custom games the panel does too, and reads twice as many to fill its list.
async fn panel_lookup(service: &Service, puuid: &str) -> Result<PanelHistory, IpcError> {
    if puuid.len() > PUUID_MAX {
        return Err(
            CoreError::Invalid(format!("not a player id ({} characters)", puuid.len())).into(),
        );
    }
    let hide_custom = service.settings().history.hide_custom_games;
    let count = if hide_custom {
        2 * PANEL_GAMES
    } else {
        PANEL_GAMES
    };
    let lookup = service.match_history(puuid, 0, count);
    let page = tokio::time::timeout(PANEL_TIMEOUT, lookup)
        .await
        .map_err(|_| CoreError::Remote("the games did not arrive in time".into()))??;
    Ok(panel_history(
        page,
        service.game_data().as_deref(),
        hide_custom,
    ))
}

/// The answer to one lookup more than a connection may have under way.
fn history_busy(request_id: u32) -> BridgeMessage {
    let busy = CoreError::Busy("winer is still reading other games".into());
    BridgeMessage::history_result(request_id, Err(busy.into()))
}

/// Looks the player up off the socket's loop and hands the answer back to it.
fn answer_history(
    service: &Service,
    puuid: String,
    request_id: u32,
    answers: mpsc::Sender<BridgeMessage>,
) {
    let service = service.clone();
    tokio::spawn(async move {
        let result = panel_lookup(&service, &puuid).await;
        match &result {
            Ok(page) => info!(
                games = page.games.len(),
                "history for the panel in the client"
            ),
            Err(error) => warn!(error = %error.message, "no history for the panel in the client"),
        }
        // The plugin may have gone meanwhile; then the answer goes nowhere.
        let _ = answers
            .send(BridgeMessage::history_result(request_id, result))
            .await;
    });
}

fn query_token(query: &str) -> Option<&str> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

/// Compares without an early exit, so timing does not reveal how much of a guess was right.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |diff, (x, y)| diff | (x ^ y))
            == 0
}

fn token() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    #[test]
    fn tokens_are_compared_whole() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
        assert_eq!(query_token("x=1&token=t0k"), Some("t0k"));
        assert_eq!(query_token("tokens=1"), None);
        assert_eq!(token().unwrap().len(), 64);
    }

    #[test]
    fn plugin_messages_decode_from_the_wire() {
        let swap: PluginMessage =
            serde_json::from_str(r#"{"type":"benchSwap","championId":22}"#).unwrap();
        assert_eq!(swap, PluginMessage::BenchSwap { champion_id: 22 });
        let log: PluginMessage =
            serde_json::from_str(r#"{"type":"log","level":"info","message":"m"}"#).unwrap();
        assert_eq!(
            log,
            PluginMessage::Log {
                level: LogLevel::Info,
                message: "m".into()
            }
        );
        assert!(serde_json::from_str::<PluginMessage>(r#"{"type":"benchSwap"}"#).is_err());
        let open: PluginMessage =
            serde_json::from_str(r#"{"type":"openHistory","puuid":"p-1"}"#).unwrap();
        assert_eq!(
            open,
            PluginMessage::OpenHistory {
                puuid: "p-1".into()
            }
        );
    }

    #[tokio::test]
    async fn a_click_in_the_client_asks_for_a_history_and_a_bad_id_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tokio::runtime::Handle::current(),
        );
        let bridge = Bridge::start(service.clone(), "9.9.9").await.unwrap();
        let mut events = service.subscribe();
        let url = format!("ws://127.0.0.1:{}/?token={}", bridge.port(), bridge.token());
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket.next().await.unwrap().unwrap(); // hello
        for puuid in ["../lol-login", "abc-123"] {
            let message = serde_json::json!({"type": "openHistory", "puuid": puuid}).to_string();
            socket.send(Message::text(message)).await.unwrap();
        }
        assert_eq!(
            events.recv().await.unwrap(),
            Event::OpenHistory {
                puuid: "abc-123".into()
            },
            "only the well-formed id comes through"
        );
    }

    #[tokio::test]
    async fn a_plugin_with_the_token_gets_the_state_and_one_without_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tokio::runtime::Handle::current(),
        );
        let bridge = Bridge::start(service.clone(), "9.9.9").await.unwrap();

        let refused = connect_async(format!("ws://127.0.0.1:{}/?token=wrong", bridge.port())).await;
        assert!(refused.is_err(), "a wrong token must not get a socket");

        let url = format!("ws://127.0.0.1:{}/?token={}", bridge.port(), bridge.token());
        let (mut socket, _) = connect_async(url).await.unwrap();
        let first = socket.next().await.unwrap().unwrap();
        let hello: serde_json::Value = serde_json::from_str(first.to_text().unwrap()).unwrap();
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["version"], "9.9.9");
        assert_eq!(hello["snapshot"]["connection"]["status"], "searching");

        let mut settings = service.settings();
        settings.plugin.team_panel = false;
        service.set_settings(settings).unwrap();
        let next = socket.next().await.unwrap().unwrap();
        let event: serde_json::Value = serde_json::from_str(next.to_text().unwrap()).unwrap();
        assert_eq!(event["event"]["type"], "settings");
        assert_eq!(event["event"]["data"]["plugin"]["teamPanel"], false);
        assert_eq!(bridge.connected(), 1);
    }

    #[tokio::test]
    async fn a_plugins_hello_tells_the_service_the_client_page_is_up() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tokio::runtime::Handle::current(),
        );
        let bridge = Bridge::start(service.clone(), "9.9.9").await.unwrap();
        let mut hellos = service.plugin_hellos();
        let url = format!("ws://127.0.0.1:{}/?token={}", bridge.port(), bridge.token());
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket.next().await.unwrap().unwrap(); // the bridge's hello
        assert!(
            !hellos.has_changed().unwrap(),
            "connected, but no plugin has said hello"
        );
        let hello = serde_json::json!({"type": "hello", "version": "9.9.9", "context": "c-1"});
        socket.send(Message::text(hello.to_string())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), hellos.changed())
            .await
            .expect("the hello reached the service")
            .unwrap();
    }

    // ---- The history panel in the client ----

    use crate::{
        analysis,
        model::Game,
        rating::Roles,
        test_support::fixture,
        view::{HistorySource, MatchSummary, QueueInfo},
    };
    use serde_json::{Value, json};

    #[test]
    fn history_requests_decode_and_their_answers_name_the_request() {
        let asked: PluginMessage =
            serde_json::from_str(r#"{"type":"history","puuid":"p-1","requestId":7}"#).unwrap();
        assert_eq!(
            asked,
            PluginMessage::History {
                puuid: "p-1".into(),
                request_id: 7
            }
        );
        assert!(
            serde_json::from_str::<PluginMessage>(r#"{"type":"history","puuid":"p-1"}"#).is_err(),
            "an answer needs an id to go under"
        );

        let failed = BridgeMessage::history_result(7, Err(CoreError::NotConnected.into()));
        assert_eq!(
            serde_json::to_value(&failed).unwrap(),
            json!({
                "type": "historyResult",
                "requestId": 7,
                "page": null,
                "error": {"code": "notConnected", "message": "the League client is not connected"},
            })
        );
        let found = BridgeMessage::history_result(
            8,
            Ok(PanelHistory {
                puuid: "p".into(),
                games: Vec::new(),
            }),
        );
        assert_eq!(
            serde_json::to_value(&found).unwrap(),
            json!({"type": "historyResult", "requestId": 8, "page": {"puuid": "p", "games": []}, "error": null})
        );
        let busy = serde_json::to_value(history_busy(9)).unwrap();
        assert_eq!(
            (&busy["requestId"], &busy["error"]["code"], &busy["page"]),
            (&json!(9), &json!("busy"), &Value::Null),
            "one lookup too many is answered at once, as busy"
        );
    }

    #[test]
    fn the_panel_lists_the_newest_games_of_the_first_page_with_their_queues_named() {
        let game: Game = fixture("live/responses/match-history-game-sgp-twin.json");
        let summary = analysis::match_summary(
            "PUUID-0010",
            &game,
            &Roles::new(),
            &analysis::QueueKinds::new(),
        )
        .expect("the player is in the game");
        // Twelve games, newest first, every other one in a queue the catalog does not know.
        let games = (0..12)
            .map(|n| MatchSummary {
                game_id: n,
                queue_id: if n % 2 == 0 { 2400 } else { 0 },
                ..summary.clone()
            })
            .collect();
        let page = MatchPage {
            puuid: "PUUID-0010".into(),
            begin: 0,
            games,
            has_more: true,
            source: HistorySource::Server,
        };
        let data = GameData {
            queues: vec![QueueInfo {
                id: 2400,
                name: "海克斯大乱斗 ".into(),
                game_mode: "KIWI".into(),
                ranked: false,
            }],
            ..GameData::default()
        };

        let panel = panel_history(page.clone(), Some(&data), false);
        assert_eq!(panel.puuid, "PUUID-0010");
        assert_eq!(
            panel
                .games
                .iter()
                .map(|game| game.game_id)
                .collect::<Vec<_>>(),
            (0..i64::from(PANEL_GAMES)).collect::<Vec<_>>(),
            "the newest games, in order, no more than the panel lists"
        );
        let [first, second, ..] = panel.games.as_slice() else {
            panic!("ten games expected, got {}", panel.games.len());
        };
        assert_eq!(first.queue, "海克斯大乱斗", "the catalog's name, trimmed");
        assert_eq!(second.queue, "", "a queue the catalog does not know");
        assert_eq!(
            (first.kills, first.deaths, first.assists),
            (14, 12, 45),
            "the player's own line"
        );
        let line = &summary.line;
        assert_eq!(
            (first.champion_id, first.win, first.remake, first.award),
            (line.champion_id, line.win, line.remake, line.award)
        );
        assert_eq!(
            (first.started_at, first.duration, first.placement),
            (summary.started_at, summary.duration, line.placement)
        );
        let wire = serde_json::to_string(&BridgeMessage::history_result(1, Ok(panel))).unwrap();
        assert!(wire.len() < 4096, "a small answer: {} bytes", wire.len());
        assert!(
            panel_history(page, None, false)
                .games
                .iter()
                .all(|game| game.queue.is_empty()),
            "no catalog, no names"
        );
    }

    #[test]
    fn the_panel_leaves_custom_games_out_while_the_history_page_hides_them() {
        let game: Game = fixture("live/responses/match-history-game-sgp-twin.json");
        let summary = analysis::match_summary(
            "PUUID-0010",
            &game,
            &Roles::new(),
            &analysis::QueueKinds::new(),
        )
        .expect("the player is in the game");
        // Twenty games, newest first: the three newest and every fifth one custom.
        let games = (0..20)
            .map(|n| MatchSummary {
                game_id: n,
                kind: if n < 3 || n % 5 == 0 {
                    GameKind::Custom
                } else {
                    GameKind::Matched
                },
                ..summary.clone()
            })
            .collect();
        let page = MatchPage {
            puuid: "PUUID-0010".into(),
            begin: 0,
            games,
            has_more: true,
            source: HistorySource::Server,
        };
        let ids = |panel: PanelHistory| {
            panel
                .games
                .iter()
                .map(|game| game.game_id)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            ids(panel_history(page.clone(), None, true)),
            [3, 4, 6, 7, 8, 9, 11, 12, 13, 14],
            "the newest ten that are not custom"
        );
        assert_eq!(
            ids(panel_history(page, None, false)),
            (0..i64::from(PANEL_GAMES)).collect::<Vec<_>>(),
            "shown, a custom game is listed as any other"
        );
    }

    #[tokio::test]
    async fn a_history_request_is_answered_under_its_id_and_bad_ids_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tokio::runtime::Handle::current(),
        );
        let bridge = Bridge::start(service, "9.9.9").await.unwrap();
        let url = format!("ws://127.0.0.1:{}/?token={}", bridge.port(), bridge.token());
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket.next().await.unwrap().unwrap(); // hello
        let long = "a".repeat(PUUID_MAX + 1);
        for (id, puuid) in [(1, "abc-123"), (2, "../lol-login"), (3, long.as_str())] {
            let message = json!({"type": "history", "puuid": puuid, "requestId": id}).to_string();
            socket.send(Message::text(message)).await.unwrap();
        }
        let mut answers = Vec::new();
        while answers.len() < 3 {
            let message = socket.next().await.unwrap().unwrap();
            let value: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_eq!(value["type"], "historyResult");
            answers.push((
                value["requestId"].as_u64().unwrap(),
                value["error"]["code"].clone(),
                value["page"].clone(),
            ));
        }
        answers.sort_by_key(|answer| answer.0);
        assert_eq!(
            answers,
            vec![
                (1, json!("notConnected"), Value::Null),
                (2, json!("invalid"), Value::Null),
                (3, json!("invalid"), Value::Null),
            ],
            "no client: said so; a malformed or overlong id: refused before any request"
        );
    }
}
