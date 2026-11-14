//! Explicit-target history, isolated from the connected shard's own live views.
use super::*;
use serde::Deserialize;

const IDENTITIES: usize = 200;
const RECORDS: usize = 32;

/// At most the eight supported shards; each history keeps its own bounded pages and games.
#[derive(Default)]
pub(super) struct RegionalHistory {
    viewer: String,
    regions: HashMap<String, RegionalCache>,
}

struct RegionalCache {
    history: HistoryCache,
    identities: HashMap<String, (Summoner, Instant)>,
    records: HashMap<String, (Arc<PlayerRecord>, Instant)>,
    server: Arc<HistoryServer>,
}

impl RegionalHistory {
    pub(super) fn scope(&mut self, viewer: &str) {
        if self.viewer != viewer {
            self.viewer = viewer.to_owned();
            self.regions.clear();
        }
    }

    fn region(&mut self, platform: &str) -> &mut RegionalCache {
        self.regions.entry(platform.to_owned()).or_insert_with(|| {
            let mut history = HistoryCache::default();
            history.scope(&self.viewer);
            RegionalCache {
                history,
                identities: HashMap::new(),
                records: HashMap::new(),
                server: Arc::new(HistoryServer::new()),
            }
        })
    }

    fn region_for(
        &mut self,
        viewer: &str,
        platform: &str,
    ) -> Result<&mut RegionalCache, CoreError> {
        if self.viewer != viewer {
            return Err(CoreError::Busy("the signed-in account changed".into()));
        }
        Ok(self.region(platform))
    }

    pub(super) fn entries(&self) -> usize {
        self.regions
            .values()
            .map(|c| c.history.entries() + c.identities.len() + c.records.len())
            .sum()
    }

    pub(super) fn clear(&mut self) -> usize {
        let count = self.entries();
        self.regions.clear();
        count
    }

    pub(super) fn sweep(&mut self, now: Instant) -> usize {
        let before = self.entries();
        for region in self.regions.values_mut() {
            region.history.sweep(now);
            region.identities.retain(|_, (_, at)| {
                now.saturating_duration_since(*at) < crate::history::FOUND_TTL
            });
            region
                .records
                .retain(|_, (_, at)| now.saturating_duration_since(*at) < NEWEST_TTL);
        }
        before - self.entries()
    }
}

impl RegionalCache {
    fn remember(&mut self, summoner: Summoner) {
        if self.identities.len() >= IDENTITIES
            && let Some(key) = self
                .identities
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(k, _)| k.clone())
        {
            self.identities.remove(&key);
        }
        self.identities
            .insert(summoner.puuid.clone(), (summoner, Instant::now()));
    }

    fn remember_names(&mut self, game: &Game) {
        for identity in &game.participant_identities {
            let player = &identity.player;
            if player.puuid.is_empty() || player.game_name.is_empty() {
                continue;
            }
            if self.identities.contains_key(&player.puuid) {
                continue;
            }
            self.remember(Summoner {
                puuid: player.puuid.clone(),
                summoner_id: player.summoner_id,
                game_name: player.game_name.clone(),
                tag_line: player.tag_line.clone(),
                profile_icon_id: player.profile_icon,
                ..Summoner::default()
            });
        }
    }
}

fn platform(input: &str) -> Result<String, CoreError> {
    let platform = input.trim().to_ascii_uppercase();
    if sgp::base(&platform).is_none() {
        return Err(CoreError::Invalid(
            "choose a supported Tencent region".into(),
        ));
    }
    Ok(platform)
}

struct Context {
    client: Client,
    viewer: String,
    platform: String,
    shard: Shard,
    league_token: String,
    server: Arc<HistoryServer>,
}

#[derive(Deserialize)]
struct AccountAlias {
    alias: AccountName,
    puuid: String,
}
#[derive(Deserialize)]
struct AccountName {
    game_name: String,
    tag_line: String,
}

fn exact_alias<'a>(aliases: &'a [AccountAlias], name: &str, tag: &str) -> Option<&'a AccountAlias> {
    aliases.iter().find(|a| {
        !a.puuid.is_empty()
            && a.alias.game_name.eq_ignore_ascii_case(name)
            && a.alias.tag_line.eq_ignore_ascii_case(tag)
    })
}

impl Service {
    async fn regional_context(&self, requested: &str) -> Result<Context, CoreError> {
        let platform = platform(requested)?;
        let client = self.client()?;
        let viewer = lock(&client.live).me.clone();
        self.scope_account(&viewer);
        let (entitlements, version, league_token) = tokio::join!(
            client.lcu.get::<Entitlements>("/entitlements/v1/token"),
            client.lcu.get::<String>("/lol-patch/v1/game-version"),
            client
                .lcu
                .get::<String>("/lol-league-session/v1/league-session-token"),
        );
        let entitlements = entitlements?;
        let league_token = league_token?;
        if entitlements.access_token.is_empty() || league_token.is_empty() {
            return Err(CoreError::Remote(
                "the client has no active session token for cross-region history".into(),
            ));
        }
        self.same_account(&client, &viewer)?;
        let server = lock(&self.inner.regional_history)
            .region_for(&viewer, &platform)?
            .server
            .clone();
        Ok(Context {
            client,
            viewer,
            platform: platform.clone(),
            shard: Shard {
                base: sgp::base(&platform).expect("validated region"),
                token: entitlements.access_token,
                user_agent: sgp::user_agent(&version.unwrap_or_default()),
            },
            league_token,
            server,
        })
    }

    async fn riot_client(&self, context: &Context) -> Result<Lcu, CoreError> {
        let pid = context.client.credentials.pid.ok_or_else(|| {
            CoreError::Remote(
                "Riot Client credentials are unavailable; reconnect the League client".into(),
            )
        })?;
        let credentials = tokio::task::spawn_blocking(move || lcu::discover_riot_client(pid))
            .await
            .map_err(|e| CoreError::Remote(e.to_string()))?
            .map_err(|_| {
                CoreError::Remote(
                    "Riot Client credentials are unavailable; reconnect the League client".into(),
                )
            })?;
        self.same_account(&context.client, &context.viewer)?;
        Lcu::new(&credentials)
            .map_err(|_| CoreError::Remote("could not connect to Riot Client".into()))
    }

    /// Resolves a full Riot ID globally, then verifies the identity exists on the selected shard.
    pub async fn find_player_in(
        &self,
        riot_id: &str,
        platform_id: &str,
    ) -> Result<PlayerProfile, CoreError> {
        let requested = platform(platform_id)?;
        let (name, tag) = riot_id
            .trim()
            .rsplit_once('#')
            .ok_or_else(|| CoreError::Invalid("enter a complete Riot ID as name#tag".into()))?;
        let (name, tag) = (name.trim(), tag.trim());
        if name.is_empty() || tag.is_empty() {
            return Err(CoreError::Invalid(
                "enter a complete Riot ID as name#tag".into(),
            ));
        }
        let context = self.regional_context(&requested).await?;
        let kept = lock(&self.inner.regional_history)
            .region_for(&context.viewer, &requested)?
            .history
            .found(&context.viewer, riot_id, Instant::now());
        if let Some(profile) = kept {
            self.same_account(&context.client, &context.viewer)?;
            return Ok(profile);
        }
        let riot = self.riot_client(&context).await?;
        let path = format!(
            "/player-account/aliases/v1/lookup?gameName={}&tagLine={}",
            query_value(name),
            query_value(tag)
        );
        let aliases: Vec<AccountAlias> = riot.get(&path).await?;
        let alias = exact_alias(&aliases, name, tag)
            .ok_or_else(|| CoreError::PlayerNotFound(riot_id.to_owned()))?;
        let mut summoner = sgp::summoner_by_puuid(
            context.shard.base,
            &context.league_token,
            &context.shard.user_agent,
            &requested,
            &alias.puuid,
        )
        .await
        .map_err(CoreError::Remote)?
        .ok_or_else(|| CoreError::PlayerNotFound(riot_id.to_owned()))?;
        summoner.game_name = alias.alias.game_name.clone();
        summoner.tag_line = alias.alias.tag_line.clone();
        self.same_account(&context.client, &context.viewer)?;
        let profile = analysis::profile(&summoner, None);
        let mut state = lock(&self.inner.regional_history);
        let region = state.region_for(&context.viewer, &requested)?;
        region
            .history
            .put_found(&context.viewer, riot_id, profile.clone(), Instant::now());
        region.remember(summoner);
        Ok(profile)
    }

    async fn regional_page(
        &self,
        context: &Context,
        puuid: &str,
        begin: u32,
        count: u32,
    ) -> Result<HistoryPage, CoreError> {
        segment(puuid)?;
        let count = count.clamp(1, HISTORY_PAGE);
        let kept = lock(&self.inner.regional_history)
            .region_for(&context.viewer, &context.platform)?
            .history
            .page(&context.viewer, puuid, begin, count, Instant::now());
        if let Some(page) = kept {
            self.same_account(&context.client, &context.viewer)?;
            return Ok(page);
        }
        // Deliberately no LCU fallback: a local page belongs to a different shard.
        let (page, more) = history_page(
            |begin, count| shard_page(&context.server, &context.shard, puuid, begin, count),
            &context.server,
            begin,
            count,
        )
        .await?;
        let page = HistoryPage {
            entries: page.entries.into_iter().map(|g| g.map(Arc::new)).collect(),
            more,
            source: HistorySource::Server,
        };
        self.same_account(&context.client, &context.viewer)?;
        let mut state = lock(&self.inner.regional_history);
        let region = state.region_for(&context.viewer, &context.platform)?;
        for game in page.entries.iter().flatten() {
            region.remember_names(game);
        }
        region
            .history
            .put_page(&context.viewer, puuid, begin, page.clone(), Instant::now());
        Ok(page)
    }

    pub async fn match_history_in(
        &self,
        puuid: &str,
        begin: u32,
        count: u32,
        platform_id: &str,
    ) -> Result<MatchPage, CoreError> {
        let context = self.regional_context(platform_id).await?;
        let page = self.regional_page(&context, puuid, begin, count).await?;
        self.same_account(&context.client, &context.viewer)?;
        let data = lock(&context.client.data).clone().unwrap_or_default();
        Ok(MatchPage {
            puuid: puuid.to_owned(),
            begin,
            next: begin + page.entries.len() as u32,
            games: page
                .entries
                .iter()
                .flatten()
                .filter_map(|g| analysis::match_summary(puuid, g, &data.roles, &data.kinds))
                .collect(),
            has_more: page.more,
            source: page.source,
        })
    }

    pub async fn match_detail_in(
        &self,
        game_id: i64,
        platform_id: &str,
    ) -> Result<MatchDetail, CoreError> {
        if game_id <= 0 {
            return Err(CoreError::Invalid("choose a recorded game".into()));
        }
        let context = self.regional_context(platform_id).await?;
        let kept = lock(&self.inner.regional_history)
            .region_for(&context.viewer, &context.platform)?
            .history
            .game(&context.viewer, game_id);
        let game = match kept {
            Some(game) => game,
            None => Arc::new(
                sgp::game(
                    context.shard.base,
                    &context.shard.token,
                    &context.shard.user_agent,
                    &context.platform,
                    game_id,
                )
                .await
                .map_err(CoreError::Remote)?,
            ),
        };
        self.same_account(&context.client, &context.viewer)?;
        lock(&self.inner.regional_history)
            .region_for(&context.viewer, &context.platform)?
            .history
            .put_game(&context.viewer, game.clone(), Instant::now());
        let data = lock(&context.client.data).clone().unwrap_or_default();
        Ok(analysis::match_detail(&game, &data.roles))
    }

    async fn regional_record(
        &self,
        context: &Context,
        puuid: &str,
    ) -> Result<Arc<PlayerRecord>, CoreError> {
        segment(puuid)?;
        let kept = lock(&self.inner.regional_history)
            .region_for(&context.viewer, &context.platform)?
            .records
            .get(puuid)
            .filter(|(_, at)| at.elapsed() < NEWEST_TTL)
            .map(|(r, _)| r.clone());
        if let Some(record) = kept {
            self.same_account(&context.client, &context.viewer)?;
            return Ok(record);
        }
        let kinds = catalog_kinds(&context.client);
        let mut games = Vec::new();
        let mut begin = 0;
        let mut complete = false;
        while begin < 100 {
            let page = self.regional_page(context, puuid, begin, 20).await?;
            begin += page.entries.len() as u32;
            complete = !page.more;
            games.extend(page.entries.into_iter().flatten().map(|g| (*g).clone()));
            if complete || analysis::window_filled(&games, &kinds, None) {
                break;
            }
        }
        let cached = lock(&self.inner.regional_history)
            .region_for(&context.viewer, &context.platform)?
            .identities
            .get(puuid)
            .map(|(s, _)| s.clone());
        let mut summoner = match cached.as_ref().filter(|s| s.summoner_level > 0) {
            Some(s) => s.clone(),
            None => sgp::summoner_by_puuid(
                context.shard.base,
                &context.league_token,
                &context.shard.user_agent,
                &context.platform,
                puuid,
            )
            .await
            .map_err(CoreError::Remote)?
            .ok_or_else(|| CoreError::PlayerNotFound(puuid.to_owned()))?,
        };
        if let Some(cached) = cached {
            summoner.game_name = cached.game_name;
            summoner.tag_line = cached.tag_line;
        }
        self.same_account(&context.client, &context.viewer)?;
        let record = Arc::new(PlayerRecord {
            summoner: summoner.clone(),
            ranked: None,
            games,
            focus: None,
            complete,
        });
        let mut state = lock(&self.inner.regional_history);
        let region = state.region_for(&context.viewer, &context.platform)?;
        region.remember(summoner);
        if region.records.len() >= RECORDS
            && let Some(key) = region
                .records
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(k, _)| k.clone())
        {
            region.records.remove(&key);
        }
        region
            .records
            .insert(puuid.to_owned(), (record.clone(), Instant::now()));
        Ok(record)
    }

    pub async fn player_summary_in(
        &self,
        puuid: &str,
        platform_id: &str,
    ) -> Result<PlayerSummary, CoreError> {
        let context = self.regional_context(platform_id).await?;
        let record = self.regional_record(&context, puuid).await?;
        self.same_account(&context.client, &context.viewer)?;
        Ok(self.summarize(&record, None))
    }

    pub async fn player_standing_in(
        &self,
        puuid: &str,
        platform_id: &str,
    ) -> Result<PlayerStanding, CoreError> {
        let context = self.regional_context(platform_id).await?;
        let record = self.regional_record(&context, puuid).await?;
        self.same_account(&context.client, &context.viewer)?;
        let summary = self.summarize(&record, None);
        let data = self.game_data().unwrap_or_default();
        let settings = self.settings();
        let rated = rate_alone(
            &summary,
            &callout::ranking(&settings.automation.callout, &settings.general),
        );
        Ok(PlayerStanding {
            scope: analysis::form_scope(
                puuid,
                &record.games,
                &analysis::Catalog {
                    kinds: &data.kinds,
                    roles: &data.roles,
                },
            ),
            band: rated.as_ref().map(|(_, b)| *b),
            rating: rated.map(|(r, _)| r),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::Game, service::CoreError};
    use std::{sync::Arc, time::Instant};

    #[test]
    fn cross_region_target_is_normalized_and_cannot_be_a_url() {
        assert_eq!(platform(" hn1 ").unwrap(), "HN1");
        for invalid in ["", "NA1", "https://example.com", "HN1/../GZ100"] {
            assert!(matches!(platform(invalid), Err(CoreError::Invalid(_))));
        }
    }

    #[test]
    fn cross_region_games_with_the_same_id_and_account_changes_are_isolated() {
        let mut state = RegionalHistory::default();
        state.scope("account-a");
        let now = Instant::now();
        for (region, mode) in [("HN1", "ARAM"), ("GZ100", "CLASSIC")] {
            let game = Arc::new(Game {
                game_id: 7,
                game_mode: mode.into(),
                ..Game::default()
            });
            state
                .region(region)
                .history
                .put_game("account-a", game, now);
        }
        assert_eq!(
            state
                .region("HN1")
                .history
                .game("account-a", 7)
                .unwrap()
                .game_mode,
            "ARAM"
        );
        assert_eq!(
            state
                .region("GZ100")
                .history
                .game("account-a", 7)
                .unwrap()
                .game_mode,
            "CLASSIC"
        );
        assert!(state.entries() >= 2);
        assert!(state.clear() >= 2);
        assert_eq!(state.entries(), 0);
        state.region("HN1").history.put_game(
            "account-a",
            Arc::new(Game {
                game_id: 7,
                ..Game::default()
            }),
            now,
        );
        state.scope("account-b");
        assert!(state.region("HN1").history.game("account-b", 7).is_none());
    }

    #[test]
    fn cross_region_aliases_match_both_name_and_tag_without_losing_zeroes() {
        let aliases = vec![
            AccountAlias {
                puuid: "wrong".into(),
                alias: AccountName {
                    game_name: "周周喵".into(),
                    tag_line: "520".into(),
                },
            },
            AccountAlias {
                puuid: "right".into(),
                alias: AccountName {
                    game_name: "周周喵".into(),
                    tag_line: "0520".into(),
                },
            },
        ];
        assert_eq!(
            exact_alias(&aliases, "周周喵", "0520").unwrap().puuid,
            "right"
        );
        assert!(exact_alias(&aliases, "另一个名字", "0520").is_none());
    }

    #[tokio::test]
    async fn cross_region_rejects_untrusted_platforms_before_discovering_any_client() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(matches!(
            service
                .find_player_in("周周喵#0520", "https://example.com")
                .await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service.find_player_in("周周喵#0520", "HN1").await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.match_history_in("p", 0, 20, "HN1").await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.match_detail_in(7, "HN1").await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.player_summary_in("p", "HN1").await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.player_standing_in("p", "HN1").await,
            Err(CoreError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn cross_region_server_errors_are_not_relabelled_as_a_missing_player_or_empty_page() {
        let server = HistoryServer::new();
        let result = history_page(
            |_, _| async {
                Err(CoreError::Remote(
                    "selected shard refused the request".into(),
                ))
            },
            &server,
            0,
            20,
        )
        .await;
        assert!(
            matches!(result,Err(CoreError::Remote(message)) if message.contains("selected shard"))
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires an owner's running, signed-in League client; reads only"]
    async fn live_cross_region_history_smoke() {
        tokio::time::timeout(Duration::from_secs(90), async {
            let credentials = lcu::discover().expect("running League client");
            let lcu = Lcu::new(&credentials).unwrap();
            let current: Summoner = lcu.get(SUMMONER).await.unwrap();
            let ent: Entitlements = lcu.get("/entitlements/v1/token").await.unwrap();
            let client = Client {
                lcu,
                credentials,
                platform_id: sgp::platform_of_issuer(&ent.issuer).unwrap_or_default(),
                data: Arc::new(Mutex::new(None)),
                live: Arc::new(Mutex::new(Live {
                    me: current.puuid,
                    ..Live::default()
                })),
            };
            let dir = tempfile::tempdir().unwrap();
            let service = Service::new(dir.path().join("settings.json"), Handle::current());
            *service.inner.client.write().unwrap() = Some(client);
            let profile = service.find_player_in("周周喵#0520", "HN1").await.unwrap();
            assert_eq!(
                profile.name.as_ref().map(|n| (&*n.game_name, &*n.tag_line)),
                Some(("周周喵", "0520"))
            );
            let page = service
                .match_history_in(&profile.puuid, 0, 2, "HN1")
                .await
                .unwrap();
            assert!(!page.games.is_empty());
            let game_id = page.games[0].game_id;
            let detail = service.match_detail_in(game_id, "HN1").await.unwrap();
            assert_eq!(detail.game_id, game_id);
            assert!(detail.teams.iter().map(|t| t.players.len()).sum::<usize>() > 0);
            let summary = service
                .player_summary_in(&profile.puuid, "HN1")
                .await
                .unwrap();
            assert_eq!(summary.puuid, profile.puuid);
            let _standing = service
                .player_standing_in(&profile.puuid, "HN1")
                .await
                .unwrap();
            println!(
                "cross-region smoke success: platform=HN1 listed={} gameId={} players={}",
                page.games.len(),
                game_id,
                detail.teams.iter().map(|t| t.players.len()).sum::<usize>()
            );
        })
        .await
        .expect("live history should finish within 90 seconds");
    }

    #[test]
    fn cross_region_stale_context_cannot_read_or_write_the_next_accounts_bucket() {
        let mut state = RegionalHistory::default();
        state.scope("account-a");
        state
            .region_for("account-a", "HN1")
            .unwrap()
            .history
            .put_game(
                "account-a",
                Arc::new(Game {
                    game_id: 7,
                    game_mode: "PRIVATE_A".into(),
                    ..Game::default()
                }),
                Instant::now(),
            );
        state.scope("account-b");
        state
            .region_for("account-b", "HN1")
            .unwrap()
            .history
            .put_game(
                "account-b",
                Arc::new(Game {
                    game_id: 7,
                    game_mode: "PUBLIC_B".into(),
                    ..Game::default()
                }),
                Instant::now(),
            );
        // An A response arriving after B scoped the outer cache must not even acquire a bucket.
        assert!(matches!(
            state.region_for("account-a", "HN1"),
            Err(CoreError::Busy(_))
        ));
        assert!(matches!(
            state.region_for("account-a", "GZ100"),
            Err(CoreError::Busy(_))
        ));
        assert!(!state.regions.contains_key("GZ100"));
        let b = state.region_for("account-b", "HN1").unwrap();
        assert_eq!(b.history.viewer(), "account-b");
        assert_eq!(
            b.history.game("account-b", 7).unwrap().game_mode,
            "PUBLIC_B"
        );
        assert!(b.identities.is_empty());
        assert!(b.records.is_empty());
    }
}
