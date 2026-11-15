//! Explicit-target history, isolated from the connected shard's own live views.
use super::*;
use serde::Deserialize;

const IDENTITIES: usize = 200;
/// What a lookup says when Riot Client, which resolves Riot IDs across shards, cannot be reached.
const NO_RIOT_CLIENT: &str = "Riot Client credentials are unavailable; reconnect the League client";
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

/// The most places of a player's history a regional record reads, as the connected shard's record
/// does (`RECORD_PAGES`).
const REGIONAL_MOST: u32 = 100;

/// A regional record's games and whether they are all there are: pages of up to fifty
/// ([`HISTORY_PAGE`]), each starting where the last one's entries ended, since a busy server gives
/// ten at a time (`history_page`); never past [`REGIONAL_MOST`] places, and no further once
/// `filled` says the form's window is held or the history ends. A page failing after the first
/// keeps what came, as the connected shard's `page_through` does.
async fn regional_games<F, Fut>(
    mut fetch: F,
    filled: impl Fn(&[Game]) -> bool,
) -> Result<(Vec<Game>, bool), CoreError>
where
    F: FnMut(u32, u32) -> Fut,
    Fut: Future<Output = Result<HistoryPage, CoreError>>,
{
    let mut games = Vec::new();
    let mut begin = 0;
    let mut complete = false;
    while begin < REGIONAL_MOST {
        let page = match fetch(begin, HISTORY_PAGE.min(REGIONAL_MOST - begin)).await {
            Ok(page) => page,
            Err(error) if begin == 0 => return Err(error),
            Err(error) => {
                debug!(%error, "a later page of a regional record failed; keeping what came");
                break;
            }
        };
        let read = page.entries.len() as u32;
        begin += read;
        complete = !page.more;
        games.extend(page.entries.into_iter().flatten().map(|g| (*g).clone()));
        if complete || read == 0 || filled(&games) {
            break;
        }
    }
    Ok((games, complete))
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

#[derive(Clone, Deserialize)]
struct AccountAlias {
    alias: AccountName,
    puuid: String,
}
#[derive(Clone, Deserialize)]
struct AccountName {
    game_name: String,
    tag_line: String,
}

/// `name#tag` with both halves there, trimmed; `None` for anything else.
fn full_riot_id(riot_id: &str) -> Option<(&str, &str)> {
    let (name, tag) = riot_id.trim().rsplit_once('#')?;
    let (name, tag) = (name.trim(), tag.trim());
    (!name.is_empty() && !tag.is_empty()).then_some((name, tag))
}

/// The supported shards other than `current`, in [`sgp::PLATFORMS`] order.
fn other_platforms(current: Option<&str>) -> Vec<String> {
    sgp::PLATFORMS
        .iter()
        .filter(|platform| {
            current.is_none_or(|current| !platform.eq_ignore_ascii_case(current.trim()))
        })
        .map(|platform| (*platform).to_owned())
        .collect()
}

/// Where in [`sgp::PLATFORMS`] a shard stands, for ordering hits of equal level.
fn platform_rank(platform: &str) -> usize {
    sgp::PLATFORMS
        .iter()
        .position(|known| known.eq_ignore_ascii_case(platform))
        .unwrap_or(usize::MAX)
}

/// The shard a lookup opens among those that hold the account: the highest level, then the shards'
/// own order; and the others, in the same order.
fn pick_regions(mut hits: Vec<(String, Summoner)>) -> Option<((String, Summoner), Vec<String>)> {
    hits.sort_by(|a, b| {
        b.1.summoner_level
            .cmp(&a.1.summoner_level)
            .then(platform_rank(&a.0).cmp(&platform_rank(&b.0)))
    });
    let mut hits = hits.into_iter();
    let open = hits.next()?;
    Some((open, hits.map(|(platform, _)| platform).collect()))
}

/// Asks every shard in `platforms` for the account at once (`probe`): those that hold it, and
/// whether any of them did not answer (which is said in a debug line, not to the user).
async fn probe_regions<F, Fut>(platforms: Vec<String>, probe: F) -> (Vec<(String, Summoner)>, bool)
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Option<Summoner>, String>>,
{
    let answers = futures_util::future::join_all(platforms.into_iter().map(|platform| {
        let answer = probe(platform.clone());
        async move { (platform, answer.await) }
    }))
    .await;
    let mut hits = Vec::new();
    let mut failed = false;
    for (platform, answer) in answers {
        match answer {
            Ok(Some(summoner)) => hits.push((platform, summoner)),
            Ok(None) => {}
            Err(error) => {
                debug!(%platform, %error, "a shard did not answer a cross-region lookup");
                failed = true;
            }
        }
    }
    (hits, failed)
}

/// A lookup that tries the connected shard first (`local`) and only when it has nobody of a full
/// Riot ID asks the other shards (`elsewhere`).
async fn anywhere<L, E, Fut>(
    riot_id: &str,
    local: L,
    elsewhere: E,
) -> Result<PlayerProfile, CoreError>
where
    L: Future<Output = Result<PlayerProfile, CoreError>>,
    E: FnOnce() -> Fut,
    Fut: Future<Output = Result<PlayerProfile, CoreError>>,
{
    match local.await {
        Err(CoreError::PlayerNotFound(_)) if full_riot_id(riot_id).is_some() => elsewhere().await,
        other => other,
    }
}

/// Keeps each shard's own summoner of the account, named as the alias names it (a shard's ledger
/// may give no name), so that a record read there later, games or none, shows who it is.
fn record_hits(
    state: &mut RegionalHistory,
    viewer: &str,
    alias: &AccountAlias,
    hits: &[(String, Summoner)],
) -> Result<(), CoreError> {
    for (platform, summoner) in hits {
        let mut summoner = summoner.clone();
        summoner.game_name = alias.alias.game_name.clone();
        summoner.tag_line = alias.alias.tag_line.clone();
        state.region_for(viewer, platform)?.remember(summoner);
    }
    Ok(())
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
        let pid = context
            .client
            .credentials
            .pid
            .ok_or_else(|| CoreError::Remote(NO_RIOT_CLIENT.into()))?;
        let credentials = tokio::task::spawn_blocking(move || lcu::discover_riot_client(pid))
            .await
            .map_err(|e| CoreError::Remote(e.to_string()))?
            .map_err(|_| CoreError::Remote(NO_RIOT_CLIENT.into()))?;
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
        let mut profile = analysis::profile(&summoner, None);
        profile.region = Some(requested.clone());
        let mut state = lock(&self.inner.regional_history);
        let region = state.region_for(&context.viewer, &requested)?;
        region
            .history
            .put_found(&context.viewer, riot_id, profile.clone(), Instant::now());
        region.remember(summoner);
        Ok(profile)
    }

    /// A full Riot ID wherever the account plays: the connected shard first ([`Service::find_player`]),
    /// and only when it has nobody, every other supported shard at once, the one with the highest
    /// level opening ([`pick_regions`]).
    pub async fn find_player_anywhere(&self, riot_id: &str) -> Result<PlayerProfile, CoreError> {
        anywhere(riot_id, self.find_player(riot_id), || {
            self.find_elsewhere(riot_id)
        })
        .await
    }

    async fn find_elsewhere(&self, riot_id: &str) -> Result<PlayerProfile, CoreError> {
        let (alias, hits, failed, context) = self.locate(riot_id).await?;
        let Some(((platform, summoner), others)) = pick_regions(hits.clone()) else {
            return Err(if failed {
                CoreError::Remote("some regions did not answer; try again".into())
            } else {
                CoreError::PlayerNotFound(riot_id.to_owned())
            });
        };
        self.same_account(&context.client, &context.viewer)?;
        let mut named = summoner;
        named.game_name = alias.alias.game_name.clone();
        named.tag_line = alias.alias.tag_line.clone();
        let mut profile = analysis::profile(&named, None);
        profile.region = Some(platform.clone());
        profile.other_regions = others;
        let mut state = lock(&self.inner.regional_history);
        record_hits(&mut state, &context.viewer, &alias, &hits)?;
        state
            .region_for(&context.viewer, &platform)?
            .history
            .put_found(&context.viewer, riot_id, profile.clone(), Instant::now());
        Ok(profile)
    }

    /// The other shards holding a full Riot ID's account, for a player opened on the connected
    /// shard, as [`pick_regions`] orders them; empty where Riot Client cannot be reached or knows
    /// no such alias, an error only when no shard answered.
    pub async fn player_regions(&self, riot_id: &str) -> Result<Vec<String>, CoreError> {
        let (alias, hits, failed, context) = match self.locate(riot_id).await {
            Ok(located) => located,
            Err(CoreError::PlayerNotFound(_)) => return Ok(Vec::new()),
            Err(CoreError::Remote(message)) if message == NO_RIOT_CLIENT => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        if hits.is_empty() && failed {
            return Err(CoreError::Remote(
                "some regions did not answer; try again".into(),
            ));
        }
        self.same_account(&context.client, &context.viewer)?;
        record_hits(
            &mut lock(&self.inner.regional_history),
            &context.viewer,
            &alias,
            &hits,
        )?;
        Ok(pick_regions(hits)
            .map(|((open, _), others)| std::iter::once(open).chain(others).collect())
            .unwrap_or_default())
    }

    /// A full Riot ID's alias (Riot Client) and the shards other than the connected one that hold
    /// its account ([`probe_regions`]), with the context the requests were made in.
    async fn locate(
        &self,
        riot_id: &str,
    ) -> Result<(AccountAlias, Vec<(String, Summoner)>, bool, Context), CoreError> {
        let (name, tag) = full_riot_id(riot_id)
            .ok_or_else(|| CoreError::Invalid("enter a complete Riot ID as name#tag".into()))?;
        let platforms = other_platforms(self.current_platform().await.as_deref());
        let first = platforms
            .first()
            .cloned()
            .ok_or_else(|| CoreError::PlayerNotFound(riot_id.to_owned()))?;
        // One context's tokens serve every shard's ledger: the shard is in each request's path.
        let context = self.regional_context(&first).await?;
        let riot = self.riot_client(&context).await?;
        let path = format!(
            "/player-account/aliases/v1/lookup?gameName={}&tagLine={}",
            query_value(name),
            query_value(tag)
        );
        let aliases: Vec<AccountAlias> = riot.get(&path).await?;
        let alias = exact_alias(&aliases, name, tag)
            .cloned()
            .ok_or_else(|| CoreError::PlayerNotFound(riot_id.to_owned()))?;
        let (token, agent, puuid) = (
            context.league_token.as_str(),
            context.shard.user_agent.as_str(),
            alias.puuid.as_str(),
        );
        let (hits, failed) = probe_regions(platforms, |platform| async move {
            let base = sgp::base(&platform).ok_or_else(|| "no server for the shard".to_owned())?;
            sgp::summoner_by_puuid(base, token, agent, &platform, puuid).await
        })
        .await;
        Ok((alias, hits, failed, context))
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
        let (games, complete) = regional_games(
            |begin, count| self.regional_page(context, puuid, begin, count),
            |games| analysis::window_filled(games, &kinds, None),
        )
        .await?;
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
    use crate::{model::Game, service::CoreError, view::Ranked};
    use std::{sync::Arc, time::Instant};

    /// `count` games from `begin`, newest first, against players on the Rift.
    fn regional_page_of(begin: u32, count: u32, more: bool) -> HistoryPage {
        HistoryPage {
            entries: (begin..begin + count)
                .map(|at| {
                    Some(Arc::new(Game {
                        game_id: i64::from(at) + 1,
                        game_creation: 1_000_000 - i64::from(at),
                        game_mode: "CLASSIC".into(),
                        ..Game::default()
                    }))
                })
                .collect(),
            more,
            source: HistorySource::Server,
        }
    }

    #[tokio::test]
    async fn a_regional_record_reads_fifty_at_a_time_from_where_the_last_page_ended() {
        let kinds = analysis::QueueKinds::new();
        let window = |games: &[Game]| analysis::window_filled(games, &kinds, None);
        let asked = std::sync::Mutex::new(Vec::new());
        let full = |begin: u32, count: u32| {
            asked.lock().unwrap().push((begin, count));
            async move { Ok::<_, CoreError>(regional_page_of(begin, count, true)) }
        };
        let (games, complete) = regional_games(full, window).await.unwrap();
        assert_eq!(
            (games.len(), complete),
            (50, false),
            "a full page holds the window"
        );
        assert_eq!(*asked.lock().unwrap(), [(0, 50)]);

        asked.lock().unwrap().clear();
        let (games, _) = regional_games(full, |_| false).await.unwrap();
        assert_eq!(games.len(), 100, "never more than a hundred places");
        assert_eq!(*asked.lock().unwrap(), [(0, 50), (50, 50)]);

        // A busy server gives ten at a time: the next page starts after them, and the places read
        // still stop at a hundred.
        asked.lock().unwrap().clear();
        let busy_first = |begin: u32, count: u32| {
            asked.lock().unwrap().push((begin, count));
            let count = if begin == 0 { 10 } else { count };
            async move { Ok::<_, CoreError>(regional_page_of(begin, count, true)) }
        };
        let (games, _) = regional_games(busy_first, |_| false).await.unwrap();
        assert_eq!(games.len(), 100);
        assert_eq!(*asked.lock().unwrap(), [(0, 50), (10, 50), (60, 40)]);
        assert!(
            games
                .windows(2)
                .all(|pair| pair[1].game_id == pair[0].game_id + 1),
            "no place skipped"
        );

        // A later page failing keeps what came; the first failing is the record's error.
        let later = |begin: u32, count: u32| async move {
            if begin == 0 {
                Ok(regional_page_of(begin, count, true))
            } else {
                Err(CoreError::Remote("timeout".into()))
            }
        };
        let (games, complete) = regional_games(later, |_| false).await.unwrap();
        assert_eq!((games.len(), complete), (50, false));
        let first =
            |_: u32, _: u32| async { Err::<HistoryPage, _>(CoreError::Remote("401".into())) };
        assert!(regional_games(first, |_| false).await.is_err());
    }

    fn ledger(puuid: &str, level: i64) -> Summoner {
        Summoner {
            puuid: puuid.into(),
            summoner_level: level,
            ..Summoner::default()
        }
    }

    #[test]
    fn the_shard_with_the_highest_level_opens_and_the_others_follow() {
        assert!(pick_regions(Vec::new()).is_none());
        let (open, others) = pick_regions(vec![("TJ100".into(), ledger("p", 30))]).unwrap();
        assert_eq!((open.0.as_str(), others.len()), ("TJ100", 0));
        let (open, others) = pick_regions(vec![
            ("BGP2".into(), ledger("p", 40)),
            ("HN1".into(), ledger("p", 300)),
            ("TJ100".into(), ledger("p", 40)),
        ])
        .unwrap();
        assert_eq!(open.0, "HN1", "the highest level");
        assert_eq!(
            others,
            ["TJ100", "BGP2"],
            "equal levels in the shards' own order"
        );
    }

    #[tokio::test]
    async fn every_other_shard_is_asked_at_once_and_one_not_answering_is_left_out() {
        let asked = std::sync::Mutex::new(Vec::new());
        let probe = |platform: String| {
            asked.lock().unwrap().push(platform.clone());
            async move {
                match platform.as_str() {
                    "HN1" => Ok(Some(ledger("p", 120))),
                    "NJ100" => Err("timeout".to_owned()),
                    _ => Ok(None),
                }
            }
        };
        let platforms: Vec<String> = ["HN1", "NJ100", "TJ100"].map(String::from).into();
        let (hits, failed) = probe_regions(platforms, probe).await;
        assert_eq!(asked.lock().unwrap().len(), 3);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "HN1");
        assert!(failed, "NJ100 did not answer");
        assert_eq!(
            other_platforms(Some("gz100")),
            ["HN1", "HN10", "TJ100", "TJ101", "NJ100", "CQ100", "BGP2"],
            "the connected shard is not asked again"
        );
        assert_eq!(other_platforms(None).len(), sgp::PLATFORMS.len());
    }

    #[tokio::test]
    async fn the_connected_shard_answers_first_and_the_others_only_when_it_has_nobody() {
        let found = |region: Option<&str>| PlayerProfile {
            puuid: "p".into(),
            name: None,
            level: 1,
            icon_id: 0,
            private: false,
            ranked: Ranked::default(),
            region: region.map(str::to_owned),
            other_regions: Vec::new(),
        };
        let probes = std::sync::atomic::AtomicUsize::new(0);
        let elsewhere = || {
            probes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { Ok(found(Some("HN1"))) }
        };
        let local = anywhere("a#1", async { Ok(found(None)) }, elsewhere)
            .await
            .unwrap();
        assert_eq!(local.region, None);
        assert_eq!(
            probes.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "no other shard asked"
        );
        let missing = || async { Err(CoreError::PlayerNotFound("a#1".into())) };
        let remote = anywhere("a#1", missing(), elsewhere).await.unwrap();
        assert_eq!(remote.region.as_deref(), Some("HN1"));
        assert_eq!(probes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            matches!(
                anywhere("a", missing(), elsewhere).await,
                Err(CoreError::PlayerNotFound(_))
            ),
            "a name without its tag is looked for on the connected shard only"
        );
        let busy = async { Err(CoreError::Remote("503".into())) };
        assert!(matches!(
            anywhere("a#1", busy, elsewhere).await,
            Err(CoreError::Remote(_))
        ));
        assert_eq!(probes.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn every_shard_found_keeps_the_riot_id_for_a_record_with_no_games() {
        let mut state = RegionalHistory::default();
        state.scope("viewer");
        let alias = AccountAlias {
            alias: AccountName {
                game_name: "周周喵".into(),
                tag_line: "0520".into(),
            },
            puuid: "p".into(),
        };
        let hits = vec![
            ("HN1".to_owned(), ledger("p", 120)),
            ("TJ100".to_owned(), ledger("p", 30)),
        ];
        record_hits(&mut state, "viewer", &alias, &hits).unwrap();
        for platform in ["HN1", "TJ100"] {
            let kept = &state.region(platform).identities.get("p").unwrap().0;
            assert_eq!(
                (kept.game_name.as_str(), kept.tag_line.as_str()),
                ("周周喵", "0520")
            );
            assert!(
                kept.summoner_level > 0,
                "a regional record reads it instead of asking"
            );
        }
    }

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
