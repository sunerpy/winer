//! Folding LCU documents into the views: ranks, recent form and scoreboard lines.

use std::{cmp::Reverse, collections::HashMap};

use crate::{
    model::{Game, Participant, RankedEntry, RankedStats, Stats, Summoner},
    rating::{self, Contribution, Roles},
    view::{
        ChampionForm, Feat, FormScope, GameKind, MatchDetail, MatchSummary, PlayerLine,
        PlayerProfile, PlayerSummary, Position, Rank, Ranked, RecentForm, RecentMatch, RiotId,
        TeamDetail, Tier,
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

pub fn summary(
    summoner: &Summoner,
    ranked_stats: Option<&RankedStats>,
    games: &[Game],
    kinds: &QueueKinds,
) -> PlayerSummary {
    let profile = profile(summoner, ranked_stats);
    PlayerSummary {
        recent: recent_form(&summoner.puuid, games, kinds),
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

/// `puuid`'s recent games, newest first, folded into averages, a streak and a champion pool.
/// Only games against other players count: custom games (practice, lobbies among friends) and games
/// against the computer are passed over on the way to the newest [`RECENT_GAMES`], and the
/// remakes among those are shown but left out of every figure.
pub fn recent_form(puuid: &str, games: &[Game], kinds: &QueueKinds) -> RecentForm {
    form(puuid, games, kinds).0
}

/// What [`recent_form`] reads from `games` and what it leaves out.
pub fn form_scope(puuid: &str, games: &[Game], kinds: &QueueKinds) -> FormScope {
    form(puuid, games, kinds).1
}

fn form(puuid: &str, games: &[Game], kinds: &QueueKinds) -> (RecentForm, FormScope) {
    let mut newest: Vec<&Game> = games.iter().collect();
    newest.sort_by_key(|game| Reverse(game.game_creation));
    let mut scope = FormScope {
        listed: games.len() as u32,
        ..FormScope::default()
    };
    let mut games = Vec::with_capacity(RECENT_GAMES);
    for game in newest {
        if games.len() == RECENT_GAMES {
            break;
        }
        match game_kind(game, kinds) {
            GameKind::Matched => games.push(game),
            GameKind::Custom => scope.custom += 1,
            GameKind::Bots => scope.bots += 1,
        }
    }

    let matches: Vec<RecentMatch> = games
        .iter()
        .filter_map(|game| {
            // List pages carry only this player's participant row; detail pages carry ten.
            let participant = game
                .participant_of(puuid)
                .or_else(|| game.participants.first())?;
            Some(RecentMatch {
                game_id: game.game_id,
                queue_id: game.queue_id,
                champion_id: participant.champion_id,
                win: participant.stats.win,
                remake: is_remake(game, participant),
                kills: participant.stats.kills,
                deaths: participant.stats.deaths,
                assists: participant.stats.assists,
                started_at: game.game_creation,
            })
        })
        .collect();

    let counted: Vec<&RecentMatch> = matches.iter().filter(|game| !game.remake).collect();
    let total = counted.len() as u32;
    let average = |value: fn(&RecentMatch) -> i64| {
        if total == 0 {
            0.0
        } else {
            counted.iter().map(|game| value(game)).sum::<i64>() as f64 / f64::from(total)
        }
    };

    let streak = match counted.first() {
        Some(first) => {
            let run = counted
                .iter()
                .take_while(|game| game.win == first.win)
                .count() as i32;
            if first.win { run } else { -run }
        }
        None => 0,
    };

    let mut pool: HashMap<i64, ChampionForm> = HashMap::new();
    for game in &counted {
        let form = pool.entry(game.champion_id).or_insert(ChampionForm {
            champion_id: game.champion_id,
            games: 0,
            wins: 0,
        });
        form.games += 1;
        form.wins += u32::from(game.win);
    }
    let mut champions: Vec<ChampionForm> = pool.into_values().collect();
    champions.sort_by(|a, b| {
        b.games
            .cmp(&a.games)
            .then(b.wins.cmp(&a.wins))
            .then(a.champion_id.cmp(&b.champion_id))
    });
    champions.truncate(5);
    scope.remakes = matches.len() as u32 - total;

    // The counted games against their modes' averages, for the title.
    let pace = rating::Pace::of(games.iter().filter_map(|game| {
        let participant = game
            .participant_of(puuid)
            .or_else(|| game.participants.first())?;
        let average = rating::average_line(&game.game_mode)?;
        let stats = &participant.stats;
        (!is_remake(game, participant))
            .then_some(([stats.kills, stats.deaths, stats.assists], average))
    }));

    let form = RecentForm {
        games: total,
        wins: counted.iter().filter(|game| game.win).count() as u32,
        kills: average(|game| game.kills),
        deaths: average(|game| game.deaths),
        assists: average(|game| game.assists),
        streak,
        matches,
        champions,
        pace,
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
        let form = recent_form(&puuid, &list.games.games, &QueueKinds::new());
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
        let pace = recent_form("p", &games, &QueueKinds::new()).pace.unwrap();
        assert_eq!(pace.games, 3, "no remake, no mode without an average");
        assert!(
            (pace.deaths - 33.0 / (11.1 + 11.1 + 5.2)).abs() < 1e-9,
            "{pace:?}"
        );
        assert!((pace.assists - 6.0 / (25.6 + 25.6 + 7.5)).abs() < 1e-9);
        let arena = [mode(game(1, true, 9, 1, 1, false), "CHERRY")];
        assert_eq!(recent_form("p", &arena, &QueueKinds::new()).pace, None);
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
        let form = recent_form("p", &games, &QueueKinds::new());
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
        let form = recent_form(
            "p",
            &[custom, game(5, true, 10, 0, 1, false)],
            &QueueKinds::new(),
        );
        assert_eq!((form.games, form.wins, form.matches.len()), (1, 1, 1));
        let many: Vec<Game> = (0..30).map(|at| game(at, true, 1, 1, 1, false)).collect();
        let form = recent_form("p", &many, &QueueKinds::new());
        assert_eq!(form.matches.len(), RECENT_GAMES, "the newest twenty only");
        assert_eq!(form.matches[0].started_at, 29);
    }

    #[test]
    fn an_empty_history_is_all_zero() {
        let form = recent_form("p", &[], &QueueKinds::new());
        assert_eq!(
            (form.games, form.wins, form.streak, form.kills),
            (0, 0, 0, 0.0)
        );
        assert_eq!(
            form_scope("p", &[], &QueueKinds::new()),
            FormScope::default()
        );
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
        let form = recent_form("PUUID-0001", &games, &kinds);
        let scope = form_scope("PUUID-0001", &games, &kinds);
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
        let form = recent_form("PUUID-0001", &games, &kinds);
        let scope = form_scope("PUUID-0001", &games, &kinds);
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
