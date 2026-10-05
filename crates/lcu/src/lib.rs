//! The League Client Update (LCU) API, reached from outside the client.
//!
//! [`discover`] finds the running client and its credentials, [`Lcu`] issues authenticated REST
//! calls and [`subscribe`] opens the WAMP event socket. Both pin TLS to Riot's own root
//! certificate; nothing here trusts the system store.

mod credentials;
mod error;
mod events;
mod http;
#[cfg(windows)]
mod process;
mod tls;

pub use credentials::{Credentials, DiscoverError, discover, parse_command_line, parse_lockfile};
pub use error::Error;
pub use events::{Event, EventKind, EventStream, subscribe, subscription_name};
pub use http::Lcu;
pub use reqwest::Method;
