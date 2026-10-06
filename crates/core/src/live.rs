//! Champ select and the running game, as the window and the plugin draw them.

use std::collections::HashMap;

use crate::{
    callout::{self, Ranking},
    model::{ChampSelectPlayer, ChampSelectSession, GamePlayer, GameflowSession, Lobby},
    rating,
    view::{
        ChampSelectView, GameView, LanePreference, LobbyMember, LobbyView, Phase, PlayerStats,
        Position, RiotId, Seat, SeatRating, Side, TimerView,
    },
};

/// Modes not played between a blue and a red side: Arena's pairs and Swarm's co-op.
const SIDELESS_MODES: [&str; 2] = ["CHERRY", "STRAWBERRY"];

/// Whether `mode` (`CLASSIC`, `ARAM`, `KIWI`, …) is played between a blue and a red side. An unknown
/// mode is taken to be.
pub fn has_sides(mode: &str) -> bool {
    !SIDELESS_MODES
        .iter()
        .any(|sideless| mode.eq_ignore_ascii_case(sideless))
}

fn side(team: i64) -> Option<Side> {
    match team {
        1 => Some(Side::Blue),
        2 => Some(Side::Red),
        _ => None,
    }
}

/// The puuids whose stats a champ select view needs, local player included.
pub fn champ_select_puuids(session: &ChampSelectSession) -> Vec<String> {
    session
        .my_team
        .iter()
        .chain(&session.their_team)
        .filter_map(visible_puuid)
        .collect()
}

pub fn game_puuids(session: &GameflowSession) -> Vec<String> {
    let data = &session.game_data;
    data.team_one
        .iter()
        .chain(&data.team_two)
        .map(|player| player.puuid.clone())
        .filter(|puuid| !puuid.is_empty())
        .collect()
}

fn visible_puuid(player: &ChampSelectPlayer) -> Option<String> {
    let hidden =
        player.puuid.is_empty() || player.name_visibility_type.eq_ignore_ascii_case("HIDDEN");
    (!hidden).then(|| player.puuid.clone())
}

/// Rates every seat of one team by recent form: a grade of its own where the ranking is absolute,
/// otherwise the team split into as many tiers as there are names, best first.
fn rate(seats: &mut [Seat], ranking: &Ranking, game_id: i64) {
    let names = &ranking.names;
    if names.is_empty() {
        return;
    }
    let scores: Vec<Option<f64>> = seats
        .iter()
        .map(|seat| match &seat.stats {
            PlayerStats::Ready(summary) => rating::form_score(&summary.recent),
            _ => None,
        })
        .collect();
    let tiers = if ranking.absolute {
        scores
            .iter()
            .map(|score| score.map(|score| rating::grade(score, &rating::FORM_GRADES)))
            .collect()
    } else {
        rating::tiers(&scores, names.len())
    };
    let count = names.len() as u8;
    for ((seat, score), tier) in seats.iter_mut().zip(&scores).zip(tiers) {
        let tier = tier.map(|tier| tier.min(count - 1));
        let grade = ranking.absolute.then_some(tier).flatten();
        // The title leans the way its tier does.
        let title = match (&seat.stats, tier) {
            (PlayerStats::Ready(summary), Some(tier)) if ranking.titles => {
                rating::form_title(&summary.recent, rating::lean(tier, count, grade))
                    .map(|title| callout::title_name(title, ranking.language).to_owned())
            }
            _ => None,
        };
        let puuid = seat.puuid.clone().unwrap_or_default();
        seat.rating = score.zip(tier).map(|(score, tier)| SeatRating {
            score,
            tier,
            tiers: count,
            label: names[usize::from(tier)].clone(),
            grade,
            title,
            quip: ranking.quip(tier, &puuid, game_id),
        });
    }
}

/// The champ select as drawn, for a queue of `mode`. `callout` is left empty: its lines need the
/// catalog and settings.
pub fn champ_select_view(
    session: &ChampSelectSession,
    stats: impl Fn(&str) -> PlayerStats,
    ranking: &Ranking,
    mode: &str,
) -> ChampSelectView {
    let seat = |player: &ChampSelectPlayer| {
        let puuid = visible_puuid(player);
        let champion_id = if player.champion_id > 0 {
            player.champion_id
        } else {
            player.champion_pick_intent
        };
        Seat {
            stats: puuid.as_deref().map_or(PlayerStats::Hidden, &stats),
            name: puuid
                .as_ref()
                .and_then(|_| RiotId::new(&player.game_name, &player.tag_line)),
            puuid,
            champion_id,
            intent: player.champion_id == 0 && player.champion_pick_intent > 0,
            position: Position::parse(&player.assigned_position),
            spells: [player.spell1_id, player.spell2_id],
            is_self: player.cell_id == session.local_player_cell_id,
            premade: None,
            rating: None,
        }
    };
    let mut my_team: Vec<&ChampSelectPlayer> = session.my_team.iter().collect();
    my_team.sort_by_key(|player| player.cell_id);
    let mut their_team: Vec<&ChampSelectPlayer> = session.their_team.iter().collect();
    their_team.sort_by_key(|player| player.cell_id);

    let timer = &session.timer;
    let ends_at = if timer.is_infinite || timer.internal_now_in_epoch_ms == 0 {
        0
    } else {
        timer.internal_now_in_epoch_ms + timer.adjusted_time_left_in_phase.max(0)
    };

    let mut my_team: Vec<Seat> = my_team.into_iter().map(seat).collect();
    let mut their_team: Vec<Seat> = their_team.into_iter().map(seat).collect();
    rate(&mut my_team, ranking, session.game_id);
    rate(&mut their_team, ranking, session.game_id);

    ChampSelectView {
        game_id: session.game_id,
        queue_id: session.queue_id,
        timer: TimerView {
            phase: timer.phase.clone(),
            ends_at,
            total_ms: timer.total_time_in_phase,
        },
        my_team,
        their_team,
        my_bans: session
            .bans
            .my_team_bans
            .iter()
            .copied()
            .filter(|&id| id > 0)
            .collect(),
        their_bans: session
            .bans
            .their_team_bans
            .iter()
            .copied()
            .filter(|&id| id > 0)
            .collect(),
        bench_enabled: session.bench_enabled,
        bench: session
            .bench_champions
            .iter()
            .map(|bench| bench.champion_id)
            .filter(|&id| id > 0)
            .collect(),
        rerolls_remaining: session.rerolls_remaining.max(0),
        callout: Vec::new(),
        side: session
            .local_player()
            .and_then(|player| side(player.team))
            .filter(|_| has_sides(mode)),
    }
}

/// Both teams of the loaded game; `None` before the gameflow session names them. `callout` and
/// `ally_callout` are left empty: their lines need the catalog and settings.
pub fn game_view(
    session: &GameflowSession,
    me: &str,
    stats: impl Fn(&str) -> PlayerStats,
    ranking: &Ranking,
) -> Option<GameView> {
    let data = &session.game_data;
    if data.team_one.is_empty() && data.team_two.is_empty() {
        return None;
    }
    let selections: HashMap<&str, [i64; 2]> = data
        .player_champion_selections
        .iter()
        .map(|selection| {
            (
                selection.puuid.as_str(),
                [selection.spell1_id, selection.spell2_id],
            )
        })
        .collect();
    let mut next_group = 0u8;
    let teams = [&data.team_one, &data.team_two]
        .into_iter()
        .map(|team| {
            let parties = premade_parties(team, &mut next_group);
            team.iter()
                .map(|player| {
                    let puuid = (!player.puuid.is_empty()).then(|| player.puuid.clone());
                    Seat {
                        stats: puuid.as_deref().map_or(PlayerStats::Hidden, &stats),
                        name: RiotId::new(&player.game_name, &player.tag_line),
                        champion_id: player.champion_id,
                        intent: false,
                        position: Position::parse(&player.selected_position),
                        spells: selections
                            .get(player.puuid.as_str())
                            .copied()
                            .unwrap_or_default(),
                        is_self: !me.is_empty() && player.puuid == me,
                        premade: parties.get(&player.team_participant_id).copied(),
                        puuid,
                        rating: None,
                    }
                })
                .collect::<Vec<_>>()
        })
        .map(|mut seats| {
            rate(&mut seats, ranking, data.game_id);
            seats
        })
        .collect();
    Some(GameView {
        game_id: data.game_id,
        queue_id: data.queue.id,
        teams,
        sides: has_sides(&data.queue.game_mode),
        callout: Vec::new(),
        ally_callout: Vec::new(),
    })
}

// ---- Social: the lobby, and the party it leaves for champ select ----

/// The phases in which the client shows the lobby: in it, searching, and the match-found dialog
/// over it.
pub fn shows_lobby(phase: Phase) -> bool {
    matches!(phase, Phase::Lobby | Phase::Matchmaking | Phase::ReadyCheck)
}

/// The members of `lobby` whose stats the lobby view needs; bots have none.
pub fn lobby_puuids(lobby: &Lobby) -> Vec<String> {
    lobby
        .members
        .iter()
        .filter(|member| !member.is_bot && !member.puuid.is_empty())
        .map(|member| member.puuid.clone())
        .collect()
}

/// The party a lobby makes for champ select: every member, unless the lobby is a custom game's,
/// where everyone in it plays, on both teams.
pub fn party_of(lobby: &Lobby) -> Vec<String> {
    if lobby.game_config.is_custom {
        return Vec::new();
    }
    lobby_puuids(lobby)
}

/// The lobby as drawn, `me` marked, each member with the stats known now.
pub fn lobby_view(lobby: &Lobby, me: &str, stats: impl Fn(&str) -> PlayerStats) -> LobbyView {
    let members = lobby
        .members
        .iter()
        .filter(|member| !member.is_bot && !member.puuid.is_empty())
        .map(|member| {
            let stats = stats(&member.puuid);
            let summary = match &stats {
                PlayerStats::Ready(summary) => Some(summary),
                _ => None,
            };
            let mut positions = Vec::with_capacity(2);
            for lane in [
                &member.first_position_preference,
                &member.second_position_preference,
            ]
            .into_iter()
            .filter_map(|preference| LanePreference::parse(preference))
            {
                if !positions.contains(&lane) {
                    positions.push(lane);
                }
            }
            LobbyMember {
                name: RiotId::new(&member.game_name, &member.tag_line)
                    .or_else(|| summary.and_then(|summary| summary.name.clone())),
                icon_id: if member.summoner_icon_id > 0 {
                    member.summoner_icon_id
                } else {
                    summary.map_or(0, |summary| summary.icon_id)
                },
                is_self: !me.is_empty() && member.puuid == me,
                leader: member.is_leader,
                positions,
                score: summary.and_then(|summary| rating::form_score(&summary.recent)),
                puuid: member.puuid.clone(),
                stats,
            }
        })
        .collect();
    LobbyView {
        queue_id: lobby.game_config.queue_id,
        custom: lobby.game_config.is_custom,
        members,
    }
}

/// Marks the seats of the local player's party as premade 1, where two or more of it sit in the
/// team: champ select does not say who queued together, the lobby before it did.
pub fn mark_party(seats: &mut [Seat], party: &[String]) {
    let ours = |seat: &Seat| {
        seat.puuid
            .as_ref()
            .is_some_and(|puuid| party.contains(puuid))
    };
    if seats.iter().filter(|seat| ours(seat)).count() < 2 {
        return;
    }
    for seat in seats.iter_mut().filter(|seat| ours(seat)) {
        seat.premade = Some(1);
    }
}

/// Party ids shared by two or more players, numbered in order across both teams.
fn premade_parties(team: &[GamePlayer], next: &mut u8) -> HashMap<i64, u8> {
    let mut sizes: Vec<(i64, usize)> = Vec::new();
    for player in team.iter().filter(|player| player.team_participant_id > 0) {
        match sizes
            .iter_mut()
            .find(|(id, _)| *id == player.team_participant_id)
        {
            Some((_, size)) => *size += 1,
            None => sizes.push((player.team_participant_id, 1)),
        }
    }
    sizes
        .into_iter()
        .filter(|&(_, size)| size > 1)
        .map(|(id, _)| {
            *next += 1;
            (id, *next)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_support::fixture,
        view::{PlayerSummary, Ranked, RecentForm},
    };

    fn ranking() -> Ranking {
        Ranking {
            set: crate::settings::TierSet::Custom,
            names: vec!["上".into(), "中".into(), "下".into()],
            absolute: false,
            language: crate::settings::Language::ZhCn,
            titles: false,
        }
    }

    #[test]
    fn a_captured_ban_pick_session_becomes_a_view() {
        let session: ChampSelectSession =
            fixture("lcu-rest/lol-champ-select-v1-session--champ-select-ban-pick.json");
        let view = champ_select_view(&session, |_| PlayerStats::Loading, &ranking(), "ARAM");
        assert_eq!(view.my_team.len(), session.my_team.len());
        assert_eq!(view.queue_id, 3110);
        assert_eq!(view.timer.phase, "BAN_PICK");
        assert_eq!(
            view.timer.ends_at,
            session.timer.internal_now_in_epoch_ms + session.timer.adjusted_time_left_in_phase
        );
        let me = view
            .my_team
            .iter()
            .find(|seat| seat.is_self)
            .expect("the local player has a seat");
        assert_eq!(me.position, Some(Position::Top));
        assert_eq!(me.stats, PlayerStats::Loading);
    }

    #[test]
    fn intents_show_until_a_pick_replaces_them_and_hidden_players_get_no_stats() {
        let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 1,
            "myTeam": [
                {"cellId": 1, "championId": 0, "championPickIntent": 64, "puuid": "a", "gameName": "Ann", "tagLine": "1"},
                {"cellId": 0, "championId": 157, "championPickIntent": 64, "puuid": "b", "gameName": "Bo", "tagLine": "2"},
                {"cellId": 2, "puuid": "", "nameVisibilityType": "HIDDEN"}
            ]
        }))
        .unwrap();
        let view = champ_select_view(
            &session,
            |puuid| PlayerStats::Failed {
                message: puuid.to_owned(),
            },
            &ranking(),
            "CLASSIC",
        );
        let ids: Vec<i64> = view.my_team.iter().map(|seat| seat.champion_id).collect();
        assert_eq!(ids, vec![157, 64, 0], "seats are in cell order");
        assert!(!view.my_team[0].intent && view.my_team[1].intent);
        assert!(view.my_team[1].is_self);
        assert_eq!(view.my_team[2].stats, PlayerStats::Hidden);
        assert_eq!(view.my_team[2].name, None);
        assert_eq!(
            champ_select_puuids(&session),
            vec!["a".to_owned(), "b".to_owned()]
        );
    }

    #[test]
    fn players_sharing_a_party_id_are_marked_premade() {
        let session: GameflowSession = serde_json::from_value(serde_json::json!({
            "phase": "InProgress",
            "gameData": {
                "gameId": 7, "queue": {"id": 420},
                "teamOne": [
                    {"puuid": "a", "championId": 1, "teamParticipantId": 10},
                    {"puuid": "b", "championId": 2, "teamParticipantId": 10},
                    {"puuid": "c", "championId": 3, "teamParticipantId": 11}
                ],
                "teamTwo": [
                    {"puuid": "d", "championId": 4, "teamParticipantId": 20},
                    {"puuid": "e", "championId": 5, "teamParticipantId": 20}
                ],
                "playerChampionSelections": [{"puuid": "a", "spell1Id": 4, "spell2Id": 14}]
            }
        }))
        .unwrap();
        let view = game_view(&session, "c", |_| PlayerStats::Loading, &ranking()).unwrap();
        let groups: Vec<Vec<Option<u8>>> = view
            .teams
            .iter()
            .map(|team| team.iter().map(|seat| seat.premade).collect())
            .collect();
        assert_eq!(
            groups,
            vec![vec![Some(1), Some(1), None], vec![Some(2), Some(2)]]
        );
        assert_eq!(view.teams[0][0].spells, [4, 14]);
        assert!(view.teams[0][2].is_self);
        assert!(view.sides, "a game of an unnamed mode has sides");
        assert_eq!(game_puuids(&session).len(), 5);
    }

    #[test]
    fn rated_teammates_get_standings_and_the_bench_comes_through() {
        let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 0,
            "benchEnabled": true,
            "benchChampions": [{"championId": 22, "isPriority": false}, {"championId": 0}],
            "rerollsRemaining": 1,
            "myTeam": [
                {"cellId": 0, "puuid": "a", "gameName": "A"},
                {"cellId": 1, "puuid": "b", "gameName": "B"},
                {"cellId": 2, "puuid": "c", "gameName": "C"}
            ]
        }))
        .unwrap();
        let stats = |puuid: &str| {
            let wins = match puuid {
                "a" => 15,
                "b" => 5,
                _ => return PlayerStats::Loading,
            };
            let recent = RecentForm {
                games: 20,
                wins,
                kills: 5.0,
                deaths: 5.0,
                assists: 5.0,
                score: Some(f64::from(wins) / 2.0),
                ..RecentForm::default()
            };
            PlayerStats::Ready(Box::new(PlayerSummary {
                puuid: puuid.into(),
                name: None,
                level: 1,
                icon_id: 1,
                private: false,
                ranked: Ranked::default(),
                recent,
            }))
        };
        let view = champ_select_view(&session, stats, &ranking(), "KIWI");
        let tiers: Vec<Option<(u8, u8)>> = view
            .my_team
            .iter()
            .map(|seat| {
                seat.rating
                    .as_ref()
                    .map(|rating| (rating.tier, rating.tiers))
            })
            .collect();
        assert_eq!(tiers, vec![Some((0, 3)), Some((2, 3)), None]);
        assert_eq!(view.my_team[0].rating.as_ref().unwrap().label, "上");
        assert_eq!(
            view.my_team[0].rating.as_ref().unwrap().grade,
            None,
            "a place in the team is not a grade"
        );
        assert_eq!(
            (
                view.bench_enabled,
                view.bench.clone(),
                view.rerolls_remaining
            ),
            (true, vec![22], 1)
        );
    }

    #[test]
    fn the_local_team_is_on_the_side_its_team_number_names_where_the_mode_has_sides() {
        let side = |team: i64, mode: &str| {
            let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
                "localPlayerCellId": 7,
                "myTeam": [{"cellId": 7, "team": team, "puuid": "a"}]
            }))
            .unwrap();
            champ_select_view(&session, |_| PlayerStats::Loading, &ranking(), mode).side
        };
        assert_eq!(side(1, "ARAM"), Some(Side::Blue));
        assert_eq!(side(2, "KIWI"), Some(Side::Red));
        assert_eq!(side(2, ""), Some(Side::Red), "an unknown mode has sides");
        assert_eq!(side(2, "CHERRY"), None, "Arena has none");
        assert_eq!(side(0, "CLASSIC"), None);
        let captured: ChampSelectSession =
            fixture("lcu-rest/lol-champ-select-v1-session--champ-select-ban-pick.json");
        let view = champ_select_view(&captured, |_| PlayerStats::Loading, &ranking(), "CLASSIC");
        assert_eq!(view.side, Some(Side::Blue));
    }

    #[test]
    fn absolute_grades_rate_each_player_on_their_own_and_titles_come_along() {
        let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 0,
            "myTeam": [
                {"cellId": 0, "puuid": "a", "gameName": "A"},
                {"cellId": 1, "puuid": "b", "gameName": "B"}
            ]
        }))
        .unwrap();
        // Two equally strong players: ranked against each other one would be "best", the other
        // "worst"; graded, both earn the same grade.
        let stats = |puuid: &str| {
            PlayerStats::Ready(Box::new(PlayerSummary {
                puuid: puuid.into(),
                name: None,
                level: 1,
                icon_id: 1,
                private: false,
                ranked: Ranked::default(),
                recent: RecentForm {
                    games: 20,
                    wins: 15,
                    kills: 8.0,
                    deaths: 3.0,
                    assists: 10.0,
                    streak: 4,
                    score: Some(9.0),
                    ..RecentForm::default()
                },
            }))
        };
        let grades = Ranking {
            set: crate::settings::TierSet::Grades,
            names: (0..8).map(|grade| format!("g{grade}")).collect(),
            absolute: true,
            language: crate::settings::Language::ZhCn,
            titles: true,
        };
        let view = champ_select_view(&session, stats, &grades, "CLASSIC");
        let rated: Vec<(u8, String, Option<String>)> = view
            .my_team
            .iter()
            .map(|seat| {
                let rating = seat.rating.as_ref().unwrap();
                (rating.tier, rating.label.clone(), rating.title.clone())
            })
            .collect();
        assert_eq!(rated[0], rated[1]);
        assert_eq!(rated[0].0, 1, "an S");
        assert_eq!(rated[0].2.as_deref(), Some("版本答案"));
        assert_eq!(
            view.my_team[0].rating.as_ref().unwrap().grade,
            Some(1),
            "a grade says it is one"
        );
    }

    fn ready(puuid: &str, wins: u32) -> PlayerStats {
        PlayerStats::Ready(Box::new(PlayerSummary {
            puuid: puuid.into(),
            name: Some(RiotId {
                game_name: format!("{puuid}-summoner"),
                tag_line: "9".into(),
            }),
            level: 1,
            icon_id: 7,
            private: false,
            ranked: Ranked::default(),
            recent: RecentForm {
                games: 20,
                wins,
                kills: 5.0,
                deaths: 5.0,
                assists: 5.0,
                score: Some(f64::from(wins) / 2.0),
                ..RecentForm::default()
            },
        }))
    }

    fn lobby(custom: bool) -> Lobby {
        serde_json::from_value(serde_json::json!({
            "gameConfig": {"queueId": 420, "gameMode": "CLASSIC", "isCustom": custom},
            "members": [
                {"puuid": "me", "gameName": "Me", "gameTag": "1", "summonerIconId": 29, "isLeader": true,
                 "firstPositionPreference": "MIDDLE", "secondPositionPreference": "FILL"},
                {"puuid": "mate", "summonerIconId": 0, "firstPositionPreference": "UTILITY",
                 "secondPositionPreference": "UTILITY"},
                {"puuid": "", "isBot": true, "summonerIconId": 1},
                {"puuid": "late", "gameName": "Late", "tagLine": "2", "firstPositionPreference": "UNSELECTED"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_lobby_shows_its_members_with_their_lanes_form_and_score() {
        let stats = |puuid: &str| match puuid {
            "me" => ready("me", 15),
            "mate" => ready("mate", 5),
            _ => PlayerStats::Loading,
        };
        let view = lobby_view(&lobby(false), "me", stats);
        assert_eq!((view.queue_id, view.custom), (420, false));
        let members: Vec<(&str, bool, bool, Vec<LanePreference>)> = view
            .members
            .iter()
            .map(|member| {
                (
                    member.puuid.as_str(),
                    member.is_self,
                    member.leader,
                    member.positions.clone(),
                )
            })
            .collect();
        assert_eq!(
            members,
            vec![
                (
                    "me",
                    true,
                    true,
                    vec![LanePreference::Middle, LanePreference::Fill]
                ),
                ("mate", false, false, vec![LanePreference::Utility]),
                ("late", false, false, vec![]),
            ],
            "the bot is left out, a lane asked for twice counts once"
        );
        assert_eq!(view.members[0].name.as_ref().unwrap().game_name, "Me");
        assert_eq!(
            view.members[1].name.as_ref().unwrap().game_name,
            "mate-summoner",
            "a member the lobby does not name is named by their summary"
        );
        assert_eq!(view.members[1].icon_id, 7);
        assert_eq!(view.members[2].name.as_ref().unwrap().tag_line, "2");
        let scores: Vec<Option<f64>> = view.members.iter().map(|member| member.score).collect();
        assert!(scores[0] > scores[1] && scores[1].is_some() && scores[2].is_none());
        assert_eq!(view.members[2].stats, PlayerStats::Loading);
        assert_eq!(lobby_puuids(&lobby(false)), ["me", "mate", "late"]);
    }

    #[test]
    fn champ_select_marks_the_party_the_lobby_made_and_a_custom_lobby_makes_none() {
        let seat = |puuid: &str| Seat {
            puuid: Some(puuid.into()),
            name: None,
            champion_id: 1,
            intent: false,
            position: None,
            spells: [0, 0],
            is_self: false,
            premade: None,
            stats: PlayerStats::Loading,
            rating: None,
        };
        let party = party_of(&lobby(false));
        let mut team = vec![seat("me"), seat("stranger"), seat("mate")];
        mark_party(&mut team, &party);
        let marks: Vec<Option<u8>> = team.iter().map(|seat| seat.premade).collect();
        assert_eq!(marks, vec![Some(1), None, Some(1)]);

        let mut alone = vec![seat("me"), seat("stranger")];
        mark_party(&mut alone, &party);
        assert!(
            alone.iter().all(|seat| seat.premade.is_none()),
            "one of the party in the team is no premade"
        );
        assert!(party_of(&lobby(true)).is_empty());
        assert!(shows_lobby(Phase::Matchmaking) && !shows_lobby(Phase::ChampSelect));
    }

    #[test]
    fn an_empty_gameflow_session_has_no_game() {
        assert_eq!(
            game_view(
                &GameflowSession::default(),
                "",
                |_| PlayerStats::Loading,
                &ranking()
            ),
            None
        );
    }
}
