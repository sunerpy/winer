use std::net::{IpAddr, Ipv4Addr};

use futures_util::{SinkExt as _, StreamExt as _};
use rustls::pki_types::ServerName;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{Message, client::IntoClientRequest as _, http::HeaderValue},
};

use crate::{Credentials, Error, tls};

/// One LCU JSON API event.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub uri: String,
    pub kind: EventKind,
    /// The resource after the change; `null` on `Delete`.
    pub data: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum EventKind {
    Create,
    Update,
    Delete,
}

/// The WAMP topic for one endpoint: `OnJsonApiEvent` plus the URI with every `/` as `_`.
pub fn subscription_name(uri: &str) -> String {
    format!("OnJsonApiEvent{}", uri.replace('/', "_"))
}

pub struct EventStream {
    socket: WebSocketStream<TlsStream<TcpStream>>,
}

/// Opens the event socket and subscribes to each of `uris` by name.
///
/// Only fine-grained topics are used. Adding the global `OnJsonApiEvent` next to them would make
/// the LCU deliver every matching event twice, and it cannot be narrowed afterwards.
pub async fn subscribe(credentials: &Credentials, uris: &[&str]) -> Result<EventStream, Error> {
    let tcp = TcpStream::connect((Ipv4Addr::LOCALHOST, credentials.port)).await?;
    let server_name = ServerName::IpAddress(IpAddr::V4(Ipv4Addr::LOCALHOST).into());
    let tls = TlsConnector::from(tls::config())
        .connect(server_name, tcp)
        .await?;

    let mut request = format!("wss://127.0.0.1:{}/", credentials.port)
        .into_client_request()
        .map_err(Box::new)?;
    let headers = request.headers_mut();
    let authorization = HeaderValue::from_str(&credentials.authorization())
        .expect("base64 is a valid header value");
    headers.insert("Authorization", authorization);
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_static("wamp"));
    let (mut socket, _) = client_async(request, tls).await.map_err(Box::new)?;

    for uri in uris {
        let frame = json!([5, subscription_name(uri)]).to_string();
        socket.send(Message::text(frame)).await.map_err(Box::new)?;
    }
    Ok(EventStream { socket })
}

impl EventStream {
    /// The next event, or `None` once the client has closed the socket.
    pub async fn next(&mut self) -> Option<Result<Event, Error>> {
        while let Some(message) = self.socket.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Some(event) = parse_frame(&text) {
                        return Some(Ok(event));
                    }
                }
                Ok(Message::Close(_)) => return None,
                Ok(_) => {}
                Err(error) => return Some(Err(Box::new(error).into())),
            }
        }
        None
    }
}

/// `[8, "<topic>", {"uri", "eventType", "data"}]`. Every other frame, the welcome included, is not
/// an event.
fn parse_frame(text: &str) -> Option<Event> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Envelope {
        uri: String,
        event_type: EventKind,
        #[serde(default)]
        data: Value,
    }

    let (opcode, _topic, envelope): (u8, String, Envelope) = serde_json::from_str(text).ok()?;
    (opcode == 8).then_some(Event {
        uri: envelope.uri,
        kind: envelope.event_type,
        data: envelope.data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_names_follow_the_per_endpoint_rule() {
        assert_eq!(
            subscription_name("/lol-gameflow/v1/gameflow-phase"),
            "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"
        );
    }

    #[test]
    fn parses_event_frames_and_ignores_the_rest() {
        let frame = r#"[8,"OnJsonApiEvent_lol-gameflow_v1_gameflow-phase",{"data":"ChampSelect","eventType":"Update","uri":"/lol-gameflow/v1/gameflow-phase"}]"#;
        assert_eq!(
            parse_frame(frame),
            Some(Event {
                uri: "/lol-gameflow/v1/gameflow-phase".into(),
                kind: EventKind::Update,
                data: json!("ChampSelect")
            })
        );
        let deleted = r#"[8,"t",{"eventType":"Delete","uri":"/lol-lobby/v2/lobby"}]"#;
        assert_eq!(
            parse_frame(deleted).map(|event| (event.kind, event.data)),
            Some((EventKind::Delete, Value::Null))
        );
        assert_eq!(parse_frame(r#"[0,"session",1,"server"]"#), None);
        assert_eq!(parse_frame("not json"), None);
    }
}
