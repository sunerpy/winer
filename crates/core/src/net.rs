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
