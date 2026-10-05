use std::{fmt, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Where one running client's LCU listens, and the token that authenticates against it.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub port: u16,
    pub token: String,
    /// The `LeagueClientUx.exe` the credentials were read from.
    pub pid: Option<u32>,
    /// The client's install directory (`…\LeagueClient`), from the process image path.
    pub client_dir: Option<PathBuf>,
}

impl Credentials {
    pub fn new(port: u16, token: impl Into<String>) -> Self {
        Self {
            port,
            token: token.into(),
            pid: None,
            client_dir: None,
        }
    }

    /// The `Authorization` header: HTTP Basic with the fixed user name `riot`.
    pub fn authorization(&self) -> String {
        format!("Basic {}", STANDARD.encode(format!("riot:{}", self.token)))
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .field("pid", &self.pid)
            .field("client_dir", &self.client_dir)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("the League client is not running")]
    NotRunning,
    /// The client runs at a higher integrity level than winer, so neither its command line nor a
    /// usable lockfile can be read. The Tencent client is launched elevated by WeGame.
    #[error("the League client is running but its credentials cannot be read")]
    AccessDenied,
    #[error("could not inspect the League client process: {0}")]
    Io(#[from] std::io::Error),
}

/// Finds the running client and reads its credentials.
///
/// The command line is the source that always works; the lockfile is the fallback for clients
/// that fill it in (the Tencent client leaves it empty). See `docs/platform-notes.md`.
pub fn discover() -> Result<Credentials, DiscoverError> {
    imp::discover()
}

/// `--app-port=<n>` and `--remoting-auth-token=<token>` from a `LeagueClientUx` command line,
/// quoted or not.
pub fn parse_command_line(command_line: &str) -> Option<(u16, String)> {
    let port = flag(command_line, "--app-port=")?.parse().ok()?;
    let token = flag(command_line, "--remoting-auth-token=")?;
    (!token.is_empty()).then(|| (port, token.to_owned()))
}

/// The value of `name` where it starts an argument, so `--app-port=` never matches inside
/// `--riotclient-app-port=`.
fn flag<'a>(command_line: &'a str, name: &str) -> Option<&'a str> {
    let start = command_line.match_indices(name).find_map(|(at, _)| {
        let boundary = command_line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| c == '"' || c.is_whitespace());
        boundary.then_some(at + name.len())
    })?;
    let rest = &command_line[start..];
    let end = rest
        .find(|c: char| c == '"' || c.is_whitespace())
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// A `LeagueClient:pid:port:token:https` lockfile. Riot Client writes the same format for itself,
/// so the name is checked rather than trusted.
pub fn parse_lockfile(contents: &str) -> Option<(u16, String)> {
    let fields: Vec<&str> = contents.trim().split(':').collect();
    let [name, _pid, port, token, protocol] = fields.as_slice() else {
        return None;
    };
    if *name != "LeagueClient" || *protocol != "https" || token.is_empty() {
        return None;
    }
    Some((port.parse().ok()?, (*token).to_owned()))
}

#[cfg(windows)]
mod imp {
    use std::{fs, io::ErrorKind, path::Path};

    use super::{Credentials, DiscoverError, parse_command_line, parse_lockfile};
    use crate::process::{Process, pids_named};

    pub(super) fn discover() -> Result<Credentials, DiscoverError> {
        let mut denied = false;
        for pid in pids_named("LeagueClientUx.exe")? {
            let process = match Process::open(pid) {
                Ok(process) => process,
                Err(error) => {
                    denied |= error.kind() == ErrorKind::PermissionDenied;
                    continue;
                }
            };
            let client_dir = process
                .image_path()
                .and_then(|exe| exe.parent().map(Path::to_path_buf));
            let from_command_line = match process.command_line() {
                Ok(command_line) => parse_command_line(&command_line),
                Err(error) => {
                    denied |= error.kind() == ErrorKind::PermissionDenied;
                    None
                }
            };
            let found = from_command_line.or_else(|| {
                let lockfile = fs::read_to_string(client_dir.as_ref()?.join("lockfile")).ok()?;
                parse_lockfile(&lockfile)
            });
            if let Some((port, token)) = found {
                return Ok(Credentials {
                    port,
                    token,
                    pid: Some(pid),
                    client_dir,
                });
            }
        }
        Err(if denied {
            DiscoverError::AccessDenied
        } else {
            DiscoverError::NotRunning
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Credentials, DiscoverError};

    /// The supported clients are Windows builds; elsewhere there is never a client to find.
    pub(super) fn discover() -> Result<Credentials, DiscoverError> {
        Err(DiscoverError::NotRunning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The measured Tencent shape (16.19): every argument quoted, and two decoy flags whose names
    // end with the real ones.
    const COMMAND_LINE: &str = r#""D:/英雄联盟/LeagueClient/LeagueClientUx.exe" "--riotclient-auth-token=AAAA" "--riotclient-app-port=62087" "--riotclient-tencent" "--region=TENCENT" "--remoting-auth-token=abcdefghijklmnopqrstuv" "--app-port=62194" "--install-directory=d:\鑻遍泟鑱旂洘(26)\LeagueClient""#;

    #[test]
    fn reads_port_and_token_from_a_quoted_command_line() {
        assert_eq!(
            parse_command_line(COMMAND_LINE),
            Some((62194, "abcdefghijklmnopqrstuv".to_owned()))
        );
    }

    #[test]
    fn reads_an_unquoted_command_line() {
        let line =
            "LeagueClientUx.exe --app-port=5000 --remoting-auth-token=tok --riotclient-app-port=1";
        assert_eq!(parse_command_line(line), Some((5000, "tok".to_owned())));
    }

    #[test]
    fn decoy_flags_alone_are_not_credentials() {
        let line = r#""--riotclient-app-port=62087" "--riotclient-auth-token=AAAA""#;
        assert_eq!(parse_command_line(line), None);
    }

    #[test]
    fn rejects_a_missing_or_empty_token_and_a_bad_port() {
        assert_eq!(
            parse_command_line("--app-port=1 --remoting-auth-token="),
            None
        );
        assert_eq!(
            parse_command_line("--app-port=99999 --remoting-auth-token=t"),
            None
        );
        assert_eq!(parse_command_line("--remoting-auth-token=t"), None);
    }

    #[test]
    fn accepts_only_the_league_client_lockfile() {
        assert_eq!(
            parse_lockfile("LeagueClient:123:60002:secret:https\n"),
            Some((60002, "secret".to_owned()))
        );
        assert_eq!(parse_lockfile("Riot Client:123:50000:other:https"), None);
        assert_eq!(parse_lockfile(""), None);
        assert_eq!(parse_lockfile("LeagueClient:123:60002:secret"), None);
    }

    #[test]
    fn authorization_is_basic_riot_token_and_debug_hides_the_token() {
        let credentials = Credentials::new(1, "secret");
        assert_eq!(credentials.authorization(), "Basic cmlvdDpzZWNyZXQ=");
        assert!(!format!("{credentials:?}").contains("secret"));
    }
}
