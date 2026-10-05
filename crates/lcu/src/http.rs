use std::{sync::Arc, time::Duration};

use reqwest::{
    Client, Method, StatusCode,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{Credentials, Error, tls};

/// An authenticated LCU REST client. Clones share one connection pool.
#[derive(Clone)]
pub struct Lcu {
    inner: Arc<Inner>,
}

struct Inner {
    client: Client,
    base: String,
    port: u16,
}

impl Lcu {
    pub fn new(credentials: &Credentials) -> Result<Self, reqwest::Error> {
        let mut authorization = HeaderValue::from_str(&credentials.authorization())
            .expect("base64 is a valid header value");
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .tls_backend_preconfigured((*tls::config()).clone())
            .default_headers(headers)
            // The LCU is loopback; a system or environment proxy must never see it.
            .no_proxy()
            .connect_timeout(Duration::from_secs(3))
            // Match history for a busy account is the slowest call the app makes.
            .timeout(Duration::from_secs(20))
            .build()?;
        let base = format!("https://127.0.0.1:{}", credentials.port);
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                base,
                port: credentials.port,
            }),
        })
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let body = self.send(Method::GET, path, None).await?;
        decode(&Method::GET, path, &body)
    }

    /// A GET where 404 means "absent": the LCU answers 404 for state that does not exist right
    /// now, such as the lobby while there is none.
    pub async fn get_optional<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, Error> {
        match self.get(path).await {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.is_not_found() => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Sends `body` as JSON and decodes the answer. An empty answer decodes as `null`, so `()`
    /// and `Option<_>` both accept a 204.
    pub async fn call<B, T>(&self, method: Method, path: &str, body: &B) -> Result<T, Error>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let json = serde_json::to_vec(body).expect("request bodies serialize");
        let answer = self.send(method.clone(), path, Some(json)).await?;
        decode(&method, path, &answer)
    }

    pub async fn post<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<(), Error> {
        self.call::<B, serde::de::IgnoredAny>(Method::POST, path, body)
            .await
            .map(drop)
    }

    pub async fn put<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<(), Error> {
        self.call::<B, serde::de::IgnoredAny>(Method::PUT, path, body)
            .await
            .map(drop)
    }

    pub async fn patch<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<(), Error> {
        self.call::<B, serde::de::IgnoredAny>(Method::PATCH, path, body)
            .await
            .map(drop)
    }

    pub async fn delete(&self, path: &str) -> Result<(), Error> {
        self.send(Method::DELETE, path, None).await.map(drop)
    }

    /// The raw body and its content type, for game-data assets (icons, splash art).
    pub async fn bytes(&self, path: &str) -> Result<(Option<String>, Vec<u8>), Error> {
        let response = self.response(Method::GET, path, None).await?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .await
            .map_err(|source| transport(Method::GET, path, source))?;
        Ok((content_type, body.to_vec()))
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, Error> {
        let response = self.response(method.clone(), path, body).await?;
        let body = response
            .bytes()
            .await
            .map_err(|source| transport(method, path, source))?;
        Ok(body.to_vec())
    }

    async fn response(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<reqwest::Response, Error> {
        debug_assert!(path.starts_with('/'), "LCU paths are absolute: {path}");
        let mut request = self
            .inner
            .client
            .request(method.clone(), format!("{}{path}", self.inner.base));
        if let Some(body) = body {
            request = request.header(CONTENT_TYPE, "application/json").body(body);
        }
        let response = request
            .send()
            .await
            .map_err(|source| transport(method.clone(), path, source))?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.bytes().await.unwrap_or_default();
        Err(Error::Status {
            method,
            path: path.to_owned(),
            status: status.as_u16(),
            message: error_message(status, &body),
        })
    }
}

fn transport(method: Method, path: &str, source: reqwest::Error) -> Error {
    // reqwest puts the URL in its message; the path is already in ours.
    Error::Transport {
        method,
        path: path.to_owned(),
        source: source.without_url(),
    }
}

fn decode<T: DeserializeOwned>(method: &Method, path: &str, body: &[u8]) -> Result<T, Error> {
    let body = if body.is_empty() {
        b"null".as_slice()
    } else {
        body
    };
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| Error::Decode {
        method: method.clone(),
        path: path.to_owned(),
        field: error.path().to_string(),
        message: error.into_inner().to_string(),
    })
}

/// The LCU's own `message` when the body is its error document, else the status reason.
fn error_message(status: StatusCode, body: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("message")?.as_str().map(str::to_owned))
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| status.canonical_reason().unwrap_or("error").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_body_decodes_as_null() {
        let unit: () = decode(&Method::POST, "/x", b"").unwrap();
        let none: Option<u32> = decode(&Method::POST, "/x", b"").unwrap();
        assert_eq!((unit, none), ((), None));
    }

    #[test]
    fn a_decode_error_names_the_field() {
        #[derive(serde::Deserialize, Debug)]
        #[allow(dead_code)]
        struct Shape {
            games: Vec<u32>,
        }
        let error = decode::<Shape>(&Method::GET, "/games", br#"{"games":[1,"two"]}"#).unwrap_err();
        assert!(
            matches!(&error, Error::Decode { field, .. } if field == "games[1]"),
            "{error}"
        );
    }

    #[test]
    fn error_messages_prefer_the_lcu_text() {
        let body = br#"{"errorCode":"RPC_ERROR","httpStatus":404,"message":"No active delegate"}"#;
        assert_eq!(
            error_message(StatusCode::NOT_FOUND, body),
            "No active delegate"
        );
        assert_eq!(error_message(StatusCode::NOT_FOUND, b"<html>"), "Not Found");
    }
}
