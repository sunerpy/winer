//! Runes, summoner spells, builds and item sets, carried out against the client: the champ-select
//! hooks that set up the champion in hand (the user's own setup, else winer's recommendation) and
//! write its item set; the view of it the window's and the client's panels draw, the current rune
//! page read for it, and what those panels ask for; and the build panel's commands. What to do is
//! decided in `crate::loadout`; the numbers come from `crate::builds`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use lcu::{Lcu, Method};
use serde_json::{Value, json};
use tokio::time::sleep;
use tracing::{debug, info};

use super::{Client, CoreError, SUMMONER, Service, lock, now_ms, queue_mode};
use crate::{
    builds::{self, Build, BuildSource, Known, Query, RunePage, StyleBook},
    loadout::{
        self, AppliedLoadout, ItemSets, LoadoutAction, LoadoutCurrent, LoadoutDone, LoadoutPart,
        LoadoutRecommendation, LoadoutStore, LoadoutSummary, LoadoutTarget, LoadoutView,
        PageOutcome, PageTarget, PerkInventory, PerkPage, Plan, RecommendationState,
        RecommendedPage, Setup, Watch,
    },
    model::{ChampSelectSession, PerkStyles, Summoner},
    settings::{BuildSettings, Mode, Settings},
    view::{AssetInfo, GameData, NoticeKind, Phase, Position},
};

const PAGES: &str = "/lol-perks/v1/pages";
/// Also an event the service subscribes to.
pub(super) const CURRENT_PAGE: &str = "/lol-perks/v1/currentpage";
const INVENTORY: &str = "/lol-perks/v1/inventory";
const MY_SELECTION: &str = "/lol-champ-select/v1/session/my-selection";

/// How often the current rune page is read while champ select lasts: the panels show it, and the
/// client's own event for it is not measured on Tencent's shards.
const PAGE_POLL: Duration = Duration::from_secs(2);

/// How long one source's numbers for one champion are reused. The sources recount once a day.
const BUILD_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Cached lookups at most (a build is a few to a few tens of kilobytes): past it the stale ones go,
/// then the oldest, never one being fetched.
const BUILD_ENTRIES: usize = 256;

/// One source's numbers for a champion in a kind of game, lane and patch (Tencent's only; the
/// others say theirs in the answer).
type BuildKey = (BuildSource, i64, Mode, Option<Position>, String);
/// Locked across the fetch, so two views asking at once make one request.
type BuildSlot = Arc<tokio::sync::Mutex<Option<(Instant, Arc<Build>)>>>;

/// What the service keeps for the whole run.
pub(super) struct LoadoutState {
    /// The user's own setups (我的方案), `loadout-presets.json`.
    presets: LoadoutStore,
    /// `loadouts.json`, where winer up to 0.0.24 remembered each game's setup by itself: counted for
    /// the automation page and deleted when the user asks, never read to set anything up.
    legacy: PathBuf,
    /// Every loadout view's revision comes from here, so no two views with other contents share one.
    revisions: AtomicU32,
    /// Plugin contexts connected to the bridge now: the client's runes panel is drawn only while
    /// there is one, so only then is the recommendation looked up for it.
    plugins: AtomicU32,
    builds: Mutex<HashMap<BuildKey, BuildSlot>>,
    patches: tokio::sync::Mutex<Option<(Instant, Arc<Vec<String>>)>>,
    /// What the client's catalog knows, with the catalog it was read from.
    known: Mutex<Option<(Arc<GameData>, Arc<Known>)>>,
    /// One change to the client's runes, spells or item sets at a time: a swap during a write must
    /// not leave the champion before it on the page.
    acting: tokio::sync::Mutex<()>,
}

impl LoadoutState {
    pub(super) fn new(presets: PathBuf, legacy: PathBuf) -> Self {
        Self {
            presets: LoadoutStore::open(presets),
            legacy,
            revisions: AtomicU32::new(0),
            plugins: AtomicU32::new(0),
            builds: Mutex::new(HashMap::new()),
            patches: tokio::sync::Mutex::new(None),
            known: Mutex::new(None),
            acting: tokio::sync::Mutex::new(()),
        }
    }
}

/// What a recommendation was asked for: a champion in a champ select, kind of game and lane, under
/// the build panel's sources then.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RecommendationKey {
    game_id: i64,
    champion: i64,
    mode: Mode,
    lane: Option<Position>,
    builds: BuildSettings,
}

/// What winer put on the client in a champ select, so a change of the player's is told from its own.
#[derive(Clone, Debug, Default)]
struct Written {
    game_id: i64,
    /// The page written and the runes on it.
    page: Option<(i64, RunePage)>,
    spells: Option<[i64; 2]>,
}

/// What the player put up from a panel for a champion: the rule leaves each half as it is while the
/// client still has it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Asked {
    runes: Option<RunePage>,
    spells: Option<[i64; 2]>,
}

/// Keeps what the player asked winer to put up for `champion` in champ select `game_id`, from a
/// panel or the build panel: the rule leaves those halves while the client still has them.
fn note_asked(
    client: &Client,
    game_id: i64,
    champion: i64,
    runes: Option<&RunePage>,
    spells: Option<[i64; 2]>,
) {
    if runes.is_none() && spells.is_none() {
        return;
    }
    let mut live = lock(&client.live);
    let slot = &mut live.loadout.asked;
    let mut asked = match slot.take() {
        Some((game, held, asked)) if game == game_id && held == champion => asked,
        _ => Asked::default(),
    };
    if let Some(runes) = runes {
        asked.runes = Some(runes.clone());
    }
    if spells.is_some() {
        asked.spells = spells;
    }
    *slot = Some((game_id, champion, asked));
}

/// The champ select under way and the champion in hand in it, for a write the build panel asked.
fn in_hand(client: &Client) -> Option<(i64, i64)> {
    let live = lock(&client.live);
    let session = live.champ_select.as_ref()?;
    Some((session.game_id, loadout::champion_in_hand(session)?))
}

/// What one client connection's champ selects have left to do, and what their view is drawn from.
#[derive(Default)]
pub(super) struct LoadoutLive {
    watch: Watch,
    /// The current rune page as last read, in champ select `.0`; `None` inside where there is none.
    page: Option<(i64, Option<PerkPage>)>,
    /// A task reads the current page for this champ select (`Service::poll_pages`).
    polling: Option<i64>,
    /// winer's recommendation for the champion in hand: what it is for, and what came of it.
    recommendation: Option<(RecommendationKey, RecommendationState)>,
    /// What winer set up in champ select `.0` for champion `.1`.
    applied: Option<(i64, i64, AppliedLoadout)>,
    written: Written,
    /// What the player asked for from a panel in champ select `.0` for champion `.1`.
    asked: Option<(i64, i64, Asked)>,
    /// The view drawn last, with revision 0, and the revision it was given.
    last_view: Option<(LoadoutView, u32)>,
}

impl LoadoutLive {
    /// Forgets what was read and done for one champ select; the watch follows game ids itself.
    pub(super) fn reset_champ_select(&mut self) {
        self.page = None;
        self.recommendation = None;
        self.applied = None;
        self.written = Written::default();
        self.asked = None;
        self.last_view = None;
    }

    /// What the player asked for from a panel for `champion` in champ select `game_id`.
    fn asked_for(&self, game_id: i64, champion: i64) -> Option<Asked> {
        self.asked
            .as_ref()
            .filter(|(game, held, _)| *game == game_id && *held == champion)
            .map(|(_, _, asked)| asked.clone())
    }
}

/// The champ select under way, while it is still `game_id` and the local player still holds
/// `champion`.
fn holding(client: &Client, game_id: i64, champion: i64) -> Option<ChampSelectSession> {
    lock(&client.live).champ_select.clone().filter(|session| {
        session.game_id == game_id && loadout::held_champion(session) == Some(champion)
    })
}

/// The lane a champion is set up for: the one assigned, on the Rift.
fn lane_of(session: &ChampSelectSession, mode: Mode) -> Option<Position> {
    session
        .local_player()
        .and_then(|me| Position::parse(&me.assigned_position))
        .filter(|_| matches!(mode, Mode::Ranked | Mode::Normal))
}

fn spells_of(session: &ChampSelectSession) -> [i64; 2] {
    session
        .local_player()
        .map_or([0, 0], |me| [me.spell1_id, me.spell2_id])
}

/// Whether winer's recommendation and the current page are wanted for a game of `mode`: the rule
/// sets them up, or the client's panel shows them (`plugins`, the contexts connected).
fn wants_loadout(settings: &Settings, mode: Mode, plugins: u32) -> bool {
    loadout::shows(mode)
        && (settings.automation.restores_loadout(Some(mode))
            || (settings.plugin.loadout_panel && plugins > 0))
}

/// The client's current rune page; `None` where there is none or it could not be read.
async fn current_page(lcu: &Lcu) -> Option<PerkPage> {
    lcu.get_optional::<PerkPage>(CURRENT_PAGE)
        .await
        .unwrap_or_else(|error| {
            debug!(%error, "current rune page unavailable");
            None
        })
}

/// Whether the player made another page current, or changed the runes on it, between two reads:
/// a change that is the page winer wrote itself is not theirs.
fn changed_page(
    before: Option<&PerkPage>,
    now: Option<&PerkPage>,
    written: &Written,
    game_id: i64,
) -> bool {
    if loadout::same_page(before, now) {
        return false;
    }
    let winers = written.game_id == game_id
        && written.page.as_ref().is_some_and(|(id, runes)| {
            now.is_some_and(|now| now.id == *id && now.runes().as_ref() == Some(runes))
        });
    !winers
}

/// Whether the player changed their spells between two reads; winer's own change is not theirs.
fn changed_spells(before: [i64; 2], now: [i64; 2], written: &Written, game_id: i64) -> bool {
    if loadout::same_spells(before, now) {
        return false;
    }
    let winers = written.game_id == game_id
        && written
            .spells
            .is_some_and(|spells| loadout::same_spells(spells, now));
    !winers
}

/// The runes, styles and spells `current`, `mine` and `recommendation` name, as the catalog names
/// and draws them; ids the catalog does not know are left out.
fn loadout_assets(
    data: Option<&GameData>,
    current: &LoadoutCurrent,
    mine: Option<&Setup>,
    recommendation: Option<&LoadoutRecommendation>,
) -> Vec<AssetInfo> {
    let Some(data) = data else {
        return Vec::new();
    };
    let mut perks: Vec<i64> = Vec::new();
    let mut spells: Vec<i64> = Vec::new();
    let mut page = |runes: &RunePage| {
        perks.extend([runes.primary_style, runes.sub_style]);
        perks.extend(&runes.perks);
    };
    current.runes.iter().for_each(&mut page);
    mine.and_then(|mine| mine.runes.as_ref())
        .into_iter()
        .for_each(&mut page);
    if let Some(recommendation) = recommendation {
        recommendation
            .runes
            .iter()
            .for_each(|option| page(&option.page));
        spells.extend(
            recommendation
                .spells
                .iter()
                .flat_map(|option| option.spells),
        );
    }
    spells.extend(current.spells);
    spells.extend(mine.and_then(|mine| mine.spells).into_iter().flatten());
    let mut assets: Vec<AssetInfo> = Vec::new();
    let mut add = |catalog: &[AssetInfo], ids: &[i64]| {
        for &id in ids {
            if id > 0
                && !assets.iter().any(|asset| asset.id == id)
                && let Some(asset) = catalog.iter().find(|asset| asset.id == id)
            {
                assets.push(asset.clone());
            }
        }
    };
    add(&data.perks, &perks);
    add(&data.spells, &spells);
    assets
}

impl Service {
    /// Called with every champ-select session: sets up the runes and spells, and writes the item
    /// set, of a champion that has just become the player's, where those rules are on; and keeps
    /// the current rune page read for the panels.
    pub(super) fn loadout_champ_select(&self, client: &Client, session: &ChampSelectSession) {
        let settings = self.settings();
        // Without the catalog the kind of game is not known yet; the next event asks again.
        let Some(mode) = queue_mode(client, session.queue_id) else {
            return;
        };
        if wants_loadout(&settings, mode, self.plugins()) {
            self.poll_pages(client, session.game_id);
        }
        let (setup, item_set) = {
            let mut live = lock(&client.live);
            let state = &mut live.loadout;
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
            (setup, item_set)
        };
        if let Some(champion) = setup {
            self.set_up(client, champion, mode, session.game_id);
        }
        if let Some(champion) = item_set {
            let lane = session
                .local_player()
                .and_then(|me| Position::parse(&me.assigned_position));
            self.write_item_set_by_rule(client, champion, mode, lane, session.game_id);
        }
    }

    /// Called with every gameflow phase: outside champ select the watch starts over, so the next
    /// champ select sets its champion up even where the client numbers both games alike.
    pub(super) fn loadout_phase(&self, client: &Client, phase: Phase) {
        if phase != Phase::ChampSelect {
            lock(&client.live).loadout.watch = Watch::default();
        }
    }

    /// Reads the current rune page every [`PAGE_POLL`] while champ select `game_id` lasts and the
    /// panels want it, one task a champ select; each change draws the views again.
    fn poll_pages(&self, client: &Client, game_id: i64) {
        {
            let mut live = lock(&client.live);
            if live.loadout.polling == Some(game_id) {
                return;
            }
            live.loadout.polling = Some(game_id);
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            // A connection that has ended keeps its last phase: it is told by being replaced.
            while service.is_current(&client) {
                let mode = {
                    let live = lock(&client.live);
                    let on = live.phase == Phase::ChampSelect
                        && live
                            .champ_select
                            .as_ref()
                            .is_some_and(|session| session.game_id == game_id);
                    on.then(|| {
                        live.champ_select
                            .as_ref()
                            .map_or(0, |session| session.queue_id)
                    })
                };
                let Some(queue_id) = mode else { break };
                let wanted = queue_mode(&client, queue_id).is_some_and(|mode| {
                    wants_loadout(&service.settings(), mode, service.plugins())
                });
                if !wanted {
                    break;
                }
                let page = current_page(&client.lcu).await;
                if !service.is_current(&client) {
                    break;
                }
                if service.note_page(&client, game_id, page, "poll") {
                    service.render(&client);
                }
                sleep(PAGE_POLL).await;
            }
            let mut live = lock(&client.live);
            if live.loadout.polling == Some(game_id) {
                live.loadout.polling = None;
            }
        });
    }

    /// The client said its current rune page changed (`CURRENT_PAGE`'s event).
    pub(super) fn on_current_page(&self, client: &Client, page: Option<PerkPage>) {
        let game_id = {
            let live = lock(&client.live);
            live.champ_select
                .as_ref()
                .filter(|_| live.phase == Phase::ChampSelect)
                .map(|session| session.game_id)
        };
        if let Some(game_id) = game_id
            && self.note_page(client, game_id, page, "event")
        {
            self.render(client);
        }
    }

    /// Keeps `page`, read from `source`, as champ select `game_id`'s current page; whether it is
    /// another than the one kept.
    fn note_page(
        &self,
        client: &Client,
        game_id: i64,
        page: Option<PerkPage>,
        source: &str,
    ) -> bool {
        let mut live = lock(&client.live);
        let open = live
            .champ_select
            .as_ref()
            .is_some_and(|session| session.game_id == game_id);
        if !open {
            return false;
        }
        let changed = match &live.loadout.page {
            Some((game, kept)) if *game == game_id => {
                kept.as_ref()
                    .map(|page| (page.id, &page.name, page.runes()))
                    != page
                        .as_ref()
                        .map(|page| (page.id, &page.name, page.runes()))
            }
            _ => true,
        };
        if changed {
            debug!(
                source,
                page = page.as_ref().map(|page| page.id),
                "the current rune page"
            );
            live.loadout.page = Some((game_id, page));
        }
        changed
    }

    /// Reads the current page again now, after a write.
    async fn refresh_page(&self, client: &Client, game_id: i64) {
        let page = current_page(&client.lcu).await;
        self.note_page(client, game_id, page, "write");
    }

    /// The view of the local player's champion in champ select (`LoadoutView`), from what is known
    /// now; the recommendation not asked for yet is looked up in the background, its answer drawing
    /// the views again. `None` where the kind of game has no runes to choose, or before a champion
    /// is in hand.
    pub(super) fn loadout_view(
        &self,
        client: &Client,
        session: &ChampSelectSession,
        mode: Option<Mode>,
        settings: &Settings,
    ) -> Option<LoadoutView> {
        let mode = mode.filter(|&mode| loadout::shows(mode))?;
        let champion = loadout::champion_in_hand(session)?;
        let lane = lane_of(session, mode);
        let held = loadout::held_champion(session) == Some(champion);
        let mine = self
            .inner
            .loadout
            .presets
            .find(&loadout::preset_keys(champion, mode, lane))
            .map(|(_, setup)| setup);
        let key = RecommendationKey {
            game_id: session.game_id,
            champion,
            mode,
            lane,
            builds: settings.builds.clone(),
        };
        let wanted = wants_loadout(settings, mode, self.plugins());
        let (recommendation, fetch, page, applied) = {
            let mut live = lock(&client.live);
            let state = &mut live.loadout;
            let (recommendation, fetch) = if !wanted {
                (RecommendationState::Off, false)
            } else {
                match &state.recommendation {
                    Some((asked, answer)) if *asked == key => (answer.clone(), false),
                    _ => {
                        state.recommendation = Some((key.clone(), RecommendationState::Loading));
                        (RecommendationState::Loading, true)
                    }
                }
            };
            let page = state
                .page
                .as_ref()
                .filter(|(game, _)| *game == session.game_id)
                .and_then(|(_, page)| page.clone());
            let applied = state
                .applied
                .as_ref()
                .filter(|(game, held, _)| *game == session.game_id && *held == champion)
                .map(|(_, _, applied)| applied.clone());
            (recommendation, fetch, page, applied)
        };
        if fetch {
            self.fetch_loadout_recommendation(client, key, session.clone());
        }
        let current = LoadoutCurrent {
            runes: page.as_ref().and_then(PerkPage::runes),
            page: page.map(|page| page.name).unwrap_or_default(),
            spells: spells_of(session),
        };
        let assets = loadout_assets(
            lock(&client.data).as_deref(),
            &current,
            mine.as_ref(),
            recommendation.ready(),
        );
        let mut view = LoadoutView {
            game_id: session.game_id,
            champion_id: champion,
            mode,
            lane,
            held,
            revision: 0,
            auto: loadout::LoadoutAuto::of(&settings.automation, mode),
            current,
            mine,
            recommendation,
            applied,
            assets,
        };
        // A view with other contents takes the next revision; the same one keeps its own.
        let mut live = lock(&client.live);
        let last = &mut live.loadout.last_view;
        match last {
            Some((drawn, revision)) if *drawn == view => view.revision = *revision,
            _ => {
                let revision = self.inner.loadout.revisions.fetch_add(1, Ordering::Relaxed) + 1;
                *last = Some((view.clone(), revision));
                view.revision = revision;
            }
        }
        Some(view)
    }

    /// Looks up the recommendation `key` names and keeps it while it is still the one asked for.
    fn fetch_loadout_recommendation(
        &self,
        client: &Client,
        key: RecommendationKey,
        session: ChampSelectSession,
    ) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let answer = match service
                .recommend(
                    &client,
                    key.champion,
                    key.mode,
                    key.lane,
                    &key.builds,
                    &session,
                )
                .await
            {
                Ok(recommendation) if recommendation.is_empty() => RecommendationState::Nothing,
                Ok(recommendation) => RecommendationState::Ready { recommendation },
                Err(error) => {
                    debug!(%error, champion = key.champion, "no runes or spells to recommend");
                    RecommendationState::Failed {
                        message: error.to_string(),
                    }
                }
            };
            let kept = {
                let mut live = lock(&client.live);
                match &mut live.loadout.recommendation {
                    Some((asked, slot)) if *asked == key => {
                        *slot = answer;
                        true
                    }
                    _ => false,
                }
            };
            if kept {
                service.render_current(&client);
            }
        });
    }

    /// What winer recommends for `champion`: its numbers where the build panel has a source for the
    /// mode with runes and spells (the Rift's and ARAM's), else, or for a half they lack, the
    /// client's own recommended pages.
    async fn recommend(
        &self,
        client: &Client,
        champion: i64,
        mode: Mode,
        lane: Option<Position>,
        builds: &BuildSettings,
        session: &ChampSelectSession,
    ) -> Result<LoadoutRecommendation, CoreError> {
        let build = if matches!(mode, Mode::Ranked | Mode::Normal | Mode::Aram) {
            match self.build_with(builds, champion, mode, lane).await {
                Ok(build) => Some(build),
                Err(error) => {
                    debug!(%error, champion, ?mode, "no numbers for the runes and spells");
                    None
                }
            }
        } else {
            None
        };
        let built = loadout::recommendation(build.as_deref(), &[]);
        if built.runes_from.is_some() && built.spells_from.is_some() {
            return Ok(built);
        }
        let pages = client_pages(&client.lcu, champion, mode, session).await?;
        Ok(loadout::recommendation(build.as_deref(), &pages))
    }

    /// The recommendation the view holds for `key`, or a fresh lookup where it holds none.
    async fn recommendation_for(
        &self,
        client: &Client,
        key: RecommendationKey,
        session: &ChampSelectSession,
    ) -> Result<LoadoutRecommendation, CoreError> {
        let held = lock(&client.live)
            .loadout
            .recommendation
            .as_ref()
            .filter(|(asked, _)| *asked == key)
            .and_then(|(_, answer)| answer.ready().cloned());
        match held {
            Some(recommendation) => Ok(recommendation),
            None => {
                self.recommend(
                    client,
                    key.champion,
                    key.mode,
                    key.lane,
                    &key.builds,
                    session,
                )
                .await
            }
        }
    }

    /// Sets up `champion`, now that it is the player's: each half from the user's own setup where it
    /// has that half, else the recommendation (`loadout::plan`). A half the player changes while
    /// winer looks the recommendation up, or put up from a panel and still has, stays as it is.
    fn set_up(&self, client: &Client, champion: i64, mode: Mode, game_id: i64) {
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            let Some(session) = holding(&client, game_id, champion) else {
                return;
            };
            let settings = service.settings();
            if !settings.automation.restores_loadout(Some(mode)) {
                return;
            }
            let lane = lane_of(&session, mode);
            let keys = loadout::preset_keys(champion, mode, lane);
            let spells_before = spells_of(&session);
            let page_before = current_page(&client.lcu).await;
            let mine = service.mine(&keys);
            let recommended =
                if loadout::whole(mine.as_ref()) || !settings.automation.loadout.recommended {
                    None
                } else {
                    let key = RecommendationKey {
                        game_id,
                        champion,
                        mode,
                        lane,
                        builds: settings.builds.clone(),
                    };
                    match service.recommendation_for(&client, key, &session).await {
                        Ok(recommendation) => Some(recommendation),
                        Err(error) => {
                            service.notice(NoticeKind::Failed {
                                action: "loadout".into(),
                                message: error.to_string(),
                            });
                            None
                        }
                    }
                };
            let _turn = service.inner.loadout.acting.lock().await;
            // Asked again: meanwhile the champion may have changed, or the rule been turned off.
            let Some(session) = holding(&client, game_id, champion) else {
                return;
            };
            if !service.settings().automation.restores_loadout(Some(mode)) {
                return;
            }
            // A setup saved or deleted meanwhile counts as it is now.
            let mut plan = loadout::plan(service.mine(&keys).as_ref(), recommended.as_ref());
            if plan.is_empty() {
                return;
            }
            let page_now = current_page(&client.lcu).await;
            let (written, asked) = {
                let live = lock(&client.live);
                (
                    live.loadout.written.clone(),
                    live.loadout.asked_for(game_id, champion),
                )
            };
            let mut kept = Vec::new();
            // What the player put up from a panel for this champion, and still has, stays; it is
            // news only where the rule would have put up something else.
            if let Some(asked) = asked {
                let runes_now = page_now.as_ref().and_then(PerkPage::runes);
                let runes = match (&plan.runes, &asked.runes) {
                    (Some((runes, _)), Some(wanted)) if runes_now.as_ref() == Some(wanted) => {
                        Some(runes != wanted)
                    }
                    _ => None,
                };
                if let Some(other) = runes {
                    plan.runes = None;
                    if other {
                        kept.push(LoadoutPart::Runes);
                    }
                }
                let spells = match (plan.spells, asked.spells) {
                    (Some((spells, _)), Some(wanted))
                        if loadout::same_spells(spells_of(&session), wanted) =>
                    {
                        Some(!loadout::same_spells(spells, wanted))
                    }
                    _ => None,
                };
                if let Some(other) = spells {
                    plan.spells = None;
                    if other {
                        kept.push(LoadoutPart::Spells);
                    }
                }
            }
            if plan.runes.is_some()
                && changed_page(page_before.as_ref(), page_now.as_ref(), &written, game_id)
            {
                plan.runes = None;
                kept.push(LoadoutPart::Runes);
            }
            if plan.spells.is_some()
                && changed_spells(spells_before, spells_of(&session), &written, game_id)
            {
                plan.spells = None;
                kept.push(LoadoutPart::Spells);
            }
            let (applied, errors) = service.carry_out(&client, game_id, champion, &plan).await;
            if !errors.is_empty() {
                service.notice(NoticeKind::Failed {
                    action: "loadout".into(),
                    message: errors.join("; "),
                });
            }
            service.refresh_page(&client, game_id).await;
            service.render_current(&client);
            if applied.runes.is_some()
                || applied.spells.is_some()
                || applied.no_page
                || !kept.is_empty()
            {
                info!(champion, ?mode, ?applied, ?kept, "loadout set up");
                service.notice(NoticeKind::LoadoutApplied {
                    champion_id: champion,
                    runes: applied.runes,
                    spells: applied.spells,
                    no_page: applied.no_page,
                    kept,
                });
            }
        });
    }

    /// The user's own setup under the first of `keys` there is one under.
    fn mine(&self, keys: &[String]) -> Option<Setup> {
        self.inner
            .loadout
            .presets
            .find(keys)
            .map(|(_, setup)| setup)
    }

    /// Draws the views from `client`, unless another connection has taken its place: a task of an
    /// ended one must not draw its last champ select back.
    fn render_current(&self, client: &Client) {
        if self.is_current(client) {
            self.render(client);
        }
    }

    /// Plugin contexts connected to the bridge now.
    fn plugins(&self) -> u32 {
        self.inner.loadout.plugins.load(Ordering::Acquire)
    }

    /// A plugin context connected to the bridge. The first one makes the client's runes panel
    /// drawable, so the views are drawn again for it.
    pub(crate) fn plugin_attached(&self) {
        if self.inner.loadout.plugins.fetch_add(1, Ordering::AcqRel) == 0
            && let Ok(client) = self.client()
        {
            self.render(&client);
        }
    }

    /// A plugin context left the bridge; after the last one, nothing is looked up for the panel.
    pub(crate) fn plugin_detached(&self) {
        let _ =
            self.inner
                .loadout
                .plugins
                .try_update(Ordering::AcqRel, Ordering::Acquire, |plugins| {
                    plugins.checked_sub(1)
                });
    }

    /// Puts `plan` on the client for `champion` in champ select `game_id`: the rune page first, then
    /// the spells; a failure of one does not keep the other back. What went up is kept for the
    /// view; returns it, and the failures.
    async fn carry_out(
        &self,
        client: &Client,
        game_id: i64,
        champion: i64,
        plan: &Plan,
    ) -> (AppliedLoadout, Vec<String>) {
        let mut errors = Vec::new();
        let mut applied = AppliedLoadout::default();
        if let Some((runes, origin)) = &plan.runes {
            match self.write_page(client, champion, runes).await {
                Ok(PageOutcome::Written) => applied.runes = Some(*origin),
                Ok(PageOutcome::NoPage) => applied.no_page = true,
                Err(error) => errors.push(error.to_string()),
            }
        }
        if let Some((spells, origin)) = plan.spells {
            match self.set_spells(client, spells).await {
                Ok(()) => applied.spells = Some(origin),
                Err(error) => errors.push(error.to_string()),
            }
        }
        self.note_applied(client, game_id, champion, &applied);
        (applied, errors)
    }

    /// What went up for `champion` in champ select `game_id`, beside what went up before: a request
    /// for one half leaves the other's origin as it was.
    fn note_applied(&self, client: &Client, game_id: i64, champion: i64, now: &AppliedLoadout) {
        let mut live = lock(&client.live);
        let slot = &mut live.loadout.applied;
        let mut applied = match slot.take() {
            Some((game, held, applied)) if game == game_id && held == champion => applied,
            _ => AppliedLoadout::default(),
        };
        if now.runes.is_some() || now.no_page {
            applied.runes = now.runes;
            applied.no_page = now.no_page;
        }
        if now.spells.is_some() {
            applied.spells = now.spells;
        }
        *slot = Some((game_id, champion, applied));
    }

    /// The view `target` names, while it is still the view drawn last and the champ select it was
    /// drawn for: else the request is stale, and nothing is done.
    fn loadout_target(
        &self,
        client: &Client,
        target: &LoadoutTarget,
    ) -> Result<(LoadoutView, ChampSelectSession), CoreError> {
        let (phase, view) = {
            let state = lock(&self.inner.state);
            (
                state.phase,
                state
                    .champ_select
                    .as_ref()
                    .and_then(|view| view.loadout.as_deref().cloned()),
            )
        };
        if phase != Phase::ChampSelect {
            return Err(CoreError::Busy("not in champ select".into()));
        }
        let stale = || CoreError::Stale("champ select has changed since".into());
        let view = view.ok_or_else(stale)?;
        let session = lock(&client.live).champ_select.clone().ok_or_else(stale)?;
        if !loadout::names(target, &view, &session) {
            return Err(stale());
        }
        Ok((view, session))
    }

    /// The champ select `target` names, still under way with its champion in hand and in `lane`:
    /// what a request that had to wait for a lookup checks again in winer's turn. Its view may have
    /// moved on meanwhile (the recommendation came, the page was read), so not its revision.
    fn still_on(
        &self,
        client: &Client,
        target: &LoadoutTarget,
        mode: Mode,
        lane: Option<Position>,
    ) -> Result<ChampSelectSession, CoreError> {
        if lock(&self.inner.state).phase != Phase::ChampSelect {
            return Err(CoreError::Busy("not in champ select".into()));
        }
        lock(&client.live)
            .champ_select
            .clone()
            .filter(|session| {
                session.game_id == target.game_id
                    && loadout::champion_in_hand(session) == Some(target.champion_id)
                    && lane_of(session, mode) == lane
            })
            .ok_or_else(|| CoreError::Stale("champ select has changed since".into()))
    }

    /// What the window's or the client's panel asks for, on the view `target` names: checked before
    /// winer's turn and again in it, so a request made against an older view does nothing. The
    /// runes and spells come from the view, never from the request. The views are drawn again
    /// afterwards, whatever came of it.
    pub async fn loadout_action(
        &self,
        target: LoadoutTarget,
        action: LoadoutAction,
    ) -> Result<LoadoutDone, CoreError> {
        let client = self.client()?;
        let (view, session) = self.loadout_target(&client, &target)?;
        // Back to recommended looks the recommendation up before winer's turn: it can take
        // seconds, and the champion is checked again once it is back.
        let recommended = match action {
            LoadoutAction::RestoreRecommended => {
                let key = RecommendationKey {
                    game_id: view.game_id,
                    champion: view.champion_id,
                    mode: view.mode,
                    lane: view.lane,
                    builds: self.settings().builds,
                };
                Some(self.recommendation_for(&client, key, &session).await?)
            }
            _ => None,
        };
        let _turn = self.inner.loadout.acting.lock().await;
        let done = self
            .act_on(&client, &target, &view, action, recommended)
            .await;
        self.refresh_page(&client, target.game_id).await;
        self.render_current(&client);
        done
    }

    /// [`Self::loadout_action`] in winer's turn.
    async fn act_on(
        &self,
        client: &Client,
        target: &LoadoutTarget,
        asked_on: &LoadoutView,
        action: LoadoutAction,
        recommended: Option<LoadoutRecommendation>,
    ) -> Result<LoadoutDone, CoreError> {
        let stale = || CoreError::Stale("that option is not in the view any more".into());
        let (game_id, champion) = (target.game_id, target.champion_id);
        if let LoadoutAction::RestoreRecommended = action {
            let recommendation = recommended.ok_or_else(stale)?;
            let (mode, lane) = (asked_on.mode, asked_on.lane);
            let session = self.still_on(client, target, mode, lane)?;
            // What the press was made on must still hold: the setup it showed, and the runes and
            // spells the player had then. A save or an apply made meanwhile is theirs to keep.
            let shown = self
                .inner
                .loadout
                .presets
                .find(&loadout::preset_keys(champion, mode, lane));
            let page = current_page(&client.lcu).await;
            if shown.as_ref().map(|(_, setup)| setup) != asked_on.mine.as_ref()
                || page.as_ref().and_then(PerkPage::runes) != asked_on.current.runes
                || !loadout::same_spells(spells_of(&session), asked_on.current.spells)
            {
                return Err(stale());
            }
            // Only the setup the view showed: a lane's own goes, and the one saved with no lane,
            // which every lane falls back to, stays for the next press.
            if let Some((key, _)) = shown {
                let service = self.clone();
                tokio::task::spawn_blocking(move || service.inner.loadout.presets.remove(&[key]))
                    .await
                    .map_err(|error| CoreError::Io(std::io::Error::other(error)))??;
            }
            let plan = loadout::plan(None, Some(&recommendation));
            return if plan.is_empty() {
                Ok(LoadoutDone::default())
            } else {
                self.carry_out_asked(client, game_id, champion, &plan).await
            };
        }
        let (view, session) = self.loadout_target(client, target)?;
        match action {
            LoadoutAction::SaveMine => {
                let page = current_page(&client.lcu).await;
                let setup = loadout::current_setup(page.as_ref(), spells_of(&session), now_ms())
                    .ok_or_else(|| {
                        CoreError::Invalid("no full rune page nor two spells to keep".into())
                    })?;
                let key = loadout::preset_key(champion, view.mode, view.lane);
                let service = self.clone();
                tokio::task::spawn_blocking(move || service.inner.loadout.presets.put(key, setup))
                    .await
                    .map_err(|error| CoreError::Io(std::io::Error::other(error)))??;
                info!(champion, mode = ?view.mode, "the player's setup kept");
                Ok(LoadoutDone::default())
            }
            LoadoutAction::ApplyRunes { index } => {
                let recommendation = view.recommendation.ready().ok_or_else(stale)?;
                let option = recommendation
                    .runes
                    .get(usize::from(index))
                    .ok_or_else(stale)?;
                let origin = recommendation.runes_from.ok_or_else(stale)?.origin();
                let plan = Plan {
                    runes: Some((option.page.clone(), origin)),
                    spells: None,
                };
                self.carry_out_asked(client, game_id, champion, &plan).await
            }
            LoadoutAction::ApplySpells { index } => {
                let recommendation = view.recommendation.ready().ok_or_else(stale)?;
                let option = recommendation
                    .spells
                    .get(usize::from(index))
                    .ok_or_else(stale)?;
                let origin = recommendation.spells_from.ok_or_else(stale)?.origin();
                let plan = Plan {
                    runes: None,
                    spells: Some((option.spells, origin)),
                };
                self.carry_out_asked(client, game_id, champion, &plan).await
            }
            LoadoutAction::ApplyMine => {
                let mine = view.mine.as_ref().ok_or_else(stale)?;
                let plan = loadout::plan(Some(mine), None);
                self.carry_out_asked(client, game_id, champion, &plan).await
            }
            LoadoutAction::RestoreRecommended => unreachable!("handled above"),
        }
    }

    /// [`Self::carry_out`] for a request, whose failure is the answer.
    async fn carry_out_asked(
        &self,
        client: &Client,
        game_id: i64,
        champion: i64,
        plan: &Plan,
    ) -> Result<LoadoutDone, CoreError> {
        let (applied, errors) = self.carry_out(client, game_id, champion, plan).await;
        note_asked(
            client,
            game_id,
            champion,
            plan.runes
                .as_ref()
                .filter(|_| applied.runes.is_some())
                .map(|(runes, _)| runes),
            plan.spells
                .filter(|_| applied.spells.is_some())
                .map(|(spells, _)| spells),
        );
        if let Some(error) = errors.into_iter().next() {
            return Err(CoreError::Invalid(error));
        }
        Ok(LoadoutDone {
            runes: if applied.no_page {
                Some(PageOutcome::NoPage)
            } else {
                applied.runes.map(|_| PageOutcome::Written)
            },
            spells: applied.spells.is_some(),
        })
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
        let mut live = lock(&client.live);
        if let Some(game_id) = live.champ_select.as_ref().map(|session| session.game_id) {
            let written = &mut live.loadout.written;
            if written.game_id != game_id {
                *written = Written {
                    game_id,
                    ..Written::default()
                };
            }
            written.page = Some((id, runes.clone()));
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
        let mut live = lock(&client.live);
        if let Some(game_id) = live.champ_select.as_ref().map(|session| session.game_id) {
            let written = &mut live.loadout.written;
            if written.game_id != game_id {
                *written = Written {
                    game_id,
                    ..Written::default()
                };
            }
            written.spells = Some(spells);
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
                Err(CoreError::Full(_)) => service.notice(NoticeKind::ItemSetsFull),
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
        client
            .lcu
            .put(&path, &sets)
            .await
            .map_err(item_sets_refused)?;
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
            let key = (source, query.champion_id, query.mode, query.lane, patch);
            if !cache.contains_key(&key) {
                make_room(&mut cache, Instant::now());
            }
            cache.entry(key).or_default().clone()
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
        let build = self.build_with(&settings, champion_id, mode, lane).await?;
        Ok((*build).clone())
    }

    /// [`Self::build`] by the sources `settings` name, as passed rather than as saved now: a
    /// lookup started under one source answers for that source.
    pub(crate) async fn build_with(
        &self,
        settings: &BuildSettings,
        champion_id: i64,
        mode: Mode,
        lane: Option<Position>,
    ) -> Result<Arc<Build>, CoreError> {
        if !settings.enabled {
            return Err(CoreError::Invalid("the build panel is switched off".into()));
        }
        if champion_id <= 0 {
            return Err(CoreError::Invalid(format!("no champion {champion_id}")));
        }
        let sources = builds::sources(mode, settings);
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
                Ok(build) => return Ok(build),
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
        let outcome = self.write_page(&client, champion_id, &runes).await?;
        // The player's own choice for the champion in hand: the rule leaves it at lock-in.
        if outcome == PageOutcome::Written
            && let Some((game_id, champion)) = in_hand(&client)
            && champion == champion_id
        {
            note_asked(&client, game_id, champion, Some(&runes), None);
        }
        Ok(outcome)
    }

    /// Takes `spells` in the champ select under way, on the keys the player already uses for them.
    pub async fn apply_spells(&self, spells: [i64; 2]) -> Result<(), CoreError> {
        let client = self.client()?;
        let _turn = self.inner.loadout.acting.lock().await;
        self.set_spells(&client, spells).await?;
        if let Some((game_id, champion)) = in_hand(&client) {
            note_asked(&client, game_id, champion, None, Some(spells));
        }
        Ok(())
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

    /// The user's own setups and the old file's, for the automation page. Reads the old file:
    /// call it off the async runtime.
    pub fn loadout_summary(&self) -> LoadoutSummary {
        LoadoutSummary {
            presets: u32::try_from(self.inner.loadout.presets.len()).unwrap_or(u32::MAX),
            legacy: loadout::legacy_count(&self.inner.loadout.legacy),
        }
    }

    /// Forgets every setup of the user's own. Blocking: the file is written before it returns.
    pub fn clear_loadout_presets(&self) -> Result<LoadoutSummary, CoreError> {
        self.inner.loadout.presets.clear()?;
        // A view still showing one would let a request apply what is gone.
        if let Ok(client) = self.client() {
            self.render(&client);
        }
        Ok(self.loadout_summary())
    }

    /// Deletes `loadouts.json`, the setups winer up to 0.0.24 remembered by itself. Blocking.
    pub fn clear_loadouts(&self) -> Result<LoadoutSummary, CoreError> {
        loadout::remove_legacy(&self.inner.loadout.legacy)?;
        Ok(self.loadout_summary())
    }

    /// Where the user's own setups are kept, for the storage report.
    pub fn loadout_presets_path(&self) -> PathBuf {
        self.inner.loadout.presets.path().to_path_buf()
    }
}

/// The client's recommended pages for the champion, position and map; none for a map it has none
/// for, or a kind of game whose map is not known.
async fn client_pages(
    lcu: &Lcu,
    champion: i64,
    mode: Mode,
    session: &ChampSelectSession,
) -> Result<Vec<RecommendedPage>, CoreError> {
    let Some(map) = loadout::recommendation_map(mode) else {
        return Ok(Vec::new());
    };
    let position = loadout::recommendation_position(session);
    let pages: Option<Vec<RecommendedPage>> = lcu
        .get_optional(&format!(
            "/lol-perks/v1/recommended-pages/champion/{champion}/position/{position}/map/{map}"
        ))
        .await?;
    Ok(pages.unwrap_or_default())
}

/// The local player's item sets as the client keeps them, and where they are written back.
/// The client answers 413 to a list of item sets past the size it takes (64,881 bytes went, about
/// 66 KB did not, 2026-10-31). winer's own older sets have gone by then, so the player's fill it.
fn item_sets_refused(error: lcu::Error) -> CoreError {
    if error.status() == Some(413) {
        CoreError::Full(format!("the client's item sets are full: {error}"))
    } else {
        error.into()
    }
}

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

// ---- Storage: the sweep and the cleanup (`service/caches.rs`) ----

/// Lets the lookups go that are past [`BUILD_TTL`] or never got an answer, keeping those being
/// fetched. Returns how many went.
fn sweep_builds(cache: &mut HashMap<BuildKey, BuildSlot>, now: Instant) -> usize {
    let before = cache.len();
    cache.retain(|_, slot| {
        slot.try_lock().map_or(true, |entry| {
            entry
                .as_ref()
                .is_some_and(|(at, _)| now.saturating_duration_since(*at) < BUILD_TTL)
        })
    });
    before - cache.len()
}

/// Makes room for one more lookup within [`BUILD_ENTRIES`]: the stale ones go, then the oldest.
fn make_room(cache: &mut HashMap<BuildKey, BuildSlot>, now: Instant) {
    if cache.len() < BUILD_ENTRIES {
        return;
    }
    sweep_builds(cache, now);
    while cache.len() >= BUILD_ENTRIES {
        let oldest = cache
            .iter()
            .filter_map(|(key, slot)| {
                let at = slot.try_lock().ok()?.as_ref().map(|(at, _)| *at);
                Some((at, key))
            })
            .min_by_key(|(at, _)| *at)
            .map(|(_, key)| key.clone());
        // Every one is being fetched: they are few, and each ends in an answer.
        let Some(oldest) = oldest else { break };
        cache.remove(&oldest);
    }
}

impl LoadoutState {
    /// Numbers past their time, and Tencent's patch list once it is that old. Returns how many
    /// went.
    pub(super) fn sweep(&self, now: Instant) -> usize {
        let mut went = sweep_builds(&mut lock(&self.builds), now);
        if let Ok(mut patches) = self.patches.try_lock()
            && patches
                .as_ref()
                .is_some_and(|(at, _)| now.saturating_duration_since(*at) >= BUILD_TTL)
        {
            *patches = None;
            went += 1;
        }
        went
    }

    /// Lets go of every number, the patch list and what was read from the catalog, keeping only
    /// what is being fetched. Returns how many entries went.
    pub(super) fn clear(&self) -> usize {
        let mut went = {
            let mut builds = lock(&self.builds);
            let before = builds.len();
            builds.retain(|_, slot| slot.try_lock().is_err());
            before - builds.len()
        };
        if let Ok(mut patches) = self.patches.try_lock()
            && patches.take().is_some()
        {
            went += 1;
        }
        if lock(&self.known).take().is_some() {
            went += 1;
        }
        went
    }

    /// Lookups, the patch list and the catalog's reading kept now.
    pub(super) fn entries(&self) -> usize {
        let patches = self
            .patches
            .try_lock()
            .map_or(0, |patches| usize::from(patches.is_some()));
        lock(&self.builds).len() + patches + usize::from(lock(&self.known).is_some())
    }
}

#[cfg(test)]
mod tests {
    use lcu::Credentials;
    use tokio::runtime::Handle;

    use super::*;
    use crate::{
        test_support::fixture,
        view::{Event, GameData, QueueInfo},
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
    async fn nothing_is_remembered_when_a_game_starts_and_the_old_file_is_never_opened() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("loadouts.json");
        std::fs::write(&old, b"{ not json").unwrap();
        let (service, client) = (service(&dir), client(&[3120]));
        let mut settings = service.settings();
        settings.automation.loadout.enabled = true;
        service.set_settings(settings).unwrap();
        service.loadout_champ_select(&client, &locked());
        service.loadout_phase(&client, Phase::GameStart);
        assert_eq!(
            service.loadout_summary(),
            LoadoutSummary {
                presets: 0,
                legacy: Some(0)
            },
            "an unreadable old file counts none"
        );
        assert!(old.exists(), "not moved aside");
        assert!(!old.with_extension("json.invalid").exists());
        assert!(!dir.path().join("loadout-presets.json").exists());
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
    async fn the_users_setups_live_beside_the_settings_and_the_old_file_goes_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("loadouts.json");
        std::fs::write(
            &old,
            br#"{"202:ranked":{"runes":null,"spells":[4,7]},"22:aram":{"runes":null,"spells":[4,32]}}"#,
        )
        .unwrap();
        let service = service(&dir);
        assert_eq!(
            service.loadout_summary(),
            LoadoutSummary {
                presets: 0,
                legacy: Some(2)
            }
        );
        assert_eq!(service.clear_loadouts().unwrap().legacy, None);
        assert!(!old.exists());
        assert_eq!(service.clear_loadout_presets().unwrap().presets, 0);
        assert_eq!(
            service.loadout_presets_path(),
            dir.path().join("loadout-presets.json")
        );
    }

    /// Champ select 9 on the Rift, the player in cell 0 holding `champion`, with `spells`.
    fn rift(champion: i64, spells: [i64; 2]) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "gameId": 9,
            "queueId": 420,
            "localPlayerCellId": 0,
            "myTeam": [{
                "cellId": 0,
                "championId": champion,
                "assignedPosition": "bottom",
                "spell1Id": spells[0],
                "spell2Id": spells[1]
            }],
            "actions": [[{"id": 1, "actorCellId": 0, "type": "pick", "completed": true, "championId": champion}]]
        }))
        .unwrap()
    }

    /// A port on this machine that takes each connection and closes it at once, so every request
    /// to it fails at once: a connect to a port nothing listens on keeps Windows waiting about two
    /// seconds, and a set-up makes a few of them.
    fn closing_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                drop(stream);
            }
        });
        port
    }

    /// A service in champ select `session` on a client that answers nothing, the view drawn.
    fn in_champ_select(dir: &tempfile::TempDir, session: &ChampSelectSession) -> (Service, Client) {
        let service = service(dir);
        let mut settings = service.settings();
        // Nothing to look up in a test, on the internet or in the client: no recommendation is
        // asked for, and no page read in the background.
        settings.builds.enabled = false;
        settings.plugin.loadout_panel = false;
        service.set_settings(settings).unwrap();
        let credentials = Credentials::new(closing_port(), "t");
        let data = GameData {
            queues: vec![QueueInfo {
                id: 420,
                name: "单双排".into(),
                game_mode: "CLASSIC".into(),
                ranked: true,
            }],
            ..GameData::default()
        };
        let client = Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::new(Mutex::new(Some(Arc::new(data)))),
            live: Arc::default(),
        };
        {
            let mut live = lock(&client.live);
            live.phase = Phase::ChampSelect;
            live.champ_select = Some(session.clone());
        }
        *service
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(client.clone());
        lock(&service.inner.state).phase = Phase::ChampSelect;
        let view = service.loadout_view(&client, session, Some(Mode::Ranked), &service.settings());
        show(&service, session, view);
        (service, client)
    }

    /// Puts `view` in the snapshot as champ select's, as `render` does.
    fn show(service: &Service, session: &ChampSelectSession, view: Option<LoadoutView>) {
        let mut state = lock(&service.inner.state);
        state.champ_select = Some(crate::view::ChampSelectView {
            game_id: session.game_id,
            queue_id: session.queue_id,
            timer: crate::view::TimerView::default(),
            my_team: Vec::new(),
            their_team: Vec::new(),
            my_bans: Vec::new(),
            their_bans: Vec::new(),
            bench_enabled: false,
            bench: Vec::new(),
            rerolls_remaining: 0,
            callout: Vec::new(),
            side: None,
            recommendations: Vec::new(),
            loadout: view.map(Box::new),
        });
    }

    fn target(view: &LoadoutView) -> LoadoutTarget {
        LoadoutTarget {
            game_id: view.game_id,
            champion_id: view.champion_id,
            revision: view.revision,
        }
    }

    fn drawn(service: &Service) -> LoadoutView {
        lock(&service.inner.state)
            .champ_select
            .as_ref()
            .and_then(|view| view.loadout.as_deref().cloned())
            .expect("a loadout view")
    }

    #[tokio::test]
    async fn the_view_names_the_champion_its_lane_and_a_revision_that_follows_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        let view = drawn(&service);
        assert_eq!(
            (
                view.game_id,
                view.champion_id,
                view.mode,
                view.lane,
                view.held
            ),
            (9, 202, Mode::Ranked, Some(Position::Bottom), true)
        );
        assert_eq!(view.current.spells, [4, 7]);
        assert!(view.revision > 0);
        // Drawn again unchanged, it keeps its revision; another lane takes a new one.
        let settings = service.settings();
        let again = service.loadout_view(&client, &session, Some(Mode::Ranked), &settings);
        assert_eq!(
            again.as_ref().map(|view| view.revision),
            Some(view.revision)
        );
        let mut top = session.clone();
        top.my_team[0].assigned_position = "top".into();
        lock(&client.live).champ_select = Some(top.clone());
        let moved = service
            .loadout_view(&client, &top, Some(Mode::Ranked), &settings)
            .unwrap();
        assert_eq!(moved.lane, Some(Position::Top));
        assert_ne!(moved.revision, view.revision, "only the lane changed");
        // Arena has no runes to choose; nobody holds a champion before one is hovered.
        assert!(
            service
                .loadout_view(&client, &top, Some(Mode::Arena), &settings)
                .is_none()
        );
        let empty = rift(0, [4, 7]);
        assert!(
            service
                .loadout_view(&client, &empty, Some(Mode::Ranked), &settings)
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_request_made_against_an_older_view_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        let stale =
            |result: Result<LoadoutDone, CoreError>| matches!(result, Err(CoreError::Stale(_)));
        let kept = || service.loadout_summary().presets;

        // The view as drawn: carried out. The page could not be read from this client, so the
        // spells alone are kept as the player's own setup, and the view is drawn again with it.
        let view = drawn(&service);
        let saved = service
            .loadout_action(target(&view), LoadoutAction::SaveMine)
            .await;
        assert_eq!(saved.unwrap(), LoadoutDone::default());
        assert_eq!(kept(), 1);
        let view = drawn(&service);
        assert_eq!(
            view.mine.as_ref().and_then(|mine| mine.spells),
            Some([4, 7])
        );
        assert!(
            stale(
                service
                    .loadout_action(target(&view), LoadoutAction::ApplyRunes { index: 0 })
                    .await
            ),
            "no recommendation in the view to apply"
        );

        // Another champ select, another champion, an older revision.
        for wrong in [
            LoadoutTarget {
                game_id: 8,
                ..target(&view)
            },
            LoadoutTarget {
                champion_id: 22,
                ..target(&view)
            },
            LoadoutTarget {
                revision: view.revision.wrapping_sub(1),
                ..target(&view)
            },
        ] {
            assert!(stale(
                service
                    .loadout_action(wrong, LoadoutAction::RestoreRecommended)
                    .await
            ));
        }
        assert_eq!(kept(), 1, "nothing was taken out");

        // The lane changed after the panel was drawn: its revision names the view before.
        let mut top = session.clone();
        top.my_team[0].assigned_position = "top".into();
        lock(&client.live).champ_select = Some(top.clone());
        let redrawn = service.loadout_view(&client, &top, Some(Mode::Ranked), &service.settings());
        show(&service, &top, redrawn);
        assert!(stale(
            service
                .loadout_action(target(&view), LoadoutAction::SaveMine)
                .await
        ));
        assert_eq!(kept(), 1, "nothing kept for the other lane");

        // The champion swapped in the client before the view was drawn again.
        let now = drawn(&service);
        lock(&client.live).champ_select = Some(rift(22, [4, 7]));
        assert!(stale(
            service
                .loadout_action(target(&now), LoadoutAction::SaveMine)
                .await
        ));
        assert_eq!(kept(), 1);

        // Outside champ select nothing is asked of the client at all.
        lock(&client.live).champ_select = Some(top);
        lock(&service.inner.state).phase = Phase::InProgress;
        assert!(matches!(
            service
                .loadout_action(target(&now), LoadoutAction::SaveMine)
                .await,
            Err(CoreError::Busy(_))
        ));
        assert_eq!(kept(), 1);
    }

    #[tokio::test]
    async fn a_setup_of_the_users_own_shows_in_the_view_for_its_lane_then_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        assert_eq!(drawn(&service).mine, None);
        let setup = Setup {
            runes: None,
            spells: Some([4, 6]),
            saved_at: 1,
        };
        service
            .inner
            .loadout
            .presets
            .put(loadout::preset_key(202, Mode::Ranked, None), setup.clone())
            .unwrap();
        let settings = service.settings();
        let view = service
            .loadout_view(&client, &session, Some(Mode::Ranked), &settings)
            .unwrap();
        assert_eq!(
            view.mine.as_ref(),
            Some(&setup),
            "the one without a lane, for bottom too"
        );
        let lane = Setup {
            spells: Some([4, 21]),
            ..setup
        };
        service
            .inner
            .loadout
            .presets
            .put(
                loadout::preset_key(202, Mode::Ranked, Some(Position::Bottom)),
                lane.clone(),
            )
            .unwrap();
        let view = service
            .loadout_view(&client, &session, Some(Mode::Ranked), &settings)
            .unwrap();
        assert_eq!(view.mine, Some(lane), "the lane's own first");
        assert_eq!(
            service
                .loadout_view(&client, &session, Some(Mode::Normal), &settings)
                .unwrap()
                .mine,
            None,
            "ranked and normal games are kept apart"
        );
    }

    /// The rule on for ranked games, with an empty recommendation in hand for `session`'s champion
    /// first, so the views drawn as the rule goes on look nothing up.
    fn rule_on(service: &Service, client: &Client, session: &ChampSelectSession) {
        let key = RecommendationKey {
            game_id: session.game_id,
            champion: loadout::champion_in_hand(session).unwrap(),
            mode: Mode::Ranked,
            lane: lane_of(session, Mode::Ranked),
            builds: service.settings().builds,
        };
        lock(&client.live).loadout.recommendation = Some((
            key,
            RecommendationState::Ready {
                recommendation: LoadoutRecommendation::default(),
            },
        ));
        let mut settings = service.settings();
        settings.automation.loadout.enabled = true;
        settings.automation.scopes.loadout = vec![Mode::Ranked];
        service.set_settings(settings).unwrap();
    }

    #[tokio::test]
    async fn back_to_recommended_takes_out_only_the_setup_shown_and_checks_the_champion_again() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        rule_on(&service, &client, &session);
        let bare = Setup {
            runes: None,
            spells: Some([4, 6]),
            saved_at: 1,
        };
        let lane = Setup {
            spells: Some([4, 21]),
            ..bare.clone()
        };
        let presets = &service.inner.loadout.presets;
        presets
            .put(loadout::preset_key(202, Mode::Ranked, None), bare.clone())
            .unwrap();
        presets
            .put(
                loadout::preset_key(202, Mode::Ranked, Some(Position::Bottom)),
                lane.clone(),
            )
            .unwrap();
        service.render(&client);
        let view = drawn(&service);
        assert_eq!(view.mine.as_ref(), Some(&lane));

        // The champion changed while the recommendation was looked up: in winer's turn nothing
        // is done.
        lock(&client.live).champ_select = Some(rift(22, [4, 7]));
        let done = service
            .act_on(
                &client,
                &target(&view),
                &view,
                LoadoutAction::RestoreRecommended,
                Some(LoadoutRecommendation::default()),
            )
            .await;
        assert!(matches!(done, Err(CoreError::Stale(_))));
        assert_eq!(presets.len(), 2, "nothing taken out");

        // Pressed on the view: the lane's own goes, the one every lane falls back to stays, and
        // the views are drawn again with it.
        lock(&client.live).champ_select = Some(session.clone());
        let done = service
            .loadout_action(target(&view), LoadoutAction::RestoreRecommended)
            .await;
        assert_eq!(done.unwrap(), LoadoutDone::default());
        assert_eq!(presets.len(), 1);
        let shown = drawn(&service);
        assert_eq!(shown.mine, Some(bare));

        // Cleared from the automation page: drawn again too, so a request made on the view that
        // still showed a setup applies nothing.
        service.clear_loadout_presets().unwrap();
        let cleared = drawn(&service);
        assert_eq!(cleared.mine, None);
        assert_ne!(cleared.revision, shown.revision);
        assert!(matches!(
            service
                .loadout_action(target(&shown), LoadoutAction::ApplyMine)
                .await,
            Err(CoreError::Stale(_))
        ));
    }

    #[tokio::test]
    async fn back_to_recommended_undoes_nothing_done_while_it_looked_up() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        rule_on(&service, &client, &session);
        let presets = &service.inner.loadout.presets;
        let key = loadout::preset_key(202, Mode::Ranked, Some(Position::Bottom));
        let first = Setup {
            runes: None,
            spells: Some([4, 6]),
            saved_at: 1,
        };
        presets.put(key.clone(), first).unwrap();
        service.render(&client);
        let view = drawn(&service);
        // Saved again from the other panel while the recommendation was looked up.
        let again = Setup {
            runes: None,
            spells: Some([4, 7]),
            saved_at: 2,
        };
        presets.put(key, again.clone()).unwrap();
        let done = service
            .act_on(
                &client,
                &target(&view),
                &view,
                LoadoutAction::RestoreRecommended,
                Some(LoadoutRecommendation::default()),
            )
            .await;
        assert!(matches!(done, Err(CoreError::Stale(_))));
        assert_eq!(
            service.mine(&loadout::preset_keys(
                202,
                Mode::Ranked,
                Some(Position::Bottom)
            )),
            Some(again),
            "the save made meanwhile stays"
        );
        // Spells changed meanwhile, the setup as shown: nothing is done either.
        service.render(&client);
        let view = drawn(&service);
        lock(&client.live).champ_select = Some(rift(202, [4, 21]));
        let done = service
            .act_on(
                &client,
                &target(&view),
                &view,
                LoadoutAction::RestoreRecommended,
                Some(LoadoutRecommendation::default()),
            )
            .await;
        assert!(matches!(done, Err(CoreError::Stale(_))));
        assert_eq!(presets.len(), 1);
    }

    #[tokio::test]
    async fn the_rule_leaves_what_the_player_put_up_from_a_panel() {
        let dir = tempfile::tempdir().unwrap();
        // Barrier taken from the panel's second spell pair while the champion was hovered.
        let session = rift(202, [4, 21]);
        let (service, client) = in_champ_select(&dir, &session);
        rule_on(&service, &client, &session);
        service
            .inner
            .loadout
            .presets
            .put(
                loadout::preset_key(202, Mode::Ranked, Some(Position::Bottom)),
                Setup {
                    runes: None,
                    spells: Some([4, 6]),
                    saved_at: 1,
                },
            )
            .unwrap();
        lock(&client.live).loadout.asked = Some((
            9,
            202,
            Asked {
                runes: None,
                spells: Some([21, 4]),
            },
        ));
        let mut events = service.subscribe();
        service.set_up(&client, 202, Mode::Ranked, 9);
        let notice = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Ok(Event::Notice(notice)) = events.recv().await
                    && matches!(
                        notice.kind,
                        NoticeKind::LoadoutApplied { .. } | NoticeKind::Failed { .. }
                    )
                {
                    break notice.kind;
                }
            }
        })
        .await
        .expect("the set-up says what it did");
        assert_eq!(
            notice,
            NoticeKind::LoadoutApplied {
                champion_id: 202,
                runes: None,
                spells: None,
                no_page: false,
                kept: vec![LoadoutPart::Spells],
            },
            "nothing asked of the client, which would have failed"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_page_poll_ends_with_its_connection() {
        let dir = tempfile::tempdir().unwrap();
        let session = rift(202, [4, 7]);
        let (service, client) = in_champ_select(&dir, &session);
        rule_on(&service, &client, &session);
        service.poll_pages(&client, 9);
        assert_eq!(lock(&client.live).loadout.polling, Some(9));
        // The connection ends: the client goes, its phase stays as it was, and the snapshot says
        // there is no champ select.
        *service
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        lock(&service.inner.state).champ_select = None;
        tokio::time::timeout(Duration::from_secs(10), async {
            while lock(&client.live).loadout.polling.is_some() {
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("the poll ended");
        assert_eq!(
            lock(&service.inner.state).champ_select,
            None,
            "the champ select that ended is not drawn back"
        );
    }

    #[test]
    fn the_panel_wants_the_recommendation_only_while_a_plugin_is_connected() {
        let mut settings = Settings::default();
        settings.plugin.loadout_panel = true;
        assert!(!wants_loadout(&settings, Mode::Ranked, 0));
        assert!(wants_loadout(&settings, Mode::Ranked, 2));
        assert!(
            !wants_loadout(&settings, Mode::Arena, 2),
            "no runes to choose"
        );
        settings.plugin.loadout_panel = false;
        assert!(!wants_loadout(&settings, Mode::Ranked, 2));
        settings.automation.loadout.enabled = true;
        settings.automation.scopes.loadout = vec![Mode::Ranked];
        assert!(wants_loadout(&settings, Mode::Ranked, 0), "the rule's own");
    }

    #[tokio::test]
    async fn plugin_contexts_are_counted_as_they_come_and_go() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(&dir);
        service.plugin_attached();
        service.plugin_attached();
        assert_eq!(service.plugins(), 2);
        for _ in 0..3 {
            service.plugin_detached();
        }
        assert_eq!(service.plugins(), 0, "never below none");
    }

    #[test]
    fn a_change_the_player_made_is_told_from_winers_own() {
        let page: PerkPage = fixture("loadout/perks-currentpage.json");
        let mut theirs = page.clone();
        theirs.selected_perk_ids[0] = 8010;
        let none = Written::default();
        assert!(!changed_page(Some(&page), Some(&page), &none, 9));
        assert!(changed_page(Some(&page), Some(&theirs), &none, 9));
        assert!(
            !changed_page(None, Some(&theirs), &none, 9),
            "not read before"
        );
        let ours = Written {
            game_id: 9,
            page: Some((theirs.id, theirs.runes().unwrap())),
            spells: Some([4, 6]),
        };
        assert!(
            !changed_page(Some(&page), Some(&theirs), &ours, 9),
            "winer wrote it"
        );
        assert!(
            changed_page(Some(&page), Some(&theirs), &ours, 10),
            "in another champ select"
        );
        assert!(!changed_spells([4, 7], [7, 4], &none, 9), "the same two");
        assert!(changed_spells([4, 7], [4, 21], &none, 9));
        assert!(!changed_spells([4, 7], [6, 4], &ours, 9), "winer set them");
    }

    // Storage.

    /// A lookup answered at `at`, or never answered (`None`).
    fn slot(at: Option<Instant>) -> BuildSlot {
        let build = Arc::new(Build::new(BuildSource::OpGg, 1, Mode::Ranked));
        Arc::new(tokio::sync::Mutex::new(at.map(|at| (at, build))))
    }

    fn key(champion: i64) -> BuildKey {
        (
            BuildSource::OpGg,
            champion,
            Mode::Ranked,
            None,
            String::new(),
        )
    }

    #[test]
    fn lookups_stay_within_their_cap_the_stale_going_first_then_the_oldest() {
        let start = Instant::now() + BUILD_TTL;
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let mut cache = HashMap::new();
        for champion in 0..BUILD_ENTRIES as i64 - 1 {
            cache.insert(key(champion), slot(Some(at(champion as u64))));
        }
        // Being fetched, and older than any: it stays whatever happens.
        let fetching = slot(Some(start - BUILD_TTL));
        let _held = fetching.try_lock().unwrap();
        cache.insert(key(-1), fetching.clone());

        make_room(&mut cache, at(300));
        assert_eq!(cache.len(), BUILD_ENTRIES - 1);
        assert!(!cache.contains_key(&key(0)), "the oldest answer went");
        assert!(cache.contains_key(&key(-1)) && cache.contains_key(&key(1)));

        // A stale answer and one that never came go before any fresh one.
        cache.insert(key(5), slot(Some(start - BUILD_TTL)));
        cache.insert(key(6), slot(None));
        cache.insert(key(1000), slot(Some(at(300))));
        make_room(&mut cache, at(300));
        assert_eq!(cache.len(), BUILD_ENTRIES - 2);
        assert!(!cache.contains_key(&key(5)) && !cache.contains_key(&key(6)));
        assert!(cache.contains_key(&key(1)), "no fresh answer had to go");

        assert_eq!(
            sweep_builds(&mut cache, at(300) + BUILD_TTL),
            BUILD_ENTRIES - 3,
            "every answer is past its time but the one being fetched"
        );
        assert!(cache.contains_key(&key(-1)));
    }

    #[test]
    fn a_cleanup_lets_go_of_every_answer_but_one_being_fetched() {
        let dir = tempfile::tempdir().unwrap();
        let state = LoadoutState::new(
            dir.path().join("loadout-presets.json"),
            dir.path().join("loadouts.json"),
        );
        let now = Instant::now();
        let fetching = slot(None);
        let _held = fetching.try_lock().unwrap();
        {
            let mut builds = lock(&state.builds);
            builds.insert(key(1), slot(Some(now)));
            builds.insert(key(2), slot(Some(now)));
            builds.insert(key(3), fetching.clone());
        }
        *state.patches.try_lock().unwrap() = Some((now, Arc::new(vec!["16.19".into()])));
        assert_eq!(state.entries(), 4);
        assert_eq!(state.sweep(now), 0, "nothing is past its time");
        assert_eq!(state.clear(), 3, "two answers and the patch list");
        assert_eq!(state.entries(), 1);
        assert!(lock(&state.builds).contains_key(&key(3)));
    }

    #[test]
    fn a_list_of_item_sets_the_client_will_not_take_is_full_and_says_so() {
        let refused = |status| lcu::Error::Status {
            method: Method::PUT,
            path: "/lol-item-sets/v1/item-sets/1/sets".into(),
            status,
            message: "Content length is too large".into(),
        };
        let full = item_sets_refused(refused(413));
        assert!(matches!(full, CoreError::Full(_)), "{full:?}");
        assert_eq!(
            crate::view::IpcError::from(full).code,
            crate::view::ErrorCode::Full
        );
        assert!(matches!(
            item_sets_refused(refused(500)),
            CoreError::Lcu(error) if error.status() == Some(500)
        ));
    }
}
