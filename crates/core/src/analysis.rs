//! Folding LCU documents into the views: ranks, recent form and scoreboard lines.

use std::{cmp::Reverse, collections::HashMap};

use crate::{
    model::{Game, Participant, RankedEntry, RankedStats, Stats, Summoner},
    rating::{self, Contribution, Played, Roles},
    view::{
        ChampionForm, Feat, FormScope, FormSource, GameKind, MatchDetail, MatchSummary, ModeFamily,
        PlayerLine, PlayerProfile, PlayerSummary, Position, Rank, Ranked, RecentForm, RecentMatch,
        RiotId, TeamDetail, Tier,
    },
};

/// Recent form covers this many games at most, newest first.
pub const RECENT_GAMES: usize = 20;

/// Each queue's kind of game, by queue id, from the client's catalog (`catalog::queue_kind`).
pub type QueueKinds = HashMap<i64, GameKind>;

/// Games shorter than this count as remakes even when the surrender flag is missing.
const REMAKE_SECONDS: i64 = 240;

pub fn rank(entry: &RankedEntry) -> Option<Rank> {
    let tier = Tier::parse(&entry.tier)?;
    let division = matches!(entry.division.as_str(), "I" | "II" | "III" | "IV")
        .then(|| entry.division.clone());
    Some(Rank {
        tier,
        division,
        lp: entry.league_points,
        wins: entry.wins,
        losses: entry.losses,
    })
}

pub fn ranked(stats: &RankedStats) -> Ranked {
    let queue = |name: &str| stats.queue_map.get(name).and_then(rank);
    Ranked {
        solo: queue("RANKED_SOLO_5x5"),
        flex: queue("RANKED_FLEX_SR"),
    }
}

pub fn riot_id(summoner: &Summoner) -> Option<RiotId> {
    RiotId::new(&summoner.game_name, &summoner.tag_line)
}

pub fn profile(summoner: &Summoner, ranked_stats: Option<&RankedStats>) -> PlayerProfile {
    PlayerProfile {
        puuid: summoner.puuid.clone(),
        name: riot_id(summoner),
        level: summoner.summoner_level,
        icon_id: summoner.profile_icon_id,
        private: summoner.privacy.eq_ignore_ascii_case("PRIVATE"),
        ranked: ranked_stats.map(ranked).unwrap_or_default(),
    }
}

/// A player at a glance. `focus` narrows the recent form to the kind of game being played
/// ([`recent_form`]).
pub fn summary(
    summoner: &Summoner,
    ranked_stats: Option<&RankedStats>,
    games: &[Game],
    catalog: &Catalog,
    focus: Option<ModeFamily>,
) -> PlayerSummary {
    let profile = profile(summoner, ranked_stats);
    PlayerSummary {
        recent: recent_form(&summoner.puuid, games, catalog, focus),
        puuid: profile.puuid,
        name: profile.name,
        level: profile.level,
        icon_id: profile.icon_id,
        private: profile.private,
        ranked: profile.ranked,
    }
}

fn is_remake(game: &Game, participant: &Participant) -> bool {
    participant.stats.game_ended_in_early_surrender
        || (game.game_duration > 0 && game.game_duration < REMAKE_SECONDS)
}

/// Who `game` was played against. The game's own type decides first (a custom game, whatever its
/// queue; the tutorial, which is played against the computer), then the catalog's word on its
/// queue; a queue the catalog does not list counts as one against players.
pub fn game_kind(game: &Game, kinds: &QueueKinds) -> GameKind {
    match game.game_type.to_ascii_uppercase().as_str() {
        "CUSTOM_GAME" => GameKind::Custom,
        "TUTORIAL_GAME" => GameKind::Bots,
        _ => kinds.get(&game.queue_id).copied().unwrap_or_default(),
    }
}

/// What the recent form reads from the client's catalog: each queue's kind of game, and each
/// champion's role for the game score.
#[derive(Clone, Copy, Debug)]
pub struct Catalog<'a> {
    pub kinds: &'a QueueKinds,
    pub roles: &'a Roles,
}

/// `puuid`'s row of `game`: theirs by identity, or the only one a client's list page carries,
/// whose identity can be masked.
fn row_of<'a>(game: &'a Game, puuid: &str) -> Option<&'a Participant> {
    game.participant_of(puuid)
        .or(match game.participants.as_slice() {
            [only] => Some(only),
            _ => None,
        })
}

/// `puuid`'s recent games, newest first, folded into averages, a streak, a champion pool and a
/// strength. Only games against other players count: custom games (practice, lobbies among friends)
/// and games against the computer are passed over on the way to the newest [`RECENT_GAMES`], and
/// the remakes among those are shown but left out of every figure.
///
/// With a `focus`, the form reads that kind of game only, the newest twenty of it, and never mixes
/// in another: a player with fewer has fewer, which pulls the strength toward the average harder,
/// and one with none of it has no strength there.
///
/// Each counted game is scored for the strength ([`rating::strength`]): against its other players
/// where the game has them all, as a game from the shard's server does, else against the average
/// player of its mode ([`rating::lite_score`]).
pub fn recent_form(
    puuid: &str,
    games: &[Game],
    catalog: &Catalog,
    focus: Option<ModeFamily>,
) -> RecentForm {
    form(puuid, games, catalog, focus).0
}

/// Whether `games` already hold the window [`recent_form`] reads with `focus`: [`RECENT_GAMES`]
/// games against other players, of the focused kind of game where there is one. Fetching a record
/// stops here.
pub fn window_filled(games: &[Game], kinds: &QueueKinds, focus: Option<ModeFamily>) -> bool {
    games
        .iter()
        .filter(|game| game_kind(game, kinds) == GameKind::Matched)
        .filter(|game| focus.is_none_or(|family| ModeFamily::of(&game.game_mode) == family))
        .count()
        >= RECENT_GAMES
}

/// What [`recent_form`] reads from `games`, over every kind of game, and what it leaves out.
pub fn form_scope(puuid: &str, games: &[Game], catalog: &Catalog) -> FormScope {
    form(puuid, games, catalog, None).1
}

/// One game of a form, scored for the strength.
struct Read {
    game: RecentMatch,
    played: Played,
}

fn read(game: &Game, participant: &Participant, roles: &Roles) -> Read {
    let remake = is_remake(game, participant);
    let full = game.participants.len() > 1;
    let contribution_of =
        |row: &Participant| contribution(&row.stats, roles.get(&row.champion_id).copied());
    let score = if remake {
        None
    } else if full {
        let lines: Vec<Contribution> = game.participants.iter().map(contribution_of).collect();
        game.participants
            .iter()
            .position(|row| std::ptr::eq(row, participant))
            .and_then(|index| {
                rating::game_scores(&lines, rating::Scoring::of(&game.game_mode))
                    .get(index)
                    .copied()
            })
    } else {
        rating::lite_score(
            &contribution_of(participant),
            &game.game_mode,
            game.game_duration,
        )
    };
    let away = game
        .participants
        .iter()
        .any(|row| !std::ptr::eq(row, participant) && row.stats.was_afk);
    let family = ModeFamily::of(&game.game_mode);
    let par = rating::par(
        family,
        position_of(participant),
        roles.get(&participant.champion_id).copied(),
    );
    let stats = &participant.stats;
    Read {
        game: RecentMatch {
            game_id: game.game_id,
            queue_id: game.queue_id,
            champion_id: participant.champion_id,
            win: stats.win,
            remake,
            kills: stats.kills,
            deaths: stats.deaths,
            assists: stats.assists,
            started_at: game.game_creation,
            score,
            away,
        },
        played: Played {
            score: score.map(|score| (score + par).clamp(0.0, 10.0)),
            lite: !full,
            win: stats.win,
            away,
            family: Some(family),
        },
    }
}

/// Where `participant` played on the Rift: the position the game settled on, which the shard's
/// server sends, else the client's guess from lane and role where it names one.
fn position_of(participant: &Participant) -> Option<Position> {
    Position::parse(&participant.team_position).or_else(|| {
        let role = participant.timeline.role.to_ascii_uppercase();
        match Position::parse(&participant.timeline.lane)? {
            Position::Bottom if role.contains("SUPPORT") => Some(Position::Utility),
            Position::Bottom if role.contains("CARRY") => Some(Position::Bottom),
            Position::Bottom => None,
            position => Some(position),
        }
    })
}

fn form(
    puuid: &str,
    games: &[Game],
    catalog: &Catalog,
    focus: Option<ModeFamily>,
) -> (RecentForm, FormScope) {
    let mut newest: Vec<&Game> = games.iter().collect();
    newest.sort_by_key(|game| Reverse(game.game_creation));
    let mut scope = FormScope {
        listed: games.len() as u32,
        ..FormScope::default()
    };
    let mut matched = Vec::with_capacity(newest.len());
    for game in newest {
        match game_kind(game, catalog.kinds) {
            GameKind::Matched => matched.push(game),
            // Counted on the way to the newest twenty, as the window over every kind goes.
            GameKind::Custom if matched.len() < RECENT_GAMES => scope.custom += 1,
            GameKind::Bots if matched.len() < RECENT_GAMES => scope.bots += 1,
            GameKind::Custom | GameKind::Bots => {}
        }
    }
    let reads = |games: &[&Game]| -> Vec<Read> {
        games
            .iter()
            .filter_map(|game| Some(read(game, row_of(game, puuid)?, catalog.roles)))
            .collect()
    };
    let games: Vec<&Game> = matched
        .into_iter()
        .filter(|game| focus.is_none_or(|family| ModeFamily::of(&game.game_mode) == family))
        .take(RECENT_GAMES)
        .collect();
    let reads = reads(&games);
    let family = focus;

    let counted: Vec<&Read> = reads.iter().filter(|read| !read.game.remake).collect();
    let total = counted.len() as u32;
    let average = |value: fn(&RecentMatch) -> i64| {
        if total == 0 {
            0.0
        } else {
            counted.iter().map(|read| value(&read.game)).sum::<i64>() as f64 / f64::from(total)
        }
    };

    let streak = match counted.first() {
        Some(first) => {
            let run = counted
                .iter()
                .take_while(|read| read.game.win == first.game.win)
                .count() as i32;
            if first.game.win { run } else { -run }
        }
        None => 0,
    };

    let mut pool: HashMap<i64, ChampionForm> = HashMap::new();
    for read in &counted {
        let form = pool.entry(read.game.champion_id).or_insert(ChampionForm {
            champion_id: read.game.champion_id,
            games: 0,
            wins: 0,
        });
        form.games += 1;
        form.wins += u32::from(read.game.win);
    }
    let mut champions: Vec<ChampionForm> = pool.into_values().collect();
    champions.sort_by(|a, b| {
        b.games
            .cmp(&a.games)
            .then(b.wins.cmp(&a.wins))
            .then(a.champion_id.cmp(&b.champion_id))
    });
    champions.truncate(5);
    scope.remakes = reads.len() as u32 - total;

    // The counted games against their modes' averages, for the title.
    let pace = rating::Pace::of(games.iter().filter_map(|game| {
        let participant = row_of(game, puuid)?;
        let average = rating::average_line(&game.game_mode)?;
        let stats = &participant.stats;
        (!is_remake(game, participant))
            .then_some(([stats.kills, stats.deaths, stats.assists], average))
    }));

    let played: Vec<Played> = counted.iter().map(|read| read.played).collect();
    let strength = rating::strength(&played);
    let scored = played.iter().filter(|game| game.score.is_some());
    let source = scored.clone().next().map(|_| {
        if scored.clone().any(|game| game.lite) {
            FormSource::Lite
        } else {
            FormSource::Full
        }
    });
    let form = RecentForm {
        games: total,
        wins: counted.iter().filter(|read| read.game.win).count() as u32,
        kills: average(|game| game.kills),
        deaths: average(|game| game.deaths),
        assists: average(|game| game.assists),
        streak,
        champions,
        score: strength.map(|strength| strength.score),
        source,
        family,
        away: counted.iter().filter(|read| read.game.away).count() as u32,
        pace,
        spread: strength.and_then(|strength| strength.spread),
        matches: reads.into_iter().map(|read| read.game).collect(),
    };
    (form, scope)
}

fn line(game: &Game, participant: &Participant) -> PlayerLine {
    let player = game.identity(participant.participant_id);
    let stats = &participant.stats;
    PlayerLine {
        puuid: player
            .map(|player| player.puuid.clone())
            .unwrap_or_default(),
        name: player.and_then(|player| RiotId::new(&player.game_name, &player.tag_line)),
        icon_id: player.map_or(0, |player| player.profile_icon),
        champion_id: participant.champion_id,
        champion_level: stats.champ_level,
        position: Position::parse(&participant.timeline.lane),
        spells: [participant.spell1_id, participant.spell2_id],
        items: stats.items(),
        augments: stats.augments(),
        keystone: stats.perk0,
        sub_style: stats.perk_sub_style,
        kills: stats.kills,
        deaths: stats.deaths,
        assists: stats.assists,
        cs: stats.total_minions_killed + stats.neutral_minions_killed,
        gold: stats.gold_earned,
        damage: stats.total_damage_dealt_to_champions,
        damage_taken: stats.total_damage_taken,
        vision: stats.vision_score,
        largest_multi_kill: stats.largest_multi_kill,
        win: stats.win,
        remake: is_remake(game, participant),
        placement: (stats.subteam_placement > 0).then_some(stats.subteam_placement),
        damage_share: None,
        kill_participation: None,
        score: None,
        grade: None,
        award: None,
        feats: own_feats(stats),
    }
}

/// The feats one line earns on its own: its best multikill, a spree of eight, first blood, AFK.
fn own_feats(stats: &Stats) -> Vec<Feat> {
    let multikill = match stats.largest_multi_kill {
        5.. => Some(Feat::Penta),
        4 => Some(Feat::Quadra),
        3 => Some(Feat::Triple),
        2 => Some(Feat::Double),
        _ => None,
    };
    let mut feats: Vec<Feat> = [
        stats.was_afk.then_some(Feat::Afk),
        multikill,
        (stats.largest_killing_spree >= 8).then_some(Feat::Legendary),
        stats.first_blood_kill.then_some(Feat::FirstBlood),
    ]
    .into_iter()
    .flatten()
    .collect();
    feats.sort();
    feats
}

/// Reads the one number of a line that a lead compares.
type LeadValue = fn(&Stats) -> i64;

/// The stats a game leader is named for, and what each reads.
const LEADS: [(Feat, LeadValue); 7] = [
    (Feat::MostKills, |stats| stats.kills),
    (Feat::MostDamage, |stats| {
        stats.total_damage_dealt_to_champions
    }),
    (Feat::MostTowers, |stats| stats.turret_kills),
    (Feat::MostAssists, |stats| stats.assists),
    (Feat::MostGold, |stats| stats.gold_earned),
    (Feat::MostTaken, |stats| stats.total_damage_taken),
    (Feat::MostCs, |stats| {
        stats.total_minions_killed + stats.neutral_minions_killed
    }),
];

/// For each lead, which participants (by position in `participants`) share the game's best; a
/// best of nothing, or one everybody shares, names nobody.
fn leads(participants: &[Participant]) -> Vec<(Feat, Vec<usize>)> {
    LEADS
        .iter()
        .filter_map(|(feat, read)| {
            let values: Vec<i64> = participants.iter().map(|p| read(&p.stats)).collect();
            let best = values.iter().copied().max()?;
            if best <= 0 || values.iter().all(|&value| value == best) {
                return None;
            }
            let leaders = (0..values.len()).filter(|&i| values[i] == best).collect();
            Some((*feat, leaders))
        })
        .collect()
}

fn contribution(stats: &Stats, role: Option<rating::Role>) -> Contribution {
    Contribution {
        kills: stats.kills,
        deaths: stats.deaths,
        assists: stats.assists,
        damage: stats.total_damage_dealt_to_champions,
        tanked: stats.total_damage_taken,
        gold: stats.gold_earned,
        minions: stats.total_minions_killed + stats.neutral_minions_killed,
        vision: stats.vision_score,
        crowd_control: stats.time_ccing_others,
        role,
    }
}

/// One game as `puuid` played it; `None` when the game has no row for them. A client's list page
/// carries one player only, whose identity can be masked; that one row is theirs. A game with every
/// player in it (the shard's server sends those) gives the line what only the whole game knows:
/// shares, score, grade and award.
pub fn match_summary(
    puuid: &str,
    game: &Game,
    roles: &Roles,
    kinds: &QueueKinds,
) -> Option<MatchSummary> {
    let only = match game.participants.as_slice() {
        [only] => Some(only),
        _ => None,
    };
    let participant = game.participant_of(puuid).or(only)?;
    let line = if only.is_none() {
        match_detail(game, roles)
            .teams
            .into_iter()
            .flat_map(|team| team.players)
            .find(|line| line.puuid == puuid)
            .unwrap_or_else(|| line(game, participant))
    } else {
        line(game, participant)
    };
    Some(MatchSummary {
        game_id: game.game_id,
        queue_id: game.queue_id,
        game_mode: game.game_mode.clone(),
        started_at: game.game_creation,
        duration: game.game_duration,
        line,
        kind: game_kind(game, kinds),
    })
}

pub fn match_detail(game: &Game, roles: &Roles) -> MatchDetail {
    let mut teams: Vec<TeamDetail> = game
        .teams
        .iter()
        .map(|team| TeamDetail {
            team_id: team.team_id,
            win: team.win.eq_ignore_ascii_case("Win"),
            bans: team
                .bans
                .iter()
                .map(|ban| ban.champion_id)
                .filter(|&id| id > 0)
                .collect(),
            kills: 0,
            gold: 0,
            towers: team.tower_kills,
            dragons: team.dragon_kills,
            barons: team.baron_kills,
            players: Vec::new(),
        })
        .collect();
    // Where each participant's line landed, and what the score reads from it.
    let mut placed: Vec<(usize, usize, Contribution)> = Vec::with_capacity(game.participants.len());
    for participant in &game.participants {
        let index = match teams
            .iter()
            .position(|team| team.team_id == participant.team_id)
        {
            Some(index) => index,
            None => {
                // Arena games list sub-teams without team rows; group them anyway.
                teams.push(TeamDetail {
                    team_id: participant.team_id,
                    win: participant.stats.win,
                    bans: Vec::new(),
                    kills: 0,
                    gold: 0,
                    towers: 0,
                    dragons: 0,
                    barons: 0,
                    players: Vec::new(),
                });
                teams.len() - 1
            }
        };
        let team = &mut teams[index];
        team.kills += participant.stats.kills;
        team.gold += participant.stats.gold_earned;
        team.players.push(line(game, participant));
        placed.push((
            index,
            team.players.len() - 1,
            contribution(
                &participant.stats,
                roles.get(&participant.champion_id).copied(),
            ),
        ));
    }

    // Shares need the team totals, complete only now.
    for team in &mut teams {
        let damage: i64 = team.players.iter().map(|player| player.damage).sum();
        for player in &mut team.players {
            player.damage_share = (damage > 0).then(|| player.damage as f64 / damage as f64);
            player.kill_participation = (team.kills > 0)
                .then(|| ((player.kills + player.assists) as f64 / team.kills as f64).min(1.0));
        }
    }
    // A remake says nothing about anyone: no scores, no MVP.
    let remake = game
        .participants
        .first()
        .is_some_and(|participant| is_remake(game, participant));
    if !remake {
        let scores = rating::game_scores(
            &placed
                .iter()
                .map(|(_, _, contribution)| *contribution)
                .collect::<Vec<_>>(),
            rating::Scoring::of(&game.game_mode),
        );
        let won: Vec<bool> = placed.iter().map(|(team, _, _)| teams[*team].win).collect();
        for (((team, slot, _), score), award) in placed
            .iter()
            .zip(&scores)
            .zip(rating::awards(&scores, &won))
        {
            let line = &mut teams[*team].players[*slot];
            line.score = Some(*score);
            line.grade = Some(rating::grade(*score, &rating::GAME_GRADES));
            line.award = award;
        }
        if game.participants.len() > 1 {
            for (feat, leaders) in leads(&game.participants) {
                for index in leaders {
                    let (team, slot, _) = placed[index];
                    teams[team].players[slot].feats.push(feat);
                }
            }
            for line in teams.iter_mut().flat_map(|team| &mut team.players) {
                line.feats.sort();
            }
        }
    } else {
        // A remake says only who was away, which is usually why it was one.
        for line in teams.iter_mut().flat_map(|team| &mut team.players) {
            line.feats.retain(|feat| *feat == Feat::Afk);
        }
    }

    MatchDetail {
        game_id: game.game_id,
        queue_id: game.queue_id,
        game_mode: game.game_mode.clone(),
        game_version: game.game_version.clone(),
        started_at: game.game_creation,
        duration: game.game_duration,
        teams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The form over every kind of game, champions' roles unknown.
    fn form_of(puuid: &str, games: &[Game], kinds: &QueueKinds) -> RecentForm {
        let roles = Roles::new();
        recent_form(
            puuid,
            games,
            &Catalog {
                kinds,
                roles: &roles,
            },
            None,
        )
    }

    fn scope_of(puuid: &str, games: &[Game], kinds: &QueueKinds) -> FormScope {
        let roles = Roles::new();
        form_scope(
            puuid,
            games,
            &Catalog {
                kinds,
                roles: &roles,
            },
        )
    }
    use crate::{model::MatchList, test_support::fixture, view::Award};

    #[test]
    fn ranks_skip_unranked_queues_and_hide_meaningless_divisions() {
        let unranked = RankedEntry {
            tier: String::new(),
            division: "NA".into(),
            ..RankedEntry::default()
        };
        assert_eq!(rank(&unranked), None);
        let master = RankedEntry {
            tier: "MASTER".into(),
            division: "NA".into(),
            league_points: 120,
            ..RankedEntry::default()
        };
        assert_eq!(
            rank(&master).map(|rank| (rank.tier, rank.division, rank.lp)),
            Some((Tier::Master, None, 120))
        );
        let gold = RankedEntry {
            tier: "GOLD".into(),
            division: "II".into(),
            wins: 10,
            losses: 8,
            ..RankedEntry::default()
        };
        assert_eq!(
            rank(&gold).and_then(|rank| rank.division),
            Some("II".into())
        );
    }

    #[test]
    fn recent_form_from_a_live_ranked_history() {
        let list: MatchList = fixture("live/ranked/match-list.json");
        let puuid = list.games.games[0].participant_identities[0]
            .player
            .puuid
            .clone();
        let form = form_of(&puuid, &list.games.games, &QueueKinds::new());
        assert_eq!(form.matches.len(), 20);
        // Seven of these twenty ended inside the first minutes and must not count.
        let remakes = form.matches.iter().filter(|game| game.remake).count() as u32;
        assert!(remakes > 0);
        assert_eq!(form.games + remakes, 20);
        assert!(form.wins <= form.games);
        assert!(
            form.matches
                .windows(2)
                .all(|pair| pair[0].started_at >= pair[1].started_at),
            "newest first"
        );
        assert!(form.champions.len() <= 5);
        assert!(
            form.champions
                .iter()
                .map(|champion| champion.games)
                .sum::<u32>()
                <= form.games
        );
    }

    /// Two against two: a carry, a helper, a farmer on the other side and someone who left.
    fn duel(duration: i64) -> Game {
        let player = |id: i64, team: i64, stats: serde_json::Value| serde_json::json!({"participantId": id, "teamId": team, "championId": id, "stats": stats});
        serde_json::from_value(serde_json::json!({
            "gameId": 1, "gameDuration": duration,
            "participantIdentities": (1..=4).map(|id| serde_json::json!({"participantId": id, "player": {"puuid": format!("p{id}")}})).collect::<Vec<_>>(),
            "teams": [{"teamId": 100, "win": "Win"}, {"teamId": 200, "win": "Fail"}],
            "participants": [
                player(1, 100, serde_json::json!({"win": true, "kills": 10, "deaths": 2, "assists": 5, "largestMultiKill": 3, "largestKillingSpree": 8, "firstBloodKill": true, "turretKills": 2, "goldEarned": 15000, "totalDamageDealtToChampions": 30000, "totalDamageTaken": 20000, "totalMinionsKilled": 200})),
                player(2, 100, serde_json::json!({"win": true, "kills": 3, "deaths": 4, "assists": 12, "largestMultiKill": 1, "goldEarned": 9000, "totalDamageDealtToChampions": 10000, "totalDamageTaken": 35000, "totalMinionsKilled": 50})),
                player(3, 200, serde_json::json!({"kills": 4, "deaths": 6, "assists": 4, "largestMultiKill": 5, "turretKills": 2, "goldEarned": 11000, "totalDamageDealtToChampions": 20000, "totalDamageTaken": 15000, "totalMinionsKilled": 230, "neutralMinionsKilled": 20})),
                player(4, 200, serde_json::json!({"wasAfk": true, "deaths": 3, "goldEarned": 2000, "totalDamageDealtToChampions": 500, "totalDamageTaken": 3000, "totalMinionsKilled": 10}))
            ]
        }))
        .unwrap()
    }

    fn feats(detail: &MatchDetail) -> Vec<(String, Vec<Feat>)> {
        detail
            .teams
            .iter()
            .flat_map(|team| &team.players)
            .map(|line| (line.puuid.clone(), line.feats.clone()))
            .collect()
    }

    #[test]
    fn feats_name_a_lines_own_deeds_and_who_led_the_game() {
        use Feat::*;
        assert_eq!(
            feats(&match_detail(&duel(1800), &Roles::new())),
            vec![
                (
                    "p1".into(),
                    vec![
                        Legendary, Triple, MostKills, MostDamage, FirstBlood, MostTowers, MostGold
                    ]
                ),
                ("p2".into(), vec![MostAssists, MostTaken]),
                ("p3".into(), vec![Penta, MostTowers, MostCs],),
                ("p4".into(), vec![Afk]),
            ],
            "a tie leads together"
        );
        assert_eq!(
            feats(&match_detail(&duel(180), &Roles::new())),
            vec![
                ("p1".into(), vec![]),
                ("p2".into(), vec![]),
                ("p3".into(), vec![]),
                ("p4".into(), vec![Afk]),
            ],
            "a remake names only who was away"
        );
        let game = duel(1800);
        let summary = match_summary("p3", &game, &Roles::new(), &QueueKinds::new()).unwrap();
        assert_eq!(
            summary.line.feats,
            vec![Penta, MostTowers, MostCs],
            "a list row has them too"
        );
        let mut alone = game.clone();
        alone
            .participants
            .retain(|participant| participant.participant_id == 1);
        assert_eq!(
            match_summary("p1", &alone, &Roles::new(), &QueueKinds::new())
                .unwrap()
                .line
                .feats,
            vec![Legendary, Triple, FirstBlood],
            "one player's page leads nobody"
        );
    }

    fn game(created: i64, win: bool, kills: i64, deaths: i64, champion: i64, remake: bool) -> Game {
        serde_json::from_value(serde_json::json!({
            "gameId": created, "gameCreation": created, "gameDuration": if remake { 180 } else { 1800 },
            "participantIdentities": [{"participantId": 1, "player": {"puuid": "p"}}],
            "participants": [{"participantId": 1, "championId": champion, "stats": {"win": win, "kills": kills, "deaths": deaths, "assists": 2}}]
        }))
        .unwrap()
    }

    #[test]
    fn the_pace_reads_each_game_against_its_modes_average_and_skips_remakes() {
        let mode = |mut game: Game, mode: &str| {
            game.game_mode = mode.into();
            game
        };
        let games = [
            // Eleven deaths: a Rift game's double, an ARAM game's average.
            mode(game(4, true, 5, 11, 1, false), "KIWI"),
            mode(game(3, true, 5, 11, 1, false), "ARAM"),
            mode(game(2, false, 5, 11, 1, false), "CLASSIC"),
            mode(game(1, false, 0, 0, 1, true), "CLASSIC"),
            mode(game(0, false, 9, 9, 1, false), "CHERRY"),
        ];
        let pace = form_of("p", &games, &QueueKinds::new()).pace.unwrap();
        assert_eq!(pace.games, 3, "no remake, no mode without an average");
        assert!(
            (pace.deaths - 33.0 / (11.1 + 11.1 + 5.2)).abs() < 1e-9,
            "{pace:?}"
        );
        assert!((pace.assists - 6.0 / (25.6 + 25.6 + 7.5)).abs() < 1e-9);
        let arena = [mode(game(1, true, 9, 1, 1, false), "CHERRY")];
        assert_eq!(form_of("p", &arena, &QueueKinds::new()).pace, None);
    }

    #[test]
    fn a_game_with_every_player_is_scored_against_them_and_a_lone_row_against_its_mode() {
        let mut game = duel(1800);
        game.game_mode = "CLASSIC".into();
        let detail = match_detail(&game, &Roles::new());
        let scoreboard = detail.teams[0].players[0].score;
        let form = form_of("p1", std::slice::from_ref(&game), &QueueKinds::new());
        assert_eq!(form.matches[0].score, scoreboard, "the scoreboard's score");
        assert_eq!(form.source, Some(FormSource::Full));
        assert!(form.score.is_some());

        let mut alone = game.clone();
        alone.participants.retain(|row| row.participant_id == 1);
        let form = form_of("p1", &[alone.clone()], &QueueKinds::new());
        let stats = &alone.participants[0].stats;
        assert_eq!(
            form.matches[0].score,
            rating::lite_score(&contribution(stats, None), "CLASSIC", 1800)
        );
        assert_eq!(form.source, Some(FormSource::Lite));
        // Arena has no average: the game counts, unscored.
        alone.game_mode = "CHERRY".into();
        let form = form_of("p1", &[alone], &QueueKinds::new());
        assert_eq!(
            (form.games, form.matches[0].score, form.source),
            (1, None, None)
        );
        assert!(form.score.is_some(), "a strength from the result alone");
    }

    #[test]
    fn a_game_someone_else_left_is_marked_and_ones_own_is_not() {
        let game = duel(1800);
        let mate = form_of("p3", std::slice::from_ref(&game), &QueueKinds::new());
        assert!(mate.matches[0].away && mate.away == 1, "p4 left p3's side");
        let foe = form_of("p1", std::slice::from_ref(&game), &QueueKinds::new());
        assert!(foe.matches[0].away, "an enemy who left says as little");
        let gone = form_of("p4", &[game], &QueueKinds::new());
        assert!(
            !gone.matches[0].away && gone.away == 0,
            "leaving is the player's own"
        );
    }

    #[test]
    fn a_focus_reads_the_kind_of_game_being_played_and_never_mixes_in_another() {
        let mode = |mut game: Game, mode: &str| {
            game.game_mode = mode.into();
            game
        };
        let mut games: Vec<Game> = (0..10)
            .map(|at| mode(game(100 + at, true, 9, 1, 1, false), "CLASSIC"))
            .collect();
        games.extend((0..5).map(|at| mode(game(at, false, 1, 9, 2, false), "KIWI")));
        let catalog = Catalog {
            kinds: &QueueKinds::new(),
            roles: &Roles::new(),
        };
        let aram = recent_form("p", &games, &catalog, Some(ModeFamily::Aram));
        assert_eq!(
            (aram.family, aram.games, aram.wins),
            (Some(ModeFamily::Aram), 5, 0)
        );
        let rift = recent_form("p", &games, &catalog, Some(ModeFamily::Rift));
        assert_eq!((rift.family, rift.games), (Some(ModeFamily::Rift), 10));
        assert!(rift.score > aram.score);
        let every = recent_form("p", &games, &catalog, None);
        assert_eq!((every.family, every.games), (None, 15));
        // Four Arena games are few, and still only Arena's.
        games.extend((0..4).map(|at| mode(game(50 + at, true, 1, 1, 3, false), "CHERRY")));
        let arena = recent_form("p", &games, &catalog, Some(ModeFamily::Arena));
        assert_eq!((arena.family, arena.games), (Some(ModeFamily::Arena), 4));
        let none = recent_form("p", &games, &catalog, Some(ModeFamily::Other));
        assert_eq!(
            (none.family, none.games, none.score),
            (Some(ModeFamily::Other), 0, None),
            "no game of the mode, no strength in it"
        );
    }

    #[test]
    fn a_window_is_filled_by_twenty_games_against_players_of_the_focused_kind() {
        let mode = |mut game: Game, mode: &str| {
            game.game_mode = mode.into();
            game
        };
        let kinds = QueueKinds::new();
        let mut games: Vec<Game> = (0..19)
            .map(|at| mode(game(at, true, 1, 1, 1, false), "KIWI"))
            .collect();
        let mut custom = mode(game(50, true, 1, 1, 1, false), "KIWI");
        custom.game_type = "CUSTOM_GAME".into();
        games.push(custom);
        games.push(mode(game(60, true, 1, 1, 1, false), "CLASSIC"));
        assert!(
            !window_filled(&games, &kinds, Some(ModeFamily::Aram)),
            "a custom game takes no place"
        );
        assert!(
            window_filled(&games, &kinds, None),
            "twenty against players"
        );
        assert!(!window_filled(&games, &kinds, Some(ModeFamily::Rift)));
        games.push(mode(game(70, false, 1, 1, 1, true), "ARAM"));
        assert!(
            window_filled(&games, &kinds, Some(ModeFamily::Aram)),
            "a remake is in the window, shown and not counted"
        );
    }

    #[test]
    fn games_from_the_server_are_scored_against_their_whole_lobby() {
        let page: crate::sgp::History = fixture("live/responses/sgp-match-history-summary.json");
        let games: Vec<Game> = page.into_page().entries.into_iter().flatten().collect();
        let puuid = games[0].participant_identities[0].player.puuid.clone();
        let form = form_of(&puuid, &games, &QueueKinds::new());
        assert_eq!(form.source, Some(FormSource::Full));
        assert!(
            form.matches
                .iter()
                .all(|game| game.remake || game.score.is_some()),
            "{:?}",
            form.matches
        );
    }

    #[test]
    fn streaks_averages_and_pools_ignore_remakes() {
        let games = [
            game(5, true, 10, 0, 1, false),
            game(4, false, 0, 0, 1, true),
            game(3, true, 4, 2, 2, false),
            game(2, false, 1, 5, 1, false),
            game(1, true, 1, 1, 1, false),
        ];
        let form = form_of("p", &games, &QueueKinds::new());
        assert_eq!((form.games, form.wins, form.streak), (4, 3, 2));
        assert_eq!(form.kills, 4.0);
        assert_eq!(form.deaths, 2.0);
        assert_eq!(
            form.champions[0],
            ChampionForm {
                champion_id: 1,
                games: 3,
                wins: 2
            }
        );
        assert_eq!(form.matches.len(), 5);
        assert!(form.matches[1].remake);
    }

    #[test]
    fn custom_games_do_not_count_toward_form() {
        let mut custom = game(9, false, 0, 10, 1, false);
        custom.game_type = "CUSTOM_GAME".into();
        let form = form_of(
            "p",
            &[custom, game(5, true, 10, 0, 1, false)],
            &QueueKinds::new(),
        );
        assert_eq!((form.games, form.wins, form.matches.len()), (1, 1, 1));
        let many: Vec<Game> = (0..30).map(|at| game(at, true, 1, 1, 1, false)).collect();
        let form = form_of("p", &many, &QueueKinds::new());
        assert_eq!(form.matches.len(), RECENT_GAMES, "the newest twenty only");
        assert_eq!(form.matches[0].started_at, 29);
    }

    #[test]
    fn an_empty_history_is_all_zero() {
        let form = form_of("p", &[], &QueueKinds::new());
        assert_eq!(
            (form.games, form.wins, form.streak, form.kills),
            (0, 0, 0, 0.0)
        );
        assert_eq!(scope_of("p", &[], &QueueKinds::new()), FormScope::default());
    }

    /// The live catalog's kinds of queue (16.19, `fixtures/live/ranked/queues.json`).
    fn live_kinds() -> QueueKinds {
        let queues: Vec<crate::model::Queue> = fixture("live/ranked/queues.json");
        queues
            .iter()
            .map(|queue| (queue.id, crate::catalog::queue_kind(queue)))
            .collect()
    }

    /// A Tencent client's own list on GZ100 (16.19, 2026-10-06): thirty games, the newest two of
    /// them custom (an all-random ARAM lobby and a Hextech ARAM one), the rest Hextech ARAM with
    /// three remakes.
    fn client_list() -> Vec<Game> {
        fixture::<MatchList>("live/history/client-list-gz100.json")
            .games
            .games
    }

    #[test]
    fn form_reads_the_newest_twenty_games_against_players_on_a_measured_list() {
        let (games, kinds) = (client_list(), live_kinds());
        let form = form_of("PUUID-0001", &games, &kinds);
        let scope = scope_of("PUUID-0001", &games, &kinds);
        assert_eq!(
            (scope.listed, scope.custom, scope.bots, scope.remakes),
            (30, 2, 0, 2)
        );
        assert_eq!(form.matches.len(), RECENT_GAMES);
        // What the window showed as 10胜8负 · 56%, with nothing to say it was not ranked.
        assert_eq!((form.games, form.wins), (18, 10));
        assert!(
            form.matches.iter().all(|game| games
                .iter()
                .any(|listed| listed.game_id == game.game_id
                    && game_kind(listed, &kinds) == GameKind::Matched)),
            "custom games are passed over, not shown"
        );
        assert_eq!(
            form.matches.iter().filter(|game| game.remake).count(),
            2,
            "remakes are shown and not counted"
        );
    }

    #[test]
    fn games_against_the_computer_are_passed_over_like_custom_games() {
        let kinds = live_kinds();
        let mut games = client_list();
        // Co-op vs AI (VersusAi), Doom Bots (PvP, but NIGHTMARE_BOT) and the tutorial.
        games[2].queue_id = 870;
        games[3].queue_id = 4220;
        games[4].game_type = "TUTORIAL_GAME".into();
        assert_eq!(
            [&games[0], &games[2], &games[3], &games[4], &games[5]]
                .map(|game| game_kind(game, &kinds)),
            [
                GameKind::Custom,
                GameKind::Bots,
                GameKind::Bots,
                GameKind::Bots,
                GameKind::Matched
            ]
        );
        let form = form_of("PUUID-0001", &games, &kinds);
        let scope = scope_of("PUUID-0001", &games, &kinds);
        assert_eq!((scope.custom, scope.bots), (2, 3));
        assert_eq!(
            form.matches.len(),
            RECENT_GAMES,
            "the window reaches further back"
        );
        assert!(form.matches.iter().all(|game| {
            ![games[2].game_id, games[3].game_id, games[4].game_id].contains(&game.game_id)
        }));
        assert_eq!(form.matches[0].game_id, games[5].game_id);
        assert_eq!(
            form.matches.last().map(|game| game.game_id),
            Some(games[24].game_id)
        );
    }

    #[test]
    fn a_games_own_type_decides_before_its_queue() {
        let kinds = live_kinds();
        let mut custom = game(1, true, 1, 1, 1, false);
        custom.game_type = "CUSTOM_GAME".into();
        custom.queue_id = 450;
        assert_eq!(
            game_kind(&custom, &kinds),
            GameKind::Custom,
            "a custom ARAM"
        );
        let mut unlisted = game(1, true, 1, 1, 1, false);
        unlisted.queue_id = 987_654;
        assert_eq!(game_kind(&unlisted, &kinds), GameKind::Matched);
        assert_eq!(
            game_kind(&unlisted, &QueueKinds::new()),
            GameKind::Matched,
            "without a catalog"
        );
        let summary = match_summary("p", &custom, &Roles::new(), &kinds).unwrap();
        assert_eq!(summary.kind, GameKind::Custom, "a list row says it too");
    }

    #[test]
    fn match_detail_groups_ten_players_into_two_teams() {
        let game: Game = fixture("live/ranked/match-game.json");
        let detail = match_detail(&game, &Roles::new());
        assert_eq!(detail.teams.len(), 2);
        assert_eq!(
            detail
                .teams
                .iter()
                .map(|team| team.players.len())
                .sum::<usize>(),
            game.participants.len()
        );
        assert_eq!(detail.teams.iter().filter(|team| team.win).count(), 1);
        let kills: i64 = game
            .participants
            .iter()
            .map(|participant| participant.stats.kills)
            .sum();
        assert_eq!(
            detail.teams.iter().map(|team| team.kills).sum::<i64>(),
            kills
        );
    }

    #[test]
    fn a_full_scoreboard_scores_every_line_and_names_one_mvp_and_one_svp() {
        let game: Game = fixture("live/ranked/match-game.json");
        let detail = match_detail(&game, &Roles::new());
        let lines: Vec<&PlayerLine> = detail.teams.iter().flat_map(|team| &team.players).collect();
        assert!(lines.iter().all(|line| {
            line.score
                .is_some_and(|score| (0.0..=10.0).contains(&score))
        }));
        let awards = |award: Award| {
            lines
                .iter()
                .filter(|line| line.award == Some(award))
                .count()
        };
        assert_eq!((awards(Award::Mvp), awards(Award::Svp)), (1, 1));
        let mvp = lines
            .iter()
            .find(|line| line.award == Some(Award::Mvp))
            .unwrap();
        assert!(
            mvp.win
                && lines
                    .iter()
                    .filter(|line| line.win)
                    .all(|line| line.score <= mvp.score)
        );
        for team in &detail.teams {
            let share: f64 = team
                .players
                .iter()
                .filter_map(|line| line.damage_share)
                .sum();
            assert!(
                (share - 1.0).abs() < 1e-9,
                "a team's damage shares add up to one"
            );
            assert!(team.players.iter().all(|line| {
                line.kill_participation
                    .is_some_and(|kp| (0.0..=1.0).contains(&kp))
            }));
        }
    }

    #[test]
    fn a_remake_gets_no_scores() {
        let mut game: Game = fixture("live/ranked/match-game.json");
        game.game_duration = 200;
        let detail = match_detail(&game, &Roles::new());
        assert!(
            detail
                .teams
                .iter()
                .flat_map(|team| &team.players)
                .all(|line| line.score.is_none() && line.award.is_none())
        );
    }
}
