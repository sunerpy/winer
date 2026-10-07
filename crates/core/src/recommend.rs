//! Pick suggestions in champ select, shown and nothing more: no champion is hovered, picked or
//! banned for them. A suggestion weighs three things, each from -1 to 1 and 0 where it is not
//! known: how the champion fares against the enemies revealed so far in its lane's matchups, its
//! standing on the source's tier list, and how the local player has done with it in that lane.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    builds::Build,
    settings::{BuildSettings, Mode},
    view::{Position, RecommendReason, Recommendation},
};

/// Champions weighed at most.
pub const CANDIDATES: usize = 6;
/// The local player's own champions in the lane among them, the most played first.
pub const PLAYED: usize = 5;
/// Suggestions shown.
pub const SHOWN: usize = 3;

const MATCHUP_WEIGHT: f64 = 0.45;
const TIER_WEIGHT: f64 = 0.20;
const PLAYED_WEIGHT: f64 = 0.35;
/// A win rate this far from half is the most a matchup counts for (`matchup`).
const MATCHUP_SPAN: f64 = 0.05;

/// One champion the local player could take: from their pick list, or played in the lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub champion_id: i64,
    pub in_pick_list: bool,
}

/// The champions to weigh, in order: the pick list for the lane (its own list, then any lane),
/// then the ones the player played most in it (`played`, most first, [`PLAYED`] of them), each
/// once, those `free` refuses left out, [`CANDIDATES`] at most.
pub fn candidates(
    pick_list: &[i64],
    played: &[(i64, u32, u32)],
    free: impl Fn(i64) -> bool,
) -> Vec<Candidate> {
    let mut chosen: Vec<Candidate> = Vec::new();
    let listed = pick_list.iter().map(|&id| (id, true));
    let own = played.iter().take(PLAYED).map(|&(id, _, _)| (id, false));
    for (champion_id, in_pick_list) in listed.chain(own) {
        if chosen.len() == CANDIDATES {
            break;
        }
        if champion_id > 0
            && free(champion_id)
            && !chosen
                .iter()
                .any(|candidate| candidate.champion_id == champion_id)
        {
            chosen.push(Candidate {
                champion_id,
                in_pick_list,
            });
        }
    }
    chosen
}

/// How `build`'s champion fares against `enemies`: for each enemy among its lane's matchups, the
/// win rate's distance from half (the good list first where an enemy is on both), summed, then
/// over [`MATCHUP_SPAN`] and clamped. Enemies its matchups do not name count for nothing.
pub fn matchup(build: &Build, enemies: &[i64]) -> (f64, Vec<RecommendReason>) {
    let mut sum = 0.0;
    let mut reasons = Vec::new();
    for &enemy in enemies.iter().filter(|&&enemy| enemy > 0) {
        let found = build
            .matchups
            .good
            .iter()
            .chain(&build.matchups.bad)
            .find(|matchup| matchup.champion_id == enemy)
            .and_then(|matchup| matchup.rates.win);
        let Some(win) = found else {
            continue;
        };
        let difference = win - 0.5;
        sum += difference;
        if difference > 0.0 {
            reasons.push(RecommendReason::Counters {
                champion_id: enemy,
                win,
            });
        } else if difference < 0.0 {
            reasons.push(RecommendReason::CounteredBy {
                champion_id: enemy,
                win,
            });
        }
    }
    ((sum / MATCHUP_SPAN).clamp(-1.0, 1.0), reasons)
}

/// The source's standing for the champion, 1 the best of five (OP.GG), as -1 to 1; 0 where the
/// source has none (Tencent's numbers have no tiers).
pub fn tier(build: &Build) -> f64 {
    match build.tier {
        Some(tier @ 1..=5) => (3.0 - f64::from(tier)) / 2.0,
        _ => 0.0,
    }
}

/// How the player has done with a champion in the lane: `games` and `wins` there. Comfort grows
/// with the games up to ten; the win rate, pulled toward half by two wins and two losses, moves it
/// up or down. No games is 0.
pub fn proficiency(games: u32, wins: u32) -> f64 {
    let comfort = f64::from(games.min(10)) / 10.0;
    let rate = (f64::from(wins) + 2.0) / (f64::from(games) + 4.0);
    let form = ((rate - 0.5) / 0.25).clamp(-1.0, 1.0);
    0.5 * comfort + 0.5 * comfort * form
}

/// The suggestions among `candidates`, the best [`SHOWN`]: each one's score, `builds` holding what
/// the sources answered (`None`: asked, no numbers; absent: not answered yet), `played` the
/// player's games and wins in the lane by champion. Equal scores keep the candidates' order.
pub fn suggest(
    candidates: &[Candidate],
    builds: &HashMap<i64, Option<Arc<Build>>>,
    enemies: &[i64],
    played: &[(i64, u32, u32)],
) -> Vec<Recommendation> {
    let mut scored: Vec<(usize, Recommendation)> = candidates
        .iter()
        .enumerate()
        .map(|(order, candidate)| {
            let mut reasons = Vec::new();
            let (fit, tier_score) = match builds.get(&candidate.champion_id) {
                Some(Some(build)) => {
                    let (fit, mut matchups) = matchup(build, enemies);
                    reasons.append(&mut matchups);
                    if let Some(tier) = build.tier.filter(|tier| (1..=5).contains(tier)) {
                        reasons.push(RecommendReason::Tier { tier });
                    }
                    (fit, tier(build))
                }
                // Asked, and no source had numbers.
                Some(None) => {
                    reasons.push(RecommendReason::NoData);
                    (0.0, 0.0)
                }
                // Not answered yet.
                None => (0.0, 0.0),
            };
            let (games, wins) = played
                .iter()
                .find(|(id, _, _)| *id == candidate.champion_id)
                .map_or((0, 0), |&(_, games, wins)| (games, wins));
            if games > 0 {
                reasons.push(RecommendReason::Played { games, wins });
            }
            if candidate.in_pick_list {
                reasons.push(RecommendReason::InPickList);
            }
            let score = MATCHUP_WEIGHT * fit
                + TIER_WEIGHT * tier_score
                + PLAYED_WEIGHT * proficiency(games, wins);
            (
                order,
                Recommendation {
                    champion_id: candidate.champion_id,
                    score,
                    reasons,
                },
            )
        })
        .collect();
    scored.sort_by(|(a_order, a), (b_order, b)| {
        let difference = b.score - a.score;
        if difference.abs() < 1e-9 {
            a_order.cmp(b_order).then(a.champion_id.cmp(&b.champion_id))
        } else if difference > 0.0 {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        }
    });
    scored
        .into_iter()
        .take(SHOWN)
        .map(|(_, recommendation)| recommendation)
        .collect()
}

/// What one champ select's suggestions are asked for: its game, the kind of game and lane, and the
/// build settings that decide the source. Any of them changing starts the answers over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecommendKey {
    pub game_id: i64,
    pub mode: Mode,
    pub lane: Position,
    pub settings: BuildSettings,
}

/// The numbers asked for under one key, and the answers in: `None` where no source had any.
#[derive(Clone, Debug)]
pub struct RecommendState {
    pub key: RecommendKey,
    pub builds: HashMap<i64, Option<Arc<Build>>>,
    asked: HashSet<i64>,
}

impl RecommendState {
    /// The state for `key`: `kept` if it was for the same key, else a new one with nothing in.
    pub fn for_key(kept: Option<Self>, key: RecommendKey) -> Self {
        match kept {
            Some(state) if state.key == key => state,
            _ => Self {
                key,
                builds: HashMap::new(),
                asked: HashSet::new(),
            },
        }
    }

    /// The candidates not asked for yet, marked as asked now.
    pub fn to_ask(&mut self, candidates: &[Candidate]) -> Vec<i64> {
        candidates
            .iter()
            .map(|candidate| candidate.champion_id)
            .filter(|&champion| self.asked.insert(champion))
            .collect()
    }
}

/// Keeps the answer for `champion` asked under `key`, if `state` is still for that key; an answer
/// for a key since replaced (another lane, another source, another game) is dropped. Returns
/// whether it was kept.
pub fn answer(
    state: &mut Option<RecommendState>,
    key: &RecommendKey,
    champion: i64,
    answer: Option<Arc<Build>>,
) -> bool {
    match state {
        Some(state) if &state.key == key => {
            state.builds.insert(champion, answer);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builds::{BuildSource, Matchup, Matchups, Rates},
        settings::RiftSource,
    };

    fn key(lane: Position, rift_source: RiftSource) -> RecommendKey {
        RecommendKey {
            game_id: 9,
            mode: Mode::Ranked,
            lane,
            settings: BuildSettings {
                rift_source,
                ..BuildSettings::default()
            },
        }
    }

    /// A source switched or a lane changed mid champ select starts the answers over, and an
    /// answer asked under the old key that arrives late is dropped.
    #[test]
    fn answers_belong_to_the_key_they_were_asked_under() {
        let tencent = key(Position::Middle, RiftSource::Tencent);
        let mut state = Some(RecommendState::for_key(None, tencent.clone()));
        let asked = state
            .as_mut()
            .unwrap()
            .to_ask(&[candidate(1, true), candidate(2, false)]);
        assert_eq!(asked, [1, 2]);
        assert!(
            state
                .as_mut()
                .unwrap()
                .to_ask(&[candidate(1, true)])
                .is_empty(),
            "asked once"
        );
        let ahri = Some(Arc::new(build(1, None, &[], &[])));
        assert!(answer(&mut state, &tencent, 1, ahri.clone()));

        // The user switches the Rift source: the next draw starts over under the new key.
        let opgg = key(Position::Middle, RiftSource::OpGg);
        state = Some(RecommendState::for_key(state.take(), opgg.clone()));
        assert!(
            state.as_ref().unwrap().builds.is_empty(),
            "the old answers go"
        );
        assert_eq!(state.as_mut().unwrap().to_ask(&[candidate(1, true)]), [1]);
        // Tencent's answer for 2, asked before the switch, arrives now.
        assert!(!answer(&mut state, &tencent, 2, None));
        assert!(!state.as_ref().unwrap().builds.contains_key(&2));
        assert!(answer(&mut state, &opgg, 1, ahri));

        // A lane change is a new key too; the same key keeps what it has.
        let same = RecommendState::for_key(state.clone(), opgg.clone());
        assert_eq!(same.builds.len(), 1);
        let top = RecommendState::for_key(state, key(Position::Top, RiftSource::OpGg));
        assert!(top.builds.is_empty());
    }

    fn build(champion_id: i64, tier: Option<u8>, good: &[(i64, f64)], bad: &[(i64, f64)]) -> Build {
        let matchup = |&(champion_id, win): &(i64, f64)| Matchup {
            champion_id,
            rates: Rates {
                win: Some(win),
                ..Rates::default()
            },
        };
        let mut build = Build::new(BuildSource::OpGg, champion_id, Mode::Ranked);
        build.tier = tier;
        build.matchups = Matchups {
            good: good.iter().map(matchup).collect(),
            bad: bad.iter().map(matchup).collect(),
        };
        build
    }

    fn candidate(champion_id: i64, in_pick_list: bool) -> Candidate {
        Candidate {
            champion_id,
            in_pick_list,
        }
    }

    #[test]
    fn candidates_take_the_pick_list_then_the_lanes_own_champions_once_each() {
        let played = [
            (7, 9, 5),
            (1, 4, 2),
            (2, 3, 1),
            (3, 2, 1),
            (4, 1, 1),
            (5, 1, 0),
        ];
        assert_eq!(
            candidates(&[1, 8, 9], &played, |id| id != 9),
            [
                candidate(1, true),
                candidate(8, true),
                candidate(7, false),
                candidate(2, false),
                candidate(3, false),
                candidate(4, false),
            ],
            "the unavailable 9 left out, 1 once, six at most, the fifth played (5) past the cut"
        );
        assert!(candidates(&[], &[], |_| true).is_empty());
    }

    #[test]
    fn matchups_count_the_revealed_enemies_the_lane_knows() {
        let ahri = build(103, None, &[(238, 0.54)], &[(84, 0.46)]);
        let (fit, reasons) = matchup(&ahri, &[238, 0, 1]);
        assert!((fit - 0.8).abs() < 1e-9, "{fit}");
        assert_eq!(
            reasons,
            [RecommendReason::Counters {
                champion_id: 238,
                win: 0.54
            }]
        );
        let (fit, reasons) = matchup(&ahri, &[84]);
        assert!((fit + 0.8).abs() < 1e-9);
        assert!(matches!(
            reasons.as_slice(),
            [RecommendReason::CounteredBy {
                champion_id: 84,
                ..
            }]
        ));
        assert_eq!(matchup(&ahri, &[]).0, 0.0, "no enemy revealed");
        assert_eq!(matchup(&ahri, &[238, 238, 238]).0, 1.0, "clamped at one");
    }

    #[test]
    fn tier_and_proficiency_scale_into_minus_one_to_one() {
        assert_eq!(tier(&build(1, Some(1), &[], &[])), 1.0);
        assert_eq!(tier(&build(1, Some(3), &[], &[])), 0.0);
        assert_eq!(tier(&build(1, Some(5), &[], &[])), -1.0);
        assert_eq!(tier(&build(1, None, &[], &[])), 0.0, "Tencent has no tier");
        assert_eq!(proficiency(0, 0), 0.0);
        assert!((proficiency(10, 5) - 0.5).abs() < 1e-9);
        assert!(proficiency(10, 7) > proficiency(10, 5));
        assert!(proficiency(10, 3) < proficiency(10, 5));
        assert!(proficiency(10, 3) > 0.0, "comfort still counts");
        assert!((proficiency(30, 30) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_best_three_come_first_with_their_reasons_and_ties_keep_the_order() {
        let builds = HashMap::from([
            (1, Some(Arc::new(build(1, Some(1), &[(238, 0.55)], &[])))),
            (2, Some(Arc::new(build(2, Some(5), &[], &[(238, 0.44)])))),
            (3, None),
        ]);
        let played = [(2, 10, 8)];
        let shown = suggest(
            &[
                candidate(3, true),
                candidate(2, false),
                candidate(1, true),
                candidate(4, true),
            ],
            &builds,
            &[238],
            &played,
        );
        let ids: Vec<i64> = shown.iter().map(|shown| shown.champion_id).collect();
        assert_eq!(ids, [1, 3, 4], "3 and 4 tie at 0, in the candidates' order");
        assert_eq!(
            shown[0].reasons,
            [
                RecommendReason::Counters {
                    champion_id: 238,
                    win: 0.55
                },
                RecommendReason::Tier { tier: 1 },
                RecommendReason::InPickList
            ]
        );
        assert_eq!(
            shown[1].reasons,
            [RecommendReason::NoData, RecommendReason::InPickList]
        );
        assert_eq!(
            shown[2].reasons,
            [RecommendReason::InPickList],
            "not answered yet: no word about the numbers"
        );
        let all = suggest(&[candidate(2, false)], &builds, &[238], &played);
        assert!(
            all[0]
                .reasons
                .contains(&RecommendReason::Played { games: 10, wins: 8 })
        );
        assert!(
            all[0].score < 0.0,
            "countered and low on the tier list, despite the games"
        );
    }
}
