use reqwest::Method;

/// A failed LCU call, naming the request it belongs to.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{method} {path}: {source}")]
    Transport {
        method: Method,
        path: String,
        #[source]
        source: reqwest::Error,
    },
    /// The LCU answered with a non-success status. `message` is the LCU's own error text when it
    /// sent one (`{"errorCode": …, "message": …}`), otherwise the status reason.
    #[error("{method} {path}: HTTP {status}: {message}")]
    Status {
        method: Method,
        path: String,
        status: u16,
        message: String,
    },
    #[error("{method} {path}: unexpected body at `{field}`: {message}")]
    Decode {
        method: Method,
        path: String,
        field: String,
        message: String,
    },
    #[error("event socket: {0}")]
    Socket(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("event socket: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Status { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// The client is gone or not yet listening: worth retrying after rediscovery, not reporting.
    pub fn is_unreachable(&self) -> bool {
        match self {
            Self::Transport { source, .. } => source.is_connect() || source.is_timeout(),
            Self::Socket(_) | Self::Io(_) => true,
            _ => false,
        }
    }
}
