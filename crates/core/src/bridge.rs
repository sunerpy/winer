//! The loopback socket the in-client plugin talks to.
//!
//! The plugin runs inside the client page and cannot read files, so the desktop app writes the
//! port and a fresh token into `bootstrap.json` beside the plugin's `index.js` (see
//! [`crate::plugin`]). The plugin fetches that sibling file, connects to
//! `ws://127.0.0.1:<port>/?token=<token>` and from then on receives the same [`Event`]s as the
//! window. A connection with any other token is refused during the HTTP upgrade.

use std::{
    net::Ipv4Addr,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use futures_util::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast::error::RecvError,
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
    Service,
    settings::Settings,
    view::{Event, Snapshot},
};

/// Core → plugin. Built, serialized and dropped at once, so the variants' sizes do not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
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
                message = incoming.next() => match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<PluginMessage>(&text) {
                        Ok(PluginMessage::Hello { version, context: id }) => {
                            info!(%version, context = %id, "plugin connected");
                            context = id;
                        }
                        Ok(PluginMessage::Log { level, message }) => log(&context, level, &message),
                        Ok(PluginMessage::BenchSwap { champion_id }) => swap(&service, champion_id),
                        Ok(PluginMessage::OpenHistory { puuid }) => {
                            if let Err(error) = service.open_history(&puuid) {
                                warn!(%error, "history asked for from the client not opened");
                            }
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
}
