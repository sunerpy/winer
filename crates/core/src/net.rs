//! The public internet, for the two things winer reads from it: what augments do (ARAM.GG) and match
//! history from the shard's own server (SGP). Nothing of the LCU's pinned trust applies here.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rustls_platform_verifier::BuilderVerifierExt as _;

/// One client for the whole run, built on first use, so pages of history share a connection. It
/// trusts the system's own store, so a corporate root the user installed is honoured. It connects
/// directly, as the client itself does: the shard's server is in the player's own region, and
/// the system proxy the updater reads is no business of these requests. Each request sets its own
/// overall timeout.
pub fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT.get_or_init(build).as_ref().map_err(Clone::clone)
}

fn build() -> Result<reqwest::Client, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(|builder| builder.with_platform_verifier())
        .map_err(|error| error.to_string())?
        .with_no_client_auth();
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
    };

    /// `{"ok":true}`, gzipped.
    const GZIPPED: [u8; 31] = [
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 202, 207, 86, 178, 42, 41, 42, 77, 173, 5, 0,
        144, 95, 212, 167, 11, 0, 0, 0,
    ];

    /// The shard's server compresses when asked: the client asks, and reads the answer unpacked.
    #[tokio::test]
    async fn answers_are_asked_for_compressed_and_read_unpacked() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]).to_ascii_lowercase();
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-encoding: gzip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                GZIPPED.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&GZIPPED).unwrap();
            request
        });
        let body = super::client()
            .unwrap()
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, r#"{"ok":true}"#);
        let request = server.join().unwrap();
        assert!(
            request
                .lines()
                .any(|line| line.starts_with("accept-encoding:") && line.contains("gzip")),
            "{request}"
        );
    }
}
