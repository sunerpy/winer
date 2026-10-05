//! Smoke test against a running client: discovery, REST and event delivery.
//!
//! `cargo run -p lcu --example probe` on the machine running the League client. With `--toggle`
//! it flips the chat availability to `away` and back so an event is guaranteed to arrive.

use std::time::Duration;

use serde_json::{Value, json};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = lcu::discover()?;
    println!(
        "client pid={:?} port={} dir={:?}",
        credentials.pid, credentials.port, credentials.client_dir
    );

    let client = lcu::Lcu::new(&credentials)?;
    let summoner: Value = client.get("/lol-summoner/v1/current-summoner").await?;
    let phase: String = client.get("/lol-gameflow/v1/gameflow-phase").await?;
    println!("rest ok: level={} phase={phase}", summoner["summonerLevel"]);
    let lobby: Option<Value> = client.get_optional("/lol-lobby/v2/lobby").await?;
    println!("lobby present={}", lobby.is_some());

    let uris = [
        "/lol-gameflow/v1/gameflow-phase",
        "/lol-chat/v1/me",
        "/lol-champ-select/v1/session",
    ];
    let mut events = lcu::subscribe(&credentials, &uris).await?;
    println!("subscribed to {} topics", uris.len());

    if std::env::args().any(|arg| arg == "--toggle") {
        let me: Value = client.get("/lol-chat/v1/me").await?;
        let original = me["availability"].as_str().unwrap_or("chat").to_owned();
        let other = if original == "away" { "chat" } else { "away" };
        client
            .put("/lol-chat/v1/me", &json!({ "availability": other }))
            .await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        client
            .put("/lol-chat/v1/me", &json!({ "availability": original }))
            .await?;
        println!("toggled availability {other} -> {original}");
    }

    let listen = async {
        while let Some(event) = events.next().await {
            match event {
                Ok(event) => println!(
                    "event {:?} {} availability={}",
                    event.kind,
                    event.uri,
                    event.data.get("availability").unwrap_or(&Value::Null)
                ),
                Err(error) => println!("event error: {error}"),
            }
        }
        println!("socket closed");
    };
    let _ = tokio::time::timeout(Duration::from_secs(4), listen).await;
    Ok(())
}
