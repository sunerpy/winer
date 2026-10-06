//! What the service keeps in memory, kept bounded: every cache has a hard cap, a sweep every few
//! minutes lets go of what has expired, and 设置 › 关于's cleanup lets go of everything that is not
//! on screen or on its way. The caps sit with each cache: the pictures' here ([`ASSETS`]), the
//! players' records' here ([`PLAYERS`]), the history's in `history.rs`, the builds' in
//! `service/loadout.rs`.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use tokio::time::sleep;
use tracing::debug;

use super::{Client, FAILED_PLAYER_TTL, PLAYER_TTL, PlayerEntry, Service, lock};
use crate::{cache::Limits, live, view::MemoryUse};

/// Between two sweeps.
const SWEEP_EVERY: Duration = Duration::from_secs(10 * 60);

/// Icons and pictures the window has drawn: 32 MB at most, a thousand champion icons (28 KB each on
/// 16.19) or eight hundred skin tiles (40 KB; the background picker lists 2,635), and one not drawn
/// for half an hour goes at the next sweep.
pub(super) const ASSETS: Limits = Limits {
    entries: 2048,
    bytes: 32 * 1024 * 1024,
    idle: Duration::from_secs(30 * 60),
};

/// Players' records kept at most (a few tens of kilobytes each), besides those being fetched or on
/// screen: champ select, the game and the lobby show twenty at a time.
pub(super) const PLAYERS: usize = 200;

/// Augment descriptions are read again after this long, so a new patch's arrive the same day.
pub(super) const AUGMENT_DETAILS_TTL: Duration = Duration::from_secs(12 * 60 * 60);

impl Service {
    /// Sweeps the caches every [`SWEEP_EVERY`] on the service's runtime, for as long as it runs.
    pub(super) fn start_sweeping(&self) {
        let service = self.clone();
        self.spawn(async move {
            loop {
                sleep(SWEEP_EVERY).await;
                let went = service.sweep_caches(Instant::now());
                if went.entries > 0 {
                    debug!(
                        entries = went.entries,
                        bytes = went.image_bytes,
                        "caches swept"
                    );
                }
            }
        });
    }

    /// The players the client's views show now: champ select, the game, the lobby and the local
    /// player. Their records stay whatever their age; nothing would fetch them again before the
    /// views move on.
    pub(super) fn on_screen(&self) -> HashSet<String> {
        let Ok(client) = self.client() else {
            return HashSet::new();
        };
        players_shown(&client)
    }

    /// Lets go of what has expired in every cache. Returns what went.
    pub fn sweep_caches(&self, now: Instant) -> MemoryUse {
        let shown = self.on_screen();
        let (assets, image_bytes) = {
            let mut assets = lock(&self.inner.assets);
            let before = assets.bytes();
            (assets.sweep(now), before - assets.bytes())
        };
        let players = {
            let mut players = lock(&self.inner.players);
            let before = players.len();
            players.retain(|puuid, entry| shown.contains(puuid) || !expired(entry, now));
            before - players.len()
        };
        let history = lock(&self.inner.history).sweep(now);
        let builds = self.inner.loadout.sweep(now);
        let augments = self
            .inner
            .augment_details
            .try_lock()
            .map_or(0, |mut cache| {
                let before = cache.len();
                cache.retain(|_, (at, _)| now.saturating_duration_since(*at) < AUGMENT_DETAILS_TTL);
                before - cache.len()
            });
        MemoryUse {
            entries: count(assets + players + history + builds + augments),
            image_bytes: image_bytes as u64,
        }
    }

    /// Lets go of everything cached that is neither on screen nor being fetched: the next view asks
    /// the client or the internet again. Returns what went.
    pub fn clear_caches(&self) -> MemoryUse {
        let shown = self.on_screen();
        let (assets, image_bytes) = lock(&self.inner.assets).clear();
        let players = {
            let mut players = lock(&self.inner.players);
            let before = players.len();
            players.retain(|puuid, entry| {
                shown.contains(puuid) || matches!(entry, PlayerEntry::Loading)
            });
            before - players.len()
        };
        let history = lock(&self.inner.history).clear();
        let builds = self.inner.loadout.clear();
        let augments = self
            .inner
            .augment_details
            .try_lock()
            .map_or(0, |mut cache| std::mem::take(&mut *cache).len());
        let went = MemoryUse {
            entries: count(assets + players + history + builds + augments),
            image_bytes: image_bytes as u64,
        };
        debug!(
            entries = went.entries,
            bytes = went.image_bytes,
            "caches cleared"
        );
        went
    }

    /// The bytes of pictures held at most ([`ASSETS`]).
    pub fn image_limit(&self) -> u64 {
        lock(&self.inner.assets).limits().bytes as u64
    }

    /// What the caches hold now.
    pub fn memory_use(&self) -> MemoryUse {
        let (assets, image_bytes) = {
            let assets = lock(&self.inner.assets);
            (assets.len(), assets.bytes())
        };
        let players = lock(&self.inner.players).len();
        let history = lock(&self.inner.history).entries();
        let builds = self.inner.loadout.entries();
        let augments = self
            .inner
            .augment_details
            .try_lock()
            .map_or(0, |cache| cache.len());
        MemoryUse {
            entries: count(assets + players + history + builds + augments),
            image_bytes: image_bytes as u64,
        }
    }
}

/// The players `client`'s views show now.
pub(super) fn players_shown(client: &Client) -> HashSet<String> {
    let live = lock(&client.live);
    let mut shown = HashSet::from([live.me.clone()]);
    shown.extend(live.champ_select.iter().flat_map(live::champ_select_puuids));
    shown.extend(live.gameflow.iter().flat_map(live::game_puuids));
    shown.extend(live.lobby.iter().flat_map(live::lobby_puuids));
    shown
}

/// A record no view needs any longer: fetched before [`PLAYER_TTL`], so champ select would fetch it
/// again; due since its player finished a game; or a failure past [`FAILED_PLAYER_TTL`].
fn expired(entry: &PlayerEntry, now: Instant) -> bool {
    let age = |at: &Instant| now.saturating_duration_since(*at);
    match entry {
        PlayerEntry::Loading => false,
        PlayerEntry::Ready(_, Some(at)) => age(at) >= PLAYER_TTL,
        PlayerEntry::Ready(_, None) => true,
        PlayerEntry::Failed(_, at) => age(at) >= FAILED_PLAYER_TTL,
    }
}

/// Keeps the players' records within [`PLAYERS`]: past it the expired go, then the oldest, never
/// one being fetched or on screen.
pub(super) fn trim_players(
    players: &mut HashMap<String, PlayerEntry>,
    shown: &HashSet<String>,
    now: Instant,
) {
    if players.len() <= PLAYERS {
        return;
    }
    players.retain(|puuid, entry| shown.contains(puuid) || !expired(entry, now));
    let excess = players.len().saturating_sub(PLAYERS);
    if excess == 0 {
        return;
    }
    let mut oldest: Vec<(Instant, String)> = players
        .iter()
        .filter(|(puuid, _)| !shown.contains(*puuid))
        .filter_map(|(puuid, entry)| match entry {
            PlayerEntry::Ready(_, Some(at)) | PlayerEntry::Failed(_, at) => {
                Some((*at, puuid.clone()))
            }
            PlayerEntry::Ready(_, None) | PlayerEntry::Loading => None,
        })
        .collect();
    oldest.sort_unstable();
    for (_, puuid) in oldest.into_iter().take(excess) {
        players.remove(&puuid);
    }
}

fn count(entries: usize) -> u32 {
    u32::try_from(entries).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lcu::{Credentials, Lcu};
    use serde_json::json;
    use tokio::runtime::Handle;

    use super::*;
    use crate::{
        history::{OLDER_TTL, Page},
        model::{Game, Summoner},
        service::{Asset, Live, PlayerRecord},
        settings::Language,
        view::HistorySource,
    };

    fn ready(puuid: &str, at: Option<Instant>) -> PlayerEntry {
        PlayerEntry::Ready(
            Arc::new(PlayerRecord {
                summoner: Summoner {
                    puuid: puuid.into(),
                    ..Summoner::default()
                },
                ranked: None,
                games: Vec::new(),
                focus: None,
                complete: true,
            }),
            at,
        )
    }

    #[test]
    fn players_records_stay_within_their_cap_never_one_on_screen_or_on_its_way() {
        let start = Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let mut players = HashMap::new();
        players.insert("loading".to_owned(), PlayerEntry::Loading);
        players.insert("shown".to_owned(), ready("shown", Some(start)));
        players.insert("due".to_owned(), ready("due", None));
        players.insert(
            "failed".to_owned(),
            PlayerEntry::Failed("refused".into(), start),
        );
        for index in 0..PLAYERS as u64 {
            let puuid = format!("p{index}");
            players.insert(puuid.clone(), ready(&puuid, Some(at(index))));
        }
        let shown = HashSet::from(["shown".to_owned()]);
        // A minute in: the failure and the record due since a game go first, then the oldest.
        trim_players(&mut players, &shown, at(60));
        assert_eq!(players.len(), PLAYERS);
        assert!(!players.contains_key("failed") && !players.contains_key("due"));
        assert!(!players.contains_key("p0") && !players.contains_key("p1"));
        assert!(players.contains_key("p2"));
        assert!(players.contains_key("loading") && players.contains_key("shown"));
    }

    /// A connection that never answers, in a game of `me` and `mate` against `foe`.
    fn in_game() -> Client {
        let credentials = Credentials::new(1, "t");
        Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::default(),
            live: Arc::new(Mutex::new(Live {
                me: "me".into(),
                gameflow: Some(
                    serde_json::from_value(json!({
                        "phase": "InProgress",
                        "gameData": {
                            "teamOne": [{"puuid": "me"}, {"puuid": "mate"}],
                            "teamTwo": [{"puuid": "foe"}]
                        }
                    }))
                    .unwrap(),
                ),
                ..Live::default()
            })),
        }
    }

    #[tokio::test]
    async fn the_sweep_and_the_cleanup_keep_what_is_on_screen_and_let_the_rest_go() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        *service
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(in_game());
        service.scope_account("me");

        let start = Instant::now();
        let just_now = start + OLDER_TTL - Duration::from_secs(1);
        {
            let mut players = lock(&service.inner.players);
            for puuid in ["me", "mate", "foe", "seen-before"] {
                players.insert(puuid.into(), ready(puuid, Some(start)));
            }
            players.insert("fetching".into(), PlayerEntry::Loading);
            players.insert("just-now".into(), ready("just-now", Some(just_now)));
        }
        lock(&service.inner.assets).insert(
            "/lol-game-data/assets/v1/champion-icons/1.png".into(),
            Arc::new(Asset {
                content_type: "image/png".into(),
                bytes: vec![0; 1000],
            }),
            1000,
            start,
        );
        // A page of one game from the server: the page's entry and the game kept whole.
        let game = Arc::new(Game {
            game_id: 1,
            ..Game::default()
        });
        let page = Page {
            entries: vec![Some(game)],
            more: false,
            source: HistorySource::Server,
        };
        lock(&service.inner.history).put_page("me", "seen-before", 0, page, start);
        service
            .inner
            .augment_details
            .lock()
            .await
            .insert(Language::ZhCn, (start, Arc::default()));
        assert_eq!(
            service.memory_use(),
            MemoryUse {
                // A picture, six records, the page's entry and its game, the descriptions.
                entries: 1 + 6 + 2 + 1,
                image_bytes: 1000,
            }
        );

        // Ten minutes on, the record of a player no view shows is past its time; the players in
        // the game are as old, and stay.
        let went = service.sweep_caches(start + PLAYER_TTL);
        assert_eq!(
            went,
            MemoryUse {
                entries: 1,
                image_bytes: 0
            }
        );
        assert!(!lock(&service.inner.players).contains_key("seen-before"));

        // Half an hour on: the picture not drawn since, the page and its game.
        let went = service.sweep_caches(start + OLDER_TTL);
        assert_eq!(
            went,
            MemoryUse {
                entries: 1 + 2,
                image_bytes: 1000,
            }
        );
        // Half a day on: the descriptions, and the record fetched last.
        let went = service.sweep_caches(start + AUGMENT_DETAILS_TTL);
        assert_eq!(went.entries, 2);

        lock(&service.inner.players).insert("stranger".into(), ready("stranger", Some(start)));
        let went = service.clear_caches();
        assert_eq!(
            went.entries, 1,
            "everything but what is on screen or on its way"
        );
        let players = lock(&service.inner.players);
        for puuid in ["me", "mate", "foe", "fetching"] {
            assert!(players.contains_key(puuid), "{puuid}");
        }
        assert_eq!(players.len(), 4);
    }
}
