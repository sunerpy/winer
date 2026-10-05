//! winer's domain, independent of any window: the connection to the League client, the live
//! views built from it, the automation acting on it, and the bridge to the in-client plugin.

pub mod analysis;
pub mod augments;
pub mod automation;
pub mod bindings;
pub mod bridge;
pub mod callout;
pub mod catalog;
pub mod live;
pub mod model;
pub mod net;
pub mod plugin;
pub mod rating;
pub mod service;
pub mod settings;
pub mod sgp;
pub mod view;

pub use service::{Asset, CoreError, Service};
pub use settings::Settings;

#[cfg(test)]
pub(crate) mod test_support {
    use std::{fs, path::Path};

    use serde::de::DeserializeOwned;
    use serde_json::Value;

    /// A file under the repository's `fixtures/`. Captured LCU fixtures wrap the response in
    /// `{ header, body }`; the body is what the client sent.
    pub(crate) fn fixture<T: DeserializeOwned>(relative: &str) -> T {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(relative);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut value: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{relative}: {error}"));
        if let Some(body) = value
            .as_object_mut()
            .filter(|object| object.contains_key("header"))
            .and_then(|object| object.remove("body"))
        {
            value = body;
        }
        serde_json::from_value(value).unwrap_or_else(|error| panic!("{relative}: {error}"))
    }
}
