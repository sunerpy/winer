//! A player's history as the window and the plugin page through it: the cache in front of the
//! shard's match-history server and the client, and a player rated on their own.
//!
//! What the cache keeps belongs to the account signed in when it was fetched
//! ([`HistoryCache::scope`]). Another account sees none of it: what one account is shown of a
//! private profile, or of itself, is not another's to see.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    callout::{self, Ranking},
    model::Game,
    rating,
    view::{HistorySource, PlayerProfile, PlayerSummary, SeatRating},
};

/// How long the newest page of a player's games is reused. A game, remakes included, lasts longer
/// than this, so a new one shows at most this long after it was recorded; the games of a player
/// who has just finished one are asked again at once ([`HistoryCache::expire_newest`]).
pub const NEWEST_TTL: Duration = Duration::from_secs(90);
/// Older pages: a finished game never changes. They go as soon as a newest page starts with games
/// that were not there, which moves every older game down.
pub const OLDER_TTL: Duration = Duration::from_secs(30 * 60);
/// A player looked up by Riot ID.
pub const FOUND_TTL: Duration = Duration::from_secs(5 * 60);
/// Players whose pages are kept, the most recently viewed.
const PLAYERS: usize = 16;
/// Games kept whole for scoreboards, and entries kept in pages across players, the player being
/// viewed included: a parsed game of ten is a few kilobytes, so this stays at a few megabytes.
const GAMES: usize = 600;
/// Riot IDs looked up.
const FOUND: usize = 64;

/// One page of a player's history: an entry per place, `None` where the source listed a game it
/// never recorded (`sgp`).
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub entries: Vec<Option<Arc<Game>>>,
    /// The source filled the page, so more may follow.
    pub more: bool,
    pub source: HistorySource,
}

struct Kept {
    begin: u32,
    page: Page,
    fetched: Instant,
    /// A newest page whose player has finished a game since: it is asked for again, and still
    /// tells whether the history has moved.
    due: bool,
}

impl Kept {
    /// The newest games only within [`NEWEST_TTL`] (and not after a game ended); older ones within
    /// [`OLDER_TTL`], from whichever page holds them.
    fn fresh(&self, begin: u32, now: Instant) -> bool {
        let age = now.saturating_duration_since(self.fetched);
        if begin == 0 {
            !self.due && age < NEWEST_TTL
        } else {
            age < OLDER_TTL
        }
    }

    /// The `count` entries from the `begin`-th, where this page has them all, or has what there is
    /// because it was the last.
    fn slice(&self, begin: u32, count: u32) -> Option<Page> {
        let (start, len) = (self.begin as usize, self.page.entries.len());
        let (begin, end) = (begin as usize, begin as usize + count as usize);
        if begin < start || (self.page.more && end > start + len) {
            return None;
        }
        let (from, to) = (begin.min(start + len), end.min(start + len));
        Some(Page {
            entries: self.page.entries[from - start..to - start].to_vec(),
            more: end < start + len || self.page.more,
            source: self.page.source,
        })
    }
}

#[derive(Default)]
struct Player {
    pages: Vec<Kept>,
    used: Option<Instant>,
}

/// Pages of players' histories, whole games and Riot ID lookups, for one signed-in account.
#[derive(Default)]
pub struct HistoryCache {
    viewer: String,
    players: HashMap<String, Player>,
    /// Each game with when it was last kept.
    games: HashMap<i64, (Arc<Game>, Instant)>,
    /// `games`' ids, oldest kept first.
    order: VecDeque<i64>,
    found: HashMap<String, (PlayerProfile, Instant)>,
}

impl HistoryCache {
    /// The account the cache holds data for; empty before the first.
    pub fn viewer(&self) -> &str {
        &self.viewer
    }

    /// Makes the cache `viewer`'s, emptying it when it was another account's. Returns whether it
    /// did.
    pub fn scope(&mut self, viewer: &str) -> bool {
        if self.viewer == viewer {
            return false;
        }
        *self = Self {
            viewer: viewer.to_owned(),
            ..Self::default()
        };
        true
    }

    /// `count` entries of `puuid`'s history from the `begin`-th newest, if a page kept for
    /// `viewer` holds them and is fresh enough.
    pub fn page(
        &mut self,
        viewer: &str,
        puuid: &str,
        begin: u32,
        count: u32,
        now: Instant,
    ) -> Option<Page> {
        self.scope(viewer);
        let player = self.players.get_mut(puuid)?;
        let page = player
            .pages
            .iter()
            .filter(|kept| kept.fresh(begin, now))
            .find_map(|kept| kept.slice(begin, count))?;
        player.used = Some(now);
        Some(page)
    }

    /// Keeps a page fetched for `viewer`, and the server's games in it whole, for their
    /// scoreboards. A newest page that starts with other games than the last one means new games
    /// arrived and moved every older one down: the player's older pages go. One that starts with
    /// the same games is the same history; the longer of the two stays, fresh again. A page
    /// fetched for an account that has since signed out is dropped.
    pub fn put_page(&mut self, viewer: &str, puuid: &str, begin: u32, page: Page, now: Instant) {
        if self.viewer != viewer {
            return;
        }
        if page.source == HistorySource::Server {
            for game in page.entries.iter().flatten() {
                self.keep_game(game.clone(), now);
            }
        }
        let player = self.players.entry(puuid.to_owned()).or_default();
        player.used = Some(now);
        let kept = Kept {
            begin,
            page,
            fetched: now,
            due: false,
        };
        if begin > 0 {
            player.pages.retain(|old| old.begin != begin);
            player.pages.push(kept);
        } else {
            match player.pages.iter_mut().find(|old| old.begin == 0) {
                Some(old) if same_start(&old.page.entries, &kept.page.entries) => {
                    if kept.page.entries.len() >= old.page.entries.len() {
                        *old = kept;
                    } else {
                        old.fetched = now;
                        old.due = false;
                    }
                }
                _ => {
                    player.pages.clear();
                    player.pages.push(kept);
                }
            }
        }
        self.trim(puuid);
    }

    /// A finished game, whole, as a page of the server's or the client's own copy brought it.
    pub fn game(&mut self, viewer: &str, game_id: i64) -> Option<Arc<Game>> {
        self.scope(viewer);
        self.games.get(&game_id).map(|(game, _)| game.clone())
    }

    pub fn put_game(&mut self, viewer: &str, game: Arc<Game>, now: Instant) {
        if self.viewer == viewer {
            self.keep_game(game, now);
        }
    }

    /// The player `riot_id` names, looked up for `viewer` within [`FOUND_TTL`].
    pub fn found(&mut self, viewer: &str, riot_id: &str, now: Instant) -> Option<PlayerProfile> {
        self.scope(viewer);
        let (profile, at) = self.found.get(&riot_key(riot_id))?;
        (now.saturating_duration_since(*at) < FOUND_TTL).then(|| profile.clone())
    }

    pub fn put_found(&mut self, viewer: &str, riot_id: &str, profile: PlayerProfile, now: Instant) {
        if self.viewer != viewer {
            return;
        }
        if self.found.len() >= FOUND {
            self.found
                .retain(|_, (_, at)| now.saturating_duration_since(*at) < FOUND_TTL);
        }
        if self.found.len() < FOUND {
            self.found.insert(riot_key(riot_id), (profile, now));
        }
    }

    /// `puuids` have just finished a game: their newest pages are asked for again.
    pub fn expire_newest<'a>(&mut self, puuids: impl IntoIterator<Item = &'a str>) {
        for puuid in puuids {
            if let Some(player) = self.players.get_mut(puuid) {
                for kept in player.pages.iter_mut().filter(|kept| kept.begin == 0) {
                    kept.due = true;
                }
            }
        }
    }

    fn keep_game(&mut self, game: Arc<Game>, now: Instant) {
        if self
            .games
            .insert(game.game_id, (game.clone(), now))
            .is_none()
        {
            self.order.push_back(game.game_id);
        }
        while self.order.len() > GAMES {
            if let Some(id) = self.order.pop_front() {
                self.games.remove(&id);
            }
        }
    }

    fn page_entries(&self) -> usize {
        self.players
            .values()
            .flat_map(|player| &player.pages)
            .map(|kept| kept.page.entries.len())
            .sum()
    }

    /// Lets the least recently viewed players other than `current` go while there are too many,
    /// or too many games in their pages. Past that, `current`'s own pages go, those fetched longest
    /// ago first: never the newest, which tells whether the history moved, nor the one kept last.
    fn trim(&mut self, current: &str) {
        loop {
            let entries = self.page_entries();
            if self.players.len() <= PLAYERS && entries <= GAMES {
                return;
            }
            let Some(oldest) = self
                .players
                .iter()
                .filter(|(puuid, _)| puuid.as_str() != current)
                .min_by_key(|(_, player)| player.used)
                .map(|(puuid, _)| puuid.clone())
            else {
                break;
            };
            self.players.remove(&oldest);
        }
        let Some(player) = self.players.get_mut(current) else {
            return;
        };
        let mut entries: usize = player
            .pages
            .iter()
            .map(|kept| kept.page.entries.len())
            .sum();
        // Pushed last: the page just kept.
        let last = player.pages.len().saturating_sub(1);
        let mut order: Vec<usize> = (0..last)
            .filter(|&index| player.pages[index].begin != 0)
            .collect();
        order.sort_by_key(|&index| player.pages[index].fetched);
        let mut going = Vec::new();
        for index in order {
            if entries <= GAMES {
                break;
            }
            entries -= player.pages[index].page.entries.len();
            going.push(index);
        }
        going.sort_unstable();
        for index in going.into_iter().rev() {
            player.pages.remove(index);
        }
    }
}

// ---- Storage: the sweep and the cleanup (`service/caches.rs`) ----

impl HistoryCache {
    /// Pages, whole games and lookups kept now.
    pub fn entries(&self) -> usize {
        self.page_entries() + self.games.len() + self.found.len()
    }

    /// Lets go of what can no longer be shown as it is: pages past [`OLDER_TTL`] (a newest page is
    /// asked again long before), the players left with none, lookups past [`FOUND_TTL`], and whole
    /// games no kept page holds that were kept that long ago. Returns how many entries went.
    pub fn sweep(&mut self, now: Instant) -> usize {
        let before = self.entries();
        let old = |at: Instant| now.saturating_duration_since(at) >= OLDER_TTL;
        for player in self.players.values_mut() {
            player.pages.retain(|kept| !old(kept.fetched));
        }
        self.players.retain(|_, player| !player.pages.is_empty());
        self.found
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < FOUND_TTL);
        let paged: HashSet<i64> = self
            .players
            .values()
            .flat_map(|player| &player.pages)
            .flat_map(|kept| kept.page.entries.iter().flatten())
            .map(|game| game.game_id)
            .collect();
        self.games
            .retain(|id, (_, at)| paged.contains(id) || !old(*at));
        let games = &self.games;
        self.order.retain(|id| games.contains_key(id));
        before - self.entries()
    }

    /// Lets everything go but whose it is. Returns how many entries went.
    pub fn clear(&mut self) -> usize {
        let went = self.entries();
        let viewer = std::mem::take(&mut self.viewer);
        *self = Self {
            viewer,
            ..Self::default()
        };
        went
    }
}

/// Whether two newest pages start with the same games, as far as both go.
fn same_start(a: &[Option<Arc<Game>>], b: &[Option<Arc<Game>>]) -> bool {
    let id = |entry: &Option<Arc<Game>>| entry.as_ref().map(|game| game.game_id);
    a.iter().zip(b).all(|(a, b)| id(a) == id(b))
}

/// A Riot ID as a key: `name#tag`, each part trimmed, in one case.
fn riot_key(riot_id: &str) -> String {
    let riot_id = riot_id.trim();
    let (name, tag) = riot_id.rsplit_once('#').unwrap_or((riot_id, ""));
    format!("{}#{}", name.trim(), tag.trim()).to_lowercase()
}

/// A player rated on their own, as the History page shows them: champ select's form score, title
/// and quip, without a team. Champ select ranks a team against itself under every scheme but
/// 峡谷八档, and one player ranked against nobody would always land in the middle tier; so a player
/// alone is graded on 峡谷八档's fixed bands, as champ select grades everyone under that scheme,
/// and the grade is spread over the scheme's tiers the way a team would be
/// (`rating::tier_of_grade`). Under 峡谷八档 the tier is the grade. Returns the rating and the
/// grade it was read from; `None` without a counted game.
pub fn rate_alone(summary: &PlayerSummary, ranking: &Ranking) -> Option<(SeatRating, u8)> {
    let names = &ranking.names;
    if names.is_empty() {
        return None;
    }
    let score = rating::form_score(&summary.recent)?;
    let band = rating::grade(score, &rating::FORM_GRADES);
    let tier = if ranking.absolute {
        band.min(names.len() as u8 - 1)
    } else {
        rating::tier_of_grade(band, names.len())
    };
    let title = ranking
        .titles
        .then(|| rating::form_title(&summary.recent))
        .flatten()
        .map(|title| callout::title_name(title, ranking.language).to_owned());
    // Champ select draws a quip per game; here the newest counted game stands for it, so the quip
    // stays while nothing is played and likely changes with the next game.
    let newest = summary
        .recent
        .matches
        .iter()
        .find(|game| !game.remake)
        .map_or(0, |game| game.game_id);
    let rating = SeatRating {
        score,
        tier,
        tiers: names.len() as u8,
        label: names[usize::from(tier)].clone(),
        grade: ranking.absolute.then_some(tier),
        title,
        quip: ranking.quip(tier, &summary.puuid, newest),
    };
    Some((rating, band))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        settings::{CalloutRule, General, Language, TierSet},
        view::{Ranked, RecentForm, RecentMatch},
    };

    fn game(id: i64) -> Arc<Game> {
        Arc::new(Game {
            game_id: id,
            ..Game::default()
        })
    }

    /// One player's history on a server: game ids, newest first, and the requests it answered.
    struct Server {
        ids: Vec<i64>,
        requests: usize,
    }

    impl Server {
        fn new(games: i64) -> Self {
            Self {
                ids: (1..=games).rev().collect(),
                requests: 0,
            }
        }

        /// A new game, at the top.
        fn play(&mut self) {
            let next = self.ids.first().map_or(1, |id| id + 1);
            self.ids.insert(0, next);
        }

        fn page(&mut self, begin: u32, count: u32) -> Page {
            self.requests += 1;
            let entries: Vec<_> = self
                .ids
                .iter()
                .skip(begin as usize)
                .take(count as usize)
                .map(|&id| Some(game(id)))
                .collect();
            Page {
                more: entries.len() == count as usize,
                entries,
                source: HistorySource::Server,
            }
        }
    }

    /// What `Service::match_history` does: the cache, else the server and into the cache.
    fn ask(
        cache: &mut HistoryCache,
        server: &mut Server,
        (viewer, puuid): (&str, &str),
        (begin, count): (u32, u32),
        now: Instant,
    ) -> Vec<i64> {
        let page = cache
            .page(viewer, puuid, begin, count, now)
            .unwrap_or_else(|| {
                let page = server.page(begin, count);
                cache.put_page(viewer, puuid, begin, page.clone(), now);
                page
            });
        page.entries
            .iter()
            .flatten()
            .map(|game| game.game_id)
            .collect()
    }

    const ME: &str = "me";

    #[test]
    fn going_back_paging_back_and_a_shorter_newest_page_cost_no_request() {
        let start = Instant::now();
        let mut cache = HistoryCache::default();
        let (mut a, mut b) = (Server::new(120), Server::new(80));
        let at = |seconds| start + Duration::from_secs(seconds);
        // The History page on A, its second fifty, B's, back to A and its second fifty again, and
        // the overview's ten newest of A's.
        let first = ask(&mut cache, &mut a, (ME, "a"), (0, 50), at(0));
        ask(&mut cache, &mut a, (ME, "a"), (50, 50), at(5));
        ask(&mut cache, &mut b, (ME, "b"), (0, 50), at(10));
        assert_eq!(ask(&mut cache, &mut a, (ME, "a"), (0, 50), at(40)), first);
        ask(&mut cache, &mut a, (ME, "a"), (50, 50), at(45));
        assert_eq!(
            ask(&mut cache, &mut a, (ME, "a"), (0, 10), at(50)),
            first[..10]
        );
        assert_eq!(
            (a.requests, b.requests),
            (2, 1),
            "six pages asked for, three fetched"
        );
        // Every game of those pages opens its scoreboard from the cache.
        assert!(
            first
                .iter()
                .all(|&id| cache.game(ME, id).is_some_and(|game| game.game_id == id))
        );
    }

    #[test]
    fn the_newest_page_is_asked_again_after_its_time_or_after_a_game() {
        let start = Instant::now();
        let mut cache = HistoryCache::default();
        let mut server = Server::new(60);
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), start);
        ask(&mut cache, &mut server, (ME, "a"), (50, 50), start);
        ask(
            &mut cache,
            &mut server,
            (ME, "a"),
            (0, 50),
            start + NEWEST_TTL,
        );
        assert_eq!(server.requests, 3, "the newest page is asked again");
        // The same history: the older page is still good.
        ask(
            &mut cache,
            &mut server,
            (ME, "a"),
            (50, 50),
            start + NEWEST_TTL,
        );
        assert_eq!(server.requests, 3);
        cache.expire_newest(["a"]);
        ask(
            &mut cache,
            &mut server,
            (ME, "a"),
            (0, 50),
            start + NEWEST_TTL,
        );
        assert_eq!(server.requests, 4, "a game just ended");
        ask(
            &mut cache,
            &mut server,
            (ME, "a"),
            (50, 50),
            start + OLDER_TTL,
        );
        assert_eq!(server.requests, 5, "older pages last half an hour");
    }

    #[test]
    fn new_games_drop_the_older_pages_they_move_so_none_goes_missing() {
        let start = Instant::now();
        let mut cache = HistoryCache::default();
        let mut server = Server::new(100);
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), start);
        ask(&mut cache, &mut server, (ME, "a"), (50, 50), start);
        server.play();
        server.play();
        let later = start + NEWEST_TTL;
        let newest = ask(&mut cache, &mut server, (ME, "a"), (0, 50), later);
        let older = ask(&mut cache, &mut server, (ME, "a"), (50, 50), later);
        assert_eq!(
            server.requests, 4,
            "the older page moved and is fetched again"
        );
        let shown: Vec<i64> = newest.into_iter().chain(older).collect();
        assert_eq!(shown, server.ids[..100], "every game once, in order");
    }

    #[test]
    fn pages_games_and_lookups_belong_to_the_account_that_fetched_them() {
        let now = Instant::now();
        let mut cache = HistoryCache::default();
        let mut server = Server::new(30);
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), now);
        cache.put_found(ME, "Someone#1", profile("a"), now);
        assert!(cache.page("other", "a", 0, 50, now).is_none());
        assert!(cache.game("other", 30).is_none());
        assert!(cache.found("other", "someone#1", now).is_none());
        // Back to the first account: nothing of its is left either, it was let go.
        assert!(cache.page(ME, "a", 0, 50, now).is_none());
        assert!(!cache.scope(ME), "already the first account's");
        // A page that arrives after its account signed out is not kept for the next one.
        cache.scope("other");
        cache.put_page(ME, "a", 0, server.page(0, 50), now);
        cache.put_game(ME, game(7), now);
        cache.put_found(ME, "Someone#1", profile("a"), now);
        assert!(cache.page("other", "a", 0, 50, now).is_none());
        assert!(cache.game("other", 7).is_none());
        assert!(cache.found("other", "Someone#1", now).is_none());
    }

    fn profile(puuid: &str) -> PlayerProfile {
        PlayerProfile {
            puuid: puuid.into(),
            name: None,
            level: 1,
            icon_id: 0,
            private: false,
            ranked: Ranked::default(),
        }
    }

    #[test]
    fn a_riot_id_is_looked_up_once_in_five_minutes_whatever_its_case_and_spaces() {
        let now = Instant::now();
        let mut cache = HistoryCache::default();
        cache.scope(ME);
        cache.put_found(ME, "  Light#10003 ", profile("a"), now);
        assert_eq!(
            cache
                .found(ME, "light # 10003", now)
                .map(|found| found.puuid),
            Some("a".into())
        );
        assert!(cache.found(ME, "light#10003", now + FOUND_TTL).is_none());
    }

    #[test]
    fn a_page_serves_what_it_holds_and_the_end_of_the_history() {
        let now = Instant::now();
        let mut cache = HistoryCache::default();
        let mut server = Server::new(30);
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), now);
        let page = cache.page(ME, "a", 10, 10, now).unwrap();
        assert_eq!((page.entries.len(), page.more), (10, true));
        let rest = cache.page(ME, "a", 20, 50, now).unwrap();
        assert_eq!(
            (rest.entries.len(), rest.more),
            (10, false),
            "the last page"
        );
        let past = cache.page(ME, "a", 50, 50, now).unwrap();
        assert_eq!(
            (past.entries.len(), past.more),
            (0, false),
            "nothing past the end"
        );
        assert_eq!(server.requests, 1);
        // A full page says nothing of what follows it.
        let mut long = Server::new(120);
        ask(&mut cache, &mut long, (ME, "b"), (0, 50), now);
        assert!(cache.page(ME, "b", 40, 20, now).is_none());
        // A place the server never recorded keeps its place.
        cache.put_page(
            ME,
            "c",
            0,
            Page {
                entries: vec![None, Some(game(5)), Some(game(4))],
                more: false,
                source: HistorySource::Server,
            },
            now,
        );
        let page = cache.page(ME, "c", 1, 1, now).unwrap();
        assert_eq!(page.entries, vec![Some(game(5))]);
    }

    #[test]
    fn the_clients_own_games_are_kept_as_pages_but_not_as_scoreboards() {
        let now = Instant::now();
        let mut cache = HistoryCache::default();
        cache.scope(ME);
        cache.put_page(
            ME,
            "a",
            0,
            Page {
                entries: vec![Some(game(3)), Some(game(2))],
                more: false,
                source: HistorySource::Client,
            },
            now,
        );
        assert_eq!(
            cache.page(ME, "a", 0, 50, now).map(|page| page.source),
            Some(HistorySource::Client)
        );
        assert!(
            cache.game(ME, 3).is_none(),
            "a list row carries one player, not the game"
        );
    }

    #[test]
    fn players_and_games_kept_are_bounded_the_least_recently_viewed_going_first() {
        let start = Instant::now();
        let mut cache = HistoryCache::default();
        for player in 0..PLAYERS as u64 + 4 {
            let mut server = Server::new(10);
            let puuid = format!("p{player}");
            ask(
                &mut cache,
                &mut server,
                (ME, &puuid),
                (0, 10),
                start + Duration::from_secs(player),
            );
        }
        assert_eq!(cache.players.len(), PLAYERS);
        assert!(!cache.players.contains_key("p0") && cache.players.contains_key("p19"));
        for id in 0..GAMES as i64 + 50 {
            cache.put_game(ME, game(id), start);
        }
        assert_eq!((cache.games.len(), cache.order.len()), (GAMES, GAMES));
        assert!(cache.game(ME, 0).is_none() && cache.game(ME, GAMES as i64 + 49).is_some());
        // A player paged far keeps their pages while others make room, and past the cap lets go of
        // the pages fetched longest ago: never the newest one, nor the page being read.
        let mut deep = Server::new(1000);
        for chunk in 0..14 {
            ask(
                &mut cache,
                &mut deep,
                (ME, "deep"),
                (chunk * 50, 50),
                start + Duration::from_secs(u64::from(chunk)),
            );
        }
        assert!(cache.players.contains_key("deep"));
        assert_eq!(cache.players.len(), 1, "everyone else went");
        assert!(cache.page_entries() <= GAMES, "{}", cache.page_entries());
        let later = start + Duration::from_secs(20);
        assert!(cache.page(ME, "deep", 0, 50, later).is_some());
        assert!(cache.page(ME, "deep", 650, 50, later).is_some());
        assert!(
            cache.page(ME, "deep", 50, 50, later).is_none(),
            "the second page, fetched longest ago after the newest, went first"
        );
        let asked = deep.requests;
        ask(&mut cache, &mut deep, (ME, "deep"), (50, 50), later);
        assert_eq!(deep.requests, asked + 1, "and is asked for again");
    }

    #[test]
    fn a_sweep_lets_go_of_what_can_no_longer_be_shown_and_keeps_the_rest() {
        let start = Instant::now();
        let mut cache = HistoryCache::default();
        let mut old = Server::new(60);
        // Games of their own: 2060 down to 2001.
        let mut new = Server {
            ids: (2001..=2060).rev().collect(),
            requests: 0,
        };
        ask(&mut cache, &mut old, (ME, "old"), (0, 50), start);
        cache.put_found(ME, "Old#1", profile("old"), start);
        cache.put_game(ME, game(1000), start);
        let soon = start + OLDER_TTL - Duration::from_secs(60);
        ask(&mut cache, &mut new, (ME, "new"), (0, 50), soon);
        ask(&mut cache, &mut new, (ME, "new"), (50, 50), soon);
        cache.put_found(ME, "New#1", profile("new"), soon);
        assert_eq!(
            cache.sweep(soon),
            1,
            "only the lookup is past its five minutes"
        );

        let at = start + OLDER_TTL;
        // `old`'s page and the game kept alone are half an hour old; `old`'s games went with
        // their page, `new`'s stay with theirs.
        assert_eq!(cache.sweep(at), 50 + 50 + 1);
        assert!(!cache.players.contains_key("old"));
        assert!(cache.game(ME, 1000).is_none() && cache.game(ME, 60).is_none());
        assert!(cache.game(ME, 2060).is_some() && cache.game(ME, 2001).is_some());
        assert!(cache.page(ME, "new", 50, 50, at).is_some());
        assert!(cache.found(ME, "New#1", soon).is_some());
        assert_eq!(cache.order.len(), cache.games.len());
        assert_eq!(cache.entries(), 60 + 60 + 1);
    }

    #[test]
    fn a_cleanup_empties_the_cache_but_keeps_whose_it_is() {
        let now = Instant::now();
        let mut cache = HistoryCache::default();
        let mut server = Server::new(30);
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), now);
        cache.put_found(ME, "A#1", profile("a"), now);
        assert_eq!(cache.clear(), 30 + 30 + 1);
        assert_eq!((cache.entries(), cache.viewer()), (0, ME));
        assert!(!cache.scope(ME), "still the same account's");
        ask(&mut cache, &mut server, (ME, "a"), (0, 50), now);
        assert_eq!(server.requests, 2, "asked for again");
    }

    fn summary(games: u32, wins: u32, kills: f64, deaths: f64, assists: f64) -> PlayerSummary {
        PlayerSummary {
            puuid: "p".into(),
            name: None,
            level: 30,
            icon_id: 0,
            private: false,
            ranked: Ranked::default(),
            recent: RecentForm {
                games,
                wins,
                kills,
                deaths,
                assists,
                matches: (0..games)
                    .map(|index| RecentMatch {
                        game_id: 100 - i64::from(index),
                        queue_id: 2400,
                        champion_id: 1,
                        win: index < wins,
                        remake: false,
                        kills: 0,
                        deaths: 0,
                        assists: 0,
                        started_at: 0,
                    })
                    .collect(),
                ..RecentForm::default()
            },
        }
    }

    fn ranking(tiers: TierSet, titles: bool) -> Ranking {
        let rule = CalloutRule {
            tiers,
            ..CalloutRule::default()
        };
        let general = General {
            titles,
            ..General::default()
        };
        callout::ranking(&rule, &general)
    }

    #[test]
    fn a_player_alone_is_graded_on_the_fixed_bands_and_placed_in_the_schemes_tiers() {
        // Half the games won at KDA 3, twenty games: 5.5, a B, the middle of the default five.
        let middling = summary(20, 10, 5.0, 5.0, 10.0);
        let (rating, band) = rate_alone(&middling, &ranking(TierSet::RiftFive, true)).unwrap();
        assert_eq!(
            (rating.score, band, rating.tier, rating.tiers),
            (5.5, 3, 2, 5)
        );
        assert_eq!(rating.label, "峡谷公务员");
        assert_eq!(rating.grade, None, "峡谷五档 shows no letter");
        let quips = [
            "不一定惊艳，但该干的活全干了",
            "按时上班，准时打卡",
            "无功无过，绩效合格",
        ];
        assert!(
            quips.contains(&rating.quip.as_deref().unwrap()),
            "{rating:?}"
        );
        // The quip follows the newest game, as champ select's follows each game.
        let again = rate_alone(&middling, &ranking(TierSet::RiftFive, true)).unwrap();
        assert_eq!(again.0.quip, rating.quip);

        // 峡谷八档 grades everyone on the bands: alone or in a team, the same tier.
        let (graded, band) = rate_alone(&middling, &ranking(TierSet::Grades, true)).unwrap();
        assert_eq!(
            (graded.tier, graded.grade, band, graded.label.as_str()),
            (3, Some(3), 3, "有用之人")
        );
    }

    #[test]
    fn a_player_alone_gets_champ_selects_title_while_titles_are_on() {
        // Eight deaths a game and no streak worth naming: the grey screen's regular.
        let mut grey = summary(20, 9, 4.0, 9.0, 8.0);
        grey.recent.streak = 1;
        let (rating, _) = rate_alone(&grey, &ranking(TierSet::RiftFive, true)).unwrap();
        assert_eq!(rating.title.as_deref(), Some("黑白电视机资深会员"));
        assert_eq!(rating.tier, 4, "an E is the last of five");
        let (untitled, _) = rate_alone(&grey, &ranking(TierSet::RiftFive, false)).unwrap();
        assert_eq!(untitled.title, None);
        let english = Ranking {
            language: Language::En,
            ..ranking(TierSet::RiftFive, true)
        };
        assert_eq!(
            rate_alone(&grey, &english).unwrap().0.title.as_deref(),
            Some("Grey-screen Regular")
        );
        assert!(
            rate_alone(
                &summary(0, 0, 0.0, 0.0, 0.0),
                &ranking(TierSet::RiftFive, true)
            )
            .is_none(),
            "no counted game, no rating"
        );
    }
}
