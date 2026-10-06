//! Runes, summoner spells, builds and item sets, carried out against the client: the champ-select
//! hooks of the rune and spell memory and of the item sets, and the build panel's commands. What to
//! do is decided in `crate::loadout`; the numbers come from `crate::builds`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lcu::{Lcu, Method};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use super::{Client, CoreError, SUMMONER, Service, lock, now_ms, queue_mode};
use crate::{
    builds::{self, Build, BuildSource, Known, Query, RunePage, StyleBook},
    loadout::{
        self, ItemSets, LoadoutStore, LoadoutSummary, PageOutcome, PageTarget, PerkInventory,
        PerkPage, RecommendedPage, Setup, Watch,
    },
    model::{ChampSelectSession, PerkStyles, Summoner},
    settings::Mode,
    view::{GameData, NoticeKind, Phase, Position},
};

const PAGES: &str = "/lol-perks/v1/pages";
const CURRENT_PAGE: &str = "/lol-perks/v1/currentpage";
const INVENTORY: &str = "/lol-perks/v1/inventory";
const MY_SELECTION: &str = "/lol-champ-select/v1/session/my-selection";

/// How long one source's numbers for one champion are reused. The sources recount once a day.
const BUILD_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Past this many cached lookups, the stale ones are dropped.
const BUILD_ENTRIES: usize = 256;

/// One source's numbers for a champion in a kind of game, lane and patch (Tencent's only; the
/// others say theirs in the answer).
type BuildKey = (BuildSource, i64, Mode, Option<Position>, String);
/// Locked across the fetch, so two views asking at once make one request.
type BuildSlot = Arc<tokio::sync::Mutex<Option<(Instant, Arc<Build>)>>>;

/// What the service keeps for the whole run.
pub(super) struct LoadoutState {
    store: LoadoutStore,
    builds: Mutex<HashMap<BuildKey, BuildSlot>>,
    patches: tokio::sync::Mutex<Option<(Instant, Arc<Vec<String>>)>>,
    /// What the client's catalog knows, with the catalog it was read from.
    known: Mutex<Option<(Arc<GameData>, Arc<Known>)>>,
    /// One change to the client's runes, spells or item sets at a time: a swap during a write must
    /// not leave the champion before it on the page.
    acting: tokio::sync::Mutex<()>,
}

impl LoadoutState {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            store: LoadoutStore::open(path),
            builds: Mutex::new(HashMap::new()),
            patches: tokio::sync::Mutex::new(None),
            known: Mutex::new(None),
            acting: tokio::sync::Mutex::new(()),
        }
    }
}

/// What one client connection's champ selects have left to do.
#[derive(Default)]
pub(super) struct LoadoutLive {
    watch: Watch,
    /// The last champ select, until the game it led to starts: what the player goes in with is
    /// remembered then.
    last: Option<ChampSelectSession>,
}

/// The champ select under way, while it is still `game_id` and the local player still holds
/// `champion`.
fn holding(client: &Client, game_id: i64, champion: i64) -> Option<ChampSelectSession> {
    lock(&client.live).champ_select.clone().filter(|session| {
        session.game_id == game_id && loadout::held_champion(session) == Some(champion)
    })
}

impl Service {
    /// Called with every champ-select session: sets up the runes and spells, and writes the item
    /// set, of a champion that has just become the player's, where those rules are on.
    pub(super) fn loadout_champ_select(&self, client: &Client, session: &ChampSelectSession) {
        let settings = self.settings();
        let mode = queue_mode(client, session.queue_id);
        let (setup, item_set) = {
            let mut live = lock(&client.live);
            let state = &mut live.loadout;
            state.last = Some(session.clone());
            // Without the catalog the kind of game is not known yet; the next event asks again.
            let Some(mode) = mode else { return };
            let setup = if settings.automation.restores_loadout(Some(mode)) {
                state.watch.loadout_due(session)
            } else {
                None
            };
            let item_set =
                if settings.builds.enabled && settings.automation.writes_item_sets(Some(mode)) {
                    state.watch.item_set_due(session)
                } else {
                    None
                };
            (
                setup.map(|champion| (champion, mode)),
                item_set.map(|champion| (champion, mode)),
            )
        };
        if let Some((champion, mode)) = setup {
            self.set_up(client, champion, mode, session.game_id);
        }
        if let Some((champion, mode)) = item_set {
            let lane = session
                .local_player()
                .and_then(|me| Position::parse(&me.assigned_position));
            self.write_item_set_by_rule(client, champion, mode, lane, session.game_id);
        }
    }

    /// Called with every gameflow phase: once the game starts, what the player took into it is
    /// remembered. Outside champ select the watch starts over, so the next champ select sets its
    /// champion up even where the client numbers both games alike.
    pub(super) fn loadout_phase(&self, client: &Client, phase: Phase) {
        if phase == Phase::ChampSelect {
            return;
        }
        let last = {
            let mut live = lock(&client.live);
            live.loadout.watch = Watch::default();
            live.loadout.last.take()
        };
        // Champ select ended without a game (a dodge) when the phase is anything else.
        if let (Phase::GameStart | Phase::InProgress, Some(session)) = (phase, last) {
            self.remember(client, session);
        }
    }

    /// Stores the final runes and spells for the champion and kind of game, whoever chose them.
    fn remember(&self, client: &Client, session: ChampSelectSession) {
        let Some(mode) = queue_mode(client, session.queue_id) else {
            return;
        };
        if !self.settings().automation.restores_loadout(Some(mode)) {
            return;
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let page = client
                .lcu
                .get_optional::<PerkPage>(CURRENT_PAGE)
                .await
                .unwrap_or_else(|error| {
                    debug!(%error, "current rune page unavailable");
                    None
                });
            let Some((champion, setup)) =
                loadout::setup_at_start(&session, page.as_ref(), now_ms())
            else {
                return;
            };
            let store = service.clone();
            match tokio::task::spawn_blocking(move || {
                store.inner.loadout.store.put(champion, mode, setup)
            })
            .await
            {
                Ok(Ok(())) => debug!(champion, ?mode, "loadout remembered"),
                Ok(Err(error)) => warn!(%error, "loadout not remembered"),
                Err(error) => warn!(%error, "loadout not remembered"),
            }
        });
    }

    /// Sets up the remembered runes and spells, or the client's own recommendation where there are
    /// none, for `champion`, now that it is the player's.
    fn set_up(&self, client: &Client, champion: i64, mode: Mode, game_id: i64) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let _turn = service.inner.loadout.acting.lock().await;
            // Asked again: meanwhile the champion may have changed, or the rule been turned off.
            let Some(session) = holding(&client, game_id, champion) else {
                return;
            };
            let rule = service.settings().automation;
            if !rule.restores_loadout(Some(mode)) {
                return;
            }
            let (setup, recommended) = match service.inner.loadout.store.get(champion, mode) {
                Some(setup) => (setup, false),
                None if rule.loadout.recommended => {
                    match client_recommendation(&client.lcu, champion, mode, &session).await {
                        Ok(Some(setup)) => (setup, true),
                        Ok(None) => return,
                        Err(error) => {
                            service.notice(NoticeKind::Failed {
                                action: "loadout".into(),
                                message: error.to_string(),
                            });
                            return;
                        }
                    }
                }
                None => return,
            };
            // The rune page first, then the spells; a failure of one does not keep the other back.
            let mut errors = Vec::new();
            let runes = match &setup.runes {
                Some(runes) => match service.write_page(&client, champion, runes).await {
                    Ok(outcome) => Some(outcome),
                    Err(error) => {
                        errors.push(error.to_string());
                        None
                    }
                },
                None => None,
            };
            let spells = match setup.spells {
                Some(spells) => match service.set_spells(&client, spells).await {
                    Ok(()) => true,
                    Err(error) => {
                        errors.push(error.to_string());
                        false
                    }
                },
                None => false,
            };
            if !errors.is_empty() {
                service.notice(NoticeKind::Failed {
                    action: "loadout".into(),
                    message: errors.join("; "),
                });
            }
            if runes.is_some() || spells {
                info!(champion, ?mode, recommended, "loadout set up");
                service.notice(NoticeKind::LoadoutApplied {
                    champion_id: champion,
                    recommended,
                    runes,
                    spells,
                });
            }
        });
    }

    /// Writes `runes` to winer's own page (`loadout::page_target`) and makes it the current one.
    async fn write_page(
        &self,
        client: &Client,
        champion: i64,
        runes: &RunePage,
    ) -> Result<PageOutcome, CoreError> {
        let lcu = &client.lcu;
        let (pages, inventory) = tokio::join!(
            lcu.get::<Vec<Value>>(PAGES),
            lcu.get::<PerkInventory>(INVENTORY)
        );
        let (pages, inventory) = (pages?, inventory?);
        let read: Vec<PerkPage> = pages
            .iter()
            .map(|page| serde_json::from_value(page.clone()).unwrap_or_default())
            .collect();
        let name = loadout::page_name(&self.champion_name(client, champion));
        let id = match loadout::page_target(&read, &inventory) {
            PageTarget::Own(id) | PageTarget::Temporary(id) => {
                let page = pages
                    .iter()
                    .find(|page| page.get("id").and_then(Value::as_i64) == Some(id))
                    .ok_or_else(|| CoreError::Invalid(format!("rune page {id} is gone")))?;
                lcu.put(
                    &format!("{PAGES}/{id}"),
                    &loadout::page_body(page, &name, runes),
                )
                .await?;
                id
            }
            PageTarget::New => {
                let created: Option<PerkPage> = lcu
                    .call(Method::POST, PAGES, &loadout::new_page_body(&name, runes))
                    .await?;
                match created.map(|page| page.id).filter(|&id| id > 0) {
                    Some(id) => id,
                    // An answer without the page: it is found again by its name.
                    None => lcu
                        .get::<Vec<PerkPage>>(PAGES)
                        .await?
                        .into_iter()
                        .find(|page| page.name == name)
                        .map(|page| page.id)
                        .ok_or_else(|| {
                            CoreError::Invalid("the new rune page did not appear".into())
                        })?,
                }
            }
            PageTarget::Full => return Ok(PageOutcome::NoPage),
        };
        // `current: true` in the write normally makes it current; where it did not, say so.
        let current: Option<PerkPage> = lcu.get_optional(CURRENT_PAGE).await?;
        if current.is_none_or(|page| page.id != id) {
            lcu.put(CURRENT_PAGE, &id).await?;
        }
        Ok(PageOutcome::Written)
    }

    /// Puts `wanted` on the player's spell keys in the champ select under way.
    async fn set_spells(&self, client: &Client, wanted: [i64; 2]) -> Result<(), CoreError> {
        if wanted[0] <= 0 || wanted[1] <= 0 || wanted[0] == wanted[1] {
            return Err(CoreError::Invalid(format!(
                "not two summoner spells: {wanted:?}"
            )));
        }
        let current = lock(&client.live)
            .champ_select
            .as_ref()
            .and_then(|session| session.local_player())
            .map(|me| [me.spell1_id, me.spell2_id])
            .ok_or_else(|| CoreError::Invalid("not in champ select".into()))?;
        let spells = loadout::arrange_spells(current, wanted);
        if spells != current {
            client
                .lcu
                .patch(
                    MY_SELECTION,
                    &json!({ "spell1Id": spells[0], "spell2Id": spells[1] }),
                )
                .await?;
        }
        Ok(())
    }

    /// Writes winer's item set for the champion now that it is the player's, once.
    fn write_item_set_by_rule(
        &self,
        client: &Client,
        champion: i64,
        mode: Mode,
        lane: Option<Position>,
        game_id: i64,
    ) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            // The numbers first, outside the turn: the rune page does not wait for the internet.
            let build = service.build(champion, mode, lane).await;
            let _turn = service.inner.loadout.acting.lock().await;
            if holding(&client, game_id, champion).is_none() {
                return;
            }
            let result = match build {
                Ok(build) => service.put_item_set(&client, &build).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => service.notice(NoticeKind::ItemSetWritten {
                    champion_id: champion,
                }),
                Err(error) => service.notice(NoticeKind::Failed {
                    action: "itemSet".into(),
                    message: error.to_string(),
                }),
            }
        });
    }

    /// Reads the client's item sets, puts winer's for `build` in, and writes them all back.
    async fn put_item_set(&self, client: &Client, build: &Build) -> Result<(), CoreError> {
        let language = self.settings().general.language;
        let set = loadout::item_set(
            build,
            &self.champion_name(client, build.champion_id),
            language,
        );
        let (path, sets) = item_sets(&client.lcu).await?;
        let sets = sets.with(build.champion_id, build.mode, set, now_ms().max(0) as u64);
        client.lcu.put(&path, &sets).await?;
        Ok(())
    }

    fn champion_name(&self, client: &Client, champion: i64) -> String {
        lock(&client.data)
            .as_ref()
            .and_then(|data| data.champions.iter().find(|entry| entry.id == champion))
            .map_or_else(|| champion.to_string(), |entry| entry.short_name.clone())
    }

    /// What the client's catalog knows, which every build is checked against.
    async fn known(&self, client: &Client) -> Result<Arc<Known>, CoreError> {
        let data = lock(&client.data)
            .clone()
            .filter(|data| !data.items.is_empty())
            .ok_or_else(|| CoreError::Invalid("the client's game data has not arrived".into()))?;
        if let Some((of, known)) = lock(&self.inner.loadout.known).as_ref()
            && Arc::ptr_eq(of, &data)
        {
            return Ok(known.clone());
        }
        // The catalog keeps the styles' names and icons only; which rune belongs to which style is
        // read here.
        let styles: PerkStyles = client
            .lcu
            .get("/lol-game-data/assets/v1/perkstyles.json")
            .await?;
        let known = Arc::new(Known {
            items: data.items.iter().map(|item| item.id).collect(),
            augments: data
                .augments
                .iter()
                .filter_map(|augment| Some((augment.id, augment.rarity?)))
                .collect(),
            styles: StyleBook::new(&styles),
        });
        *lock(&self.inner.loadout.known) = Some((data, known.clone()));
        Ok(known)
    }

    /// Tencent's patches, newest first, read again every few hours.
    async fn tencent_patches(&self) -> Result<Arc<Vec<String>>, CoreError> {
        let mut cache = self.inner.loadout.patches.lock().await;
        if let Some((at, patches)) = cache.as_ref()
            && at.elapsed() < BUILD_TTL
        {
            return Ok(patches.clone());
        }
        let patches = Arc::new(
            builds::fetch_tencent_patches()
                .await
                .map_err(CoreError::Remote)?,
        );
        *cache = Some((Instant::now(), patches.clone()));
        Ok(patches)
    }

    /// One source's numbers, from the cache while they are fresh. A failure is not cached, so a
    /// retry asks again.
    async fn source_build(
        &self,
        source: BuildSource,
        query: Query,
        known: &Known,
    ) -> Result<Arc<Build>, CoreError> {
        let patches = match source {
            BuildSource::Tencent => Some(self.tencent_patches().await?),
            _ => None,
        };
        let patch = patches
            .as_ref()
            .and_then(|patches| patches.first().cloned())
            .unwrap_or_default();
        let slot = {
            let mut cache = lock(&self.inner.loadout.builds);
            if cache.len() >= BUILD_ENTRIES {
                cache.retain(|_, slot| {
                    slot.try_lock().map_or(true, |entry| {
                        entry
                            .as_ref()
                            .is_some_and(|(at, _)| at.elapsed() < BUILD_TTL)
                    })
                });
            }
            cache
                .entry((source, query.champion_id, query.mode, query.lane, patch))
                .or_default()
                .clone()
        };
        let mut entry = slot.lock().await;
        if let Some((at, build)) = entry.as_ref()
            && at.elapsed() < BUILD_TTL
        {
            return Ok(build.clone());
        }
        let champion = query.champion_id;
        let build = match source {
            BuildSource::Tencent => {
                let patches = patches.as_deref().map_or(&[][..], Vec::as_slice);
                builds::fetch_tencent(query, patches, known).await
            }
            BuildSource::TencentHextech => builds::fetch_tencent_hextech(champion, known).await,
            BuildSource::OpGg => builds::fetch_opgg(query, known).await,
            BuildSource::AramGg => builds::fetch_aramgg(champion, known).await,
        }
        .map_err(CoreError::Remote)?;
        if build.is_empty() {
            return Err(CoreError::Remote(format!(
                "{source:?} has no numbers for champion {champion}"
            )));
        }
        let build = Arc::new(build);
        *entry = Some((Instant::now(), build.clone()));
        Ok(build)
    }

    /// What players take on `champion_id` in a game of `mode` (and `lane`, on the Rift; `None`
    /// takes the lane it is played in most), from the first source that answers
    /// (`builds::sources`).
    pub async fn build(
        &self,
        champion_id: i64,
        mode: Mode,
        lane: Option<Position>,
    ) -> Result<Build, CoreError> {
        let settings = self.settings().builds;
        if !settings.enabled {
            return Err(CoreError::Invalid("the build panel is switched off".into()));
        }
        if champion_id <= 0 {
            return Err(CoreError::Invalid(format!("no champion {champion_id}")));
        }
        let sources = builds::sources(mode, settings.rift_source);
        if sources.is_empty() {
            return Err(CoreError::Invalid(format!("no numbers for {mode:?} games")));
        }
        let client = self.client()?;
        let known = self.known(&client).await?;
        let query = Query {
            champion_id,
            mode,
            lane: lane.filter(|_| matches!(mode, Mode::Ranked | Mode::Normal)),
        };
        let mut failure = None;
        for &source in sources {
            match self.source_build(source, query, &known).await {
                Ok(build) => return Ok((*build).clone()),
                Err(error) => {
                    debug!(%error, ?source, "no build from this source");
                    failure = Some(error);
                }
            }
        }
        Err(failure.unwrap_or_else(|| CoreError::Remote("no source answered".into())))
    }

    /// Writes `runes` to winer's rune page, named after `champion_id`, and makes it current.
    pub async fn apply_runes(
        &self,
        champion_id: i64,
        runes: RunePage,
    ) -> Result<PageOutcome, CoreError> {
        if !runes.is_complete() {
            return Err(CoreError::Invalid("not a full rune page".into()));
        }
        let client = self.client()?;
        let _turn = self.inner.loadout.acting.lock().await;
        self.write_page(&client, champion_id, &runes).await
    }

    /// Takes `spells` in the champ select under way, on the keys the player already uses for them.
    pub async fn apply_spells(&self, spells: [i64; 2]) -> Result<(), CoreError> {
        let client = self.client()?;
        let _turn = self.inner.loadout.acting.lock().await;
        self.set_spells(&client, spells).await
    }

    /// Writes winer's item set for `champion_id` in a game of `mode` from the build panel's numbers.
    pub async fn write_item_set(
        &self,
        champion_id: i64,
        mode: Mode,
        lane: Option<Position>,
    ) -> Result<(), CoreError> {
        let build = self.build(champion_id, mode, lane).await?;
        let client = self.client()?;
        let _turn = self.inner.loadout.acting.lock().await;
        self.put_item_set(&client, &build).await
    }

    /// Takes every one of winer's item sets out of the client's list; returns how many went.
    pub async fn clear_item_sets(&self) -> Result<u32, CoreError> {
        let client = self.client()?;
        let _turn = self.inner.loadout.acting.lock().await;
        let (path, sets) = item_sets(&client.lcu).await?;
        let (sets, removed) = sets.without_ours(now_ms().max(0) as u64);
        if removed > 0 {
            client.lcu.put(&path, &sets).await?;
        }
        Ok(removed)
    }

    pub fn loadout_summary(&self) -> LoadoutSummary {
        self.inner.loadout.store.summary()
    }

    /// Forgets every remembered setup. Blocking: the file is written before it returns.
    pub fn clear_loadouts(&self) -> Result<LoadoutSummary, CoreError> {
        self.inner.loadout.store.clear()?;
        Ok(self.loadout_summary())
    }
}

/// The client's recommended page for the champion, position and map, as a setup.
async fn client_recommendation(
    lcu: &Lcu,
    champion: i64,
    mode: Mode,
    session: &ChampSelectSession,
) -> Result<Option<Setup>, CoreError> {
    let Some(map) = loadout::recommendation_map(mode) else {
        return Ok(None);
    };
    let position = loadout::recommendation_position(session);
    let pages: Vec<RecommendedPage> = lcu
        .get(&format!(
            "/lol-perks/v1/recommended-pages/champion/{champion}/position/{position}/map/{map}"
        ))
        .await?;
    Ok(loadout::recommended_setup(&pages))
}

/// The local player's item sets as the client keeps them, and where they are written back.
async fn item_sets(lcu: &Lcu) -> Result<(String, ItemSets), CoreError> {
    let summoner: Summoner = lcu.get(SUMMONER).await?;
    if summoner.summoner_id <= 0 {
        return Err(CoreError::Invalid("the client named no summoner".into()));
    }
    let path = format!("/lol-item-sets/v1/item-sets/{}/sets", summoner.summoner_id);
    // Read as bytes, so the sets that are not winer's go back exactly as they came.
    let (_, body) = lcu.bytes(&path).await?;
    let sets = serde_json::from_slice(&body)
        .map_err(|error| CoreError::Invalid(format!("unexpected item sets: {error}")))?;
    Ok((path, sets))
}

#[cfg(test)]
mod tests {
    use lcu::Credentials;
    use tokio::runtime::Handle;

    use super::*;
    use crate::{
        test_support::fixture,
        view::{GameData, QueueInfo},
    };

    fn service(dir: &tempfile::TempDir) -> Service {
        Service::new(dir.path().join("settings.json"), Handle::current())
    }

    /// A client that answers nothing. The captured locked-in champ select is a custom game with a
    /// bench (queue 3120); its catalog names that queue ARAM.
    fn client(queues: &[i64]) -> Client {
        let credentials = Credentials::new(1, "t");
        let data = GameData {
            queues: queues
                .iter()
                .map(|&id| QueueInfo {
                    id,
                    name: "自定义".into(),
                    game_mode: "ARAM".into(),
                    ranked: false,
                })
                .collect(),
            ..GameData::default()
        };
        Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::new(Mutex::new(Some(Arc::new(data)))),
            live: Arc::default(),
        }
    }

    fn locked() -> ChampSelectSession {
        fixture("lcu-rest/lol-champ-select-v1-session--champ-select-finalization.json")
    }

    /// Whether the champion would still be set up, and its item set still written, by the watch.
    fn still_due(client: &Client, session: &ChampSelectSession) -> (bool, bool) {
        let mut watch = lock(&client.live).loadout.watch.clone();
        (
            watch.loadout_due(session).is_some(),
            watch.item_set_due(session).is_some(),
        )
    }

    #[tokio::test]
    async fn the_last_champ_select_is_kept_until_its_game_starts_and_dropped_on_a_dodge() {
        let dir = tempfile::tempdir().unwrap();
        let (service, client) = (service(&dir), client(&[3120]));
        service.loadout_champ_select(&client, &locked());
        assert!(lock(&client.live).loadout.last.is_some());
        service.loadout_phase(&client, Phase::ChampSelect);
        assert!(lock(&client.live).loadout.last.is_some(), "still choosing");
        service.loadout_phase(&client, Phase::Lobby);
        assert!(
            lock(&client.live).loadout.last.is_none(),
            "a dodge leaves nothing to remember"
        );

        service.loadout_champ_select(&client, &locked());
        service.loadout_phase(&client, Phase::GameStart);
        assert!(
            lock(&client.live).loadout.last.is_none(),
            "taken once the game starts"
        );
    }

    #[tokio::test]
    async fn a_champion_is_set_up_only_while_its_rule_acts_in_the_mode() {
        let dir = tempfile::tempdir().unwrap();
        let (service, client) = (service(&dir), client(&[3120]));
        let session = locked();
        assert_eq!(queue_mode(&client, session.queue_id), Some(Mode::Aram));
        // Off: the champion is not used up, so turning the rules on still acts on it.
        service.loadout_champ_select(&client, &session);
        assert_eq!(still_due(&client, &session), (true, true));

        let mut settings = service.settings();
        settings.automation.loadout.enabled = true;
        settings.automation.item_sets = true;
        settings.automation.scopes.loadout = vec![Mode::Ranked];
        settings.automation.scopes.item_sets = vec![Mode::Ranked];
        service.set_settings(settings.clone()).unwrap();
        service.loadout_champ_select(&client, &session);
        assert_eq!(
            still_due(&client, &session),
            (true, true),
            "ARAM is out of both rules' modes"
        );

        // Where the kind of game is not known yet, nothing acts, whatever the scopes.
        let unknown = Client {
            data: Arc::default(),
            ..client.clone()
        };
        let mut everywhere = settings.clone();
        everywhere.automation.scopes = crate::settings::Scopes::default();
        service.set_settings(everywhere).unwrap();
        service.loadout_champ_select(&unknown, &session);
        assert_eq!(still_due(&unknown, &session), (true, true));

        settings.automation.scopes.loadout = vec![Mode::Aram];
        settings.automation.scopes.item_sets = vec![Mode::Aram];
        settings.builds.enabled = false;
        service.set_settings(settings).unwrap();
        service.loadout_champ_select(&client, &session);
        assert_eq!(
            still_due(&client, &session),
            (false, true),
            "set up once; the item set needs the build panel's numbers"
        );
        // Out of champ select the watch starts over, whatever the next game's id.
        service.loadout_phase(&client, Phase::None);
        assert_eq!(still_due(&client, &session), (true, true));
    }

    #[tokio::test]
    async fn the_build_panel_says_why_it_has_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(&dir);
        assert!(matches!(
            service.build(202, Mode::Ranked, None).await,
            Err(CoreError::NotConnected)
        ));
        assert!(matches!(
            service.build(202, Mode::Other, None).await,
            Err(CoreError::Invalid(_))
        ));
        let mut settings = service.settings();
        settings.builds.enabled = false;
        service.set_settings(settings).unwrap();
        assert!(matches!(
            service.build(202, Mode::Ranked, None).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            service
                .apply_runes(
                    202,
                    RunePage {
                        primary_style: 8000,
                        sub_style: 8000,
                        perks: vec![]
                    }
                )
                .await,
            Err(CoreError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn remembered_setups_live_beside_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(&dir);
        assert_eq!(service.loadout_summary().remembered, 0);
        assert_eq!(service.clear_loadouts().unwrap().remembered, 0);
        assert!(dir.path().join("loadouts.json").exists());
    }
}
