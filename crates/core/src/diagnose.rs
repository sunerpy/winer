//! The self-check: whether what winer relies on still answers, run when the client comes back on a
//! new version and whenever the user asks. The core checks the client and the internet; the shell
//! adds what it holds itself (the loader and plugin, the shortcut, updates) and puts the report
//! together. Nothing here writes to the client: a route that changes something is only looked up
//! by name in the client's own list.

use std::{
    collections::HashSet,
    fs, io,
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    settings::write_atomic,
    view::{Check, CheckFailure, CheckId, CheckReason, CheckStatus},
};

/// How long one check may take before it counts as failed.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Verb {
    fn pascal(self) -> &'static str {
        match self {
            Self::Get => "Get",
            Self::Post => "Post",
            Self::Put => "Put",
            Self::Patch => "Patch",
            Self::Delete => "Delete",
        }
    }

    fn upper(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Every client route winer calls, with its parameters named as the client names them. Pictures
/// (`/lol-game-data/assets/…`) are files, not routes, and are left out.
pub const LCU_ROUTES: &[(Verb, &str)] = &[
    (Verb::Get, "/entitlements/v1/token"),
    (Verb::Get, "/lol-challenges/v1/challenges/local-player"),
    (
        Verb::Get,
        "/lol-challenges/v1/summary-player-data/local-player",
    ),
    (Verb::Post, "/lol-challenges/v1/update-player-preferences"),
    (Verb::Get, "/lol-challenges/v2/titles/local-player"),
    (Verb::Get, "/lol-champ-select/v1/bannable-champion-ids"),
    (Verb::Get, "/lol-champ-select/v1/pickable-champion-ids"),
    (Verb::Get, "/lol-champ-select/v1/session"),
    (Verb::Patch, "/lol-champ-select/v1/session/actions/{id}"),
    (
        Verb::Post,
        "/lol-champ-select/v1/session/actions/{id}/complete",
    ),
    (
        Verb::Post,
        "/lol-champ-select/v1/session/bench/swap/{championId}",
    ),
    (Verb::Patch, "/lol-champ-select/v1/session/my-selection"),
    (
        Verb::Post,
        "/lol-champ-select/v1/session/my-selection/reroll",
    ),
    (
        Verb::Get,
        "/lol-champions/v1/inventories/{summonerId}/skins-minimal",
    ),
    (Verb::Get, "/lol-chat/v1/conversations"),
    (Verb::Post, "/lol-chat/v1/conversations/{id}/messages"),
    (Verb::Get, "/lol-chat/v1/friends"),
    (Verb::Get, "/lol-chat/v1/me"),
    (Verb::Put, "/lol-chat/v1/me"),
    (Verb::Get, "/lol-game-queues/v1/queues"),
    (Verb::Get, "/lol-game-settings/v1/game-settings"),
    (Verb::Patch, "/lol-game-settings/v1/game-settings"),
    (Verb::Get, "/lol-game-settings/v1/input-settings"),
    (Verb::Patch, "/lol-game-settings/v1/input-settings"),
    (Verb::Get, "/lol-game-settings/v1/ready"),
    (Verb::Post, "/lol-game-settings/v1/save"),
    (Verb::Get, "/lol-gameflow/v1/gameflow-phase"),
    (Verb::Get, "/lol-gameflow/v1/session"),
    (Verb::Get, "/lol-item-sets/v1/item-sets/{summonerId}/sets"),
    (Verb::Put, "/lol-item-sets/v1/item-sets/{summonerId}/sets"),
    (Verb::Get, "/lol-lobby/v2/lobby"),
    (Verb::Post, "/lol-lobby/v2/play-again"),
    (Verb::Get, "/lol-match-history/v1/games/{gameId}"),
    (
        Verb::Get,
        "/lol-match-history/v1/products/lol/{puuid}/matches",
    ),
    (
        Verb::Get,
        "/lol-match-history/v1/products/lol/current-summoner/matches",
    ),
    (Verb::Get, "/lol-matchmaking/v1/ready-check"),
    (Verb::Post, "/lol-matchmaking/v1/ready-check/accept"),
    (Verb::Get, "/lol-patch/v1/game-version"),
    (Verb::Get, "/lol-perks/v1/currentpage"),
    (Verb::Put, "/lol-perks/v1/currentpage"),
    (Verb::Get, "/lol-perks/v1/inventory"),
    (Verb::Get, "/lol-perks/v1/pages"),
    (Verb::Post, "/lol-perks/v1/pages"),
    (Verb::Put, "/lol-perks/v1/pages/{id}"),
    (
        Verb::Get,
        "/lol-perks/v1/recommended-pages/champion/{championId}/position/{position}/map/{mapId}",
    ),
    (Verb::Get, "/lol-ranked/v1/ranked-stats/{puuid}"),
    (Verb::Get, "/lol-regalia/v2/current-summoner/regalia"),
    (Verb::Put, "/lol-regalia/v2/current-summoner/regalia"),
    (Verb::Get, "/lol-regalia/v3/inventory/{inventoryType}"),
    (Verb::Get, "/lol-summoner/v1/current-summoner"),
    (
        Verb::Get,
        "/lol-summoner/v1/current-summoner/summoner-profile",
    ),
    (
        Verb::Post,
        "/lol-summoner/v1/current-summoner/summoner-profile",
    ),
    (Verb::Get, "/lol-summoner/v1/summoners"),
    (Verb::Get, "/lol-summoner/v2/summoners/puuid/{puuid}"),
    (Verb::Post, "/riotclient/kill-and-restart-ux"),
    (Verb::Post, "/riotclient/ux-show"),
];

/// A route the client always has while it answers at all. A list without it is not the list it
/// seems, and is not checked against.
const ANCHOR: (Verb, &str) = (Verb::Get, "/lol-gameflow/v1/gameflow-phase");

/// The name the client's `/help` lists a route under: the method, then every segment of the path
/// in Pascal case, a parameter as `By` and its name.
/// `PATCH /lol-champ-select/v1/session/actions/{id}` is `PatchLolChampSelectV1SessionActionsById`.
pub fn function_name(verb: Verb, template: &str) -> String {
    let mut name = verb.pascal().to_owned();
    let path = template.split('?').next().unwrap_or(template);
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        match segment
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        {
            Some(parameter) => {
                name.push_str("By");
                push_capitalized(&mut name, parameter);
            }
            None => {
                for word in segment.split(['-', '_']).filter(|word| !word.is_empty()) {
                    push_capitalized(&mut name, word);
                }
            }
        }
    }
    name
}

fn push_capitalized(name: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        name.extend(first.to_uppercase());
        name.push_str(chars.as_str());
    }
}

/// The function names in a `/help` answer: the keys of `functions` when it is an object, the
/// `name` of each entry when it is a list. `None` when the answer has neither.
pub fn help_functions(body: &Value) -> Option<HashSet<String>> {
    match body.get("functions")? {
        Value::Object(functions) => Some(functions.keys().cloned().collect()),
        Value::Array(functions) => Some(
            functions
                .iter()
                .filter_map(|function| function.get("name")?.as_str().map(str::to_owned))
                .collect(),
        ),
        _ => None,
    }
}

/// The routes check, from the names `/help` listed (`None`: no list to check against).
pub fn routes_check(
    functions: Option<&HashSet<String>>,
) -> (CheckStatus, CheckReason, Option<String>) {
    let anchor = function_name(ANCHOR.0, ANCHOR.1);
    let Some(functions) = functions.filter(|functions| functions.contains(&anchor)) else {
        return (CheckStatus::Unknown, CheckReason::ListUnavailable, None);
    };
    let missing: Vec<String> = LCU_ROUTES
        .iter()
        .filter(|(verb, template)| !functions.contains(&function_name(*verb, template)))
        .map(|(verb, template)| format!("{} {template}", verb.upper()))
        .collect();
    if missing.is_empty() {
        (CheckStatus::Ok, CheckReason::Fine, None)
    } else {
        (
            CheckStatus::Fail,
            CheckReason::RoutesMissing,
            Some(missing.join("\n")),
        )
    }
}

/// A check's outcome before it is timed.
pub(crate) struct Outcome {
    pub status: CheckStatus,
    pub reason: CheckReason,
    pub detail: Option<String>,
    pub failure: Option<CheckFailure>,
}

impl Outcome {
    pub(crate) fn new(status: CheckStatus, reason: CheckReason) -> Self {
        Self {
            status,
            reason,
            detail: None,
            failure: None,
        }
    }

    pub(crate) fn fine() -> Self {
        Self::new(CheckStatus::Ok, CheckReason::Fine)
    }

    pub(crate) fn skipped(reason: CheckReason) -> Self {
        Self::new(CheckStatus::Skipped, reason)
    }

    pub(crate) fn failed(failure: CheckFailure) -> Self {
        Self {
            failure: Some(failure),
            ..Self::new(CheckStatus::Fail, CheckReason::Unreachable)
        }
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Runs one check within [`CHECK_TIMEOUT`]; running out of time counts as a failed request.
pub(crate) async fn timed(id: CheckId, check: impl Future<Output = Outcome>) -> Check {
    let started = Instant::now();
    let outcome = tokio::time::timeout(CHECK_TIMEOUT, check)
        .await
        .unwrap_or_else(|_| Outcome::failed(CheckFailure::Timeout));
    Check {
        id,
        status: outcome.status,
        reason: outcome.reason,
        detail: outcome.detail,
        failure: outcome.failure,
        took_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
    }
}

/// How a request to the internet failed.
pub fn failure_of(error: &reqwest::Error) -> CheckFailure {
    if error.is_timeout() {
        CheckFailure::Timeout
    } else if error.is_connect() {
        CheckFailure::Connect
    } else if let Some(status) = error.status() {
        CheckFailure::Status {
            code: status.as_u16(),
        }
    } else if error.is_decode() || error.is_body() {
        CheckFailure::Decode
    } else {
        CheckFailure::Other
    }
}

/// How a request to the client failed.
pub fn lcu_failure(error: &lcu::Error) -> CheckFailure {
    match error {
        lcu::Error::Transport { source, .. } => failure_of(source),
        lcu::Error::Status { status, .. } => CheckFailure::Status { code: *status },
        lcu::Error::Decode { .. } => CheckFailure::Decode,
        lcu::Error::Socket(_) | lcu::Error::Io(_) => CheckFailure::Connect,
    }
}

/// The client version the last connection saw, kept beside the settings so that a version
/// change is noticed across runs.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct Seen {
    client_version: Option<String>,
}

pub(crate) struct VersionStore {
    path: PathBuf,
}

impl VersionStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Keeps `version` as the one seen. Returns it when another was seen before, which is when a
    /// self-check is due; the first version ever seen is only kept. A file that cannot be read
    /// counts as no version seen.
    pub(crate) fn note(&self, version: &str) -> io::Result<Option<String>> {
        let before = fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Seen>(&bytes).ok())
            .and_then(|seen| seen.client_version);
        if before.as_deref() == Some(version) {
            return Ok(None);
        }
        let seen = Seen {
            client_version: Some(version.to_owned()),
        };
        write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(&seen).expect("the seen version serializes"),
        )?;
        Ok(before.map(|_| version.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn route_names_follow_the_clients_own() {
        assert_eq!(
            function_name(Verb::Get, "/lol-gameflow/v1/gameflow-phase"),
            "GetLolGameflowV1GameflowPhase"
        );
        assert_eq!(
            function_name(Verb::Patch, "/lol-champ-select/v1/session/actions/{id}"),
            "PatchLolChampSelectV1SessionActionsById"
        );
        assert_eq!(
            function_name(
                Verb::Post,
                "/lol-champ-select/v1/session/bench/swap/{championId}"
            ),
            "PostLolChampSelectV1SessionBenchSwapByChampionId"
        );
        assert_eq!(
            function_name(Verb::Post, "/riotclient/kill-and-restart-ux"),
            "PostRiotclientKillAndRestartUx"
        );
        assert_eq!(
            function_name(Verb::Get, "/lol-summoner/v1/summoners?name=x"),
            "GetLolSummonerV1Summoners",
            "a query is not part of the name"
        );
        assert_eq!(
            function_name(Verb::Get, "/lol-regalia/v3/inventory/{inventoryType}"),
            "GetLolRegaliaV3InventoryByInventoryType"
        );
    }

    #[test]
    fn help_lists_its_functions_as_an_object_or_a_list() {
        let object = json!({"functions": {"GetLolGameflowV1GameflowPhase": "", "Help": ""}});
        assert_eq!(help_functions(&object).map(|names| names.len()), Some(2));
        let list = json!({"functions": [{"name": "GetLolGameflowV1GameflowPhase"}, {"x": 1}]});
        assert_eq!(
            help_functions(&list),
            Some(HashSet::from(["GetLolGameflowV1GameflowPhase".to_owned()]))
        );
        assert_eq!(help_functions(&json!({"events": {}})), None);
        assert_eq!(help_functions(&json!("text")), None);
    }

    #[test]
    fn routes_are_checked_only_against_a_list_that_has_the_anchor() {
        let all: HashSet<String> = LCU_ROUTES
            .iter()
            .map(|(verb, template)| function_name(*verb, template))
            .collect();
        assert_eq!(
            routes_check(Some(&all)),
            (CheckStatus::Ok, CheckReason::Fine, None)
        );

        let mut short = all.clone();
        short.remove("PatchLolChampSelectV1SessionActionsById");
        short.remove("PostRiotclientUxShow");
        let (status, reason, detail) = routes_check(Some(&short));
        assert_eq!(
            (status, reason),
            (CheckStatus::Fail, CheckReason::RoutesMissing)
        );
        assert_eq!(
            detail.as_deref(),
            Some("PATCH /lol-champ-select/v1/session/actions/{id}\nPOST /riotclient/ux-show")
        );

        let mut headless = all;
        headless.remove("GetLolGameflowV1GameflowPhase");
        assert_eq!(
            routes_check(Some(&headless)).0,
            CheckStatus::Unknown,
            "without the anchor the list is not trusted, and nothing is reported missing"
        );
        assert_eq!(
            routes_check(None),
            (CheckStatus::Unknown, CheckReason::ListUnavailable, None)
        );
    }

    #[test]
    fn a_version_change_is_noticed_once_and_the_first_version_only_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = VersionStore::new(dir.path().join("diagnostics.json"));
        assert_eq!(store.note("16.19.1").unwrap(), None, "the first ever");
        assert_eq!(store.note("16.19.1").unwrap(), None, "the same again");
        assert_eq!(store.note("16.20.1").unwrap().as_deref(), Some("16.20.1"));
        assert_eq!(store.note("16.20.1").unwrap(), None);
        let again = VersionStore::new(dir.path().join("diagnostics.json"));
        assert_eq!(again.note("16.20.1").unwrap(), None, "kept across runs");

        fs::write(again.path(), b"{ not json").unwrap();
        assert_eq!(
            again.note("16.21.1").unwrap(),
            None,
            "an unreadable file is no version seen"
        );
        assert_eq!(again.note("16.22.1").unwrap().as_deref(), Some("16.22.1"));
    }
}
