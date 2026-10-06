//! winer's own performance numbers: a 0–10 score for every line of a scoreboard, the MVP and SVP
//! it implies, and a form score that ranks teammates in champ select. Both formulas are written
//! out here and on the site's rating page (`docs/site/rating.md`). WeGame does not publish its
//! rating; the game score's weights were fitted so that its MVP and SVP fall where WeGame's do as
//! often as possible (`Weights`).

use crate::view::{Award, RecentForm};

/// What one player did in one game, as the score reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contribution {
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub damage: i64,
    /// Damage taken: what a frontline soaked for the team, as WeGame counts it (no mitigation).
    pub tanked: i64,
    pub gold: i64,
    /// Lane minions and jungle monsters.
    pub minions: i64,
    pub vision: i64,
    pub crowd_control: i64,
}

/// How much each part of a line counts in one kind of game. Every part but survival is the line's
/// value over the game's per-player average; survival is dying less than that average player.
///
/// Fitted on games WeGame scored (October 2026, `fixtures/wegame/`), so that the best line of each
/// side is the one WeGame names MVP or SVP. On those games winer's MVP and SVP are WeGame's in 85%
/// and 83% of 126 Summoner's Rift games and in 76% and 70% of 139 Hextech ARAM games; the previous
/// weights, one set for every mode, managed 68%/58% and 68%/56%. WeGame is said to compare each
/// player with others on the same champion, which no single game shows; that is most of what is
/// left. The two kinds of game want different weights: farming and vision only exist on a map
/// with lanes, and gold says more where nobody farms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub kills: f64,
    pub assists: f64,
    pub damage: f64,
    pub tanked: f64,
    pub gold: f64,
    pub minions: f64,
    pub vision: f64,
    pub crowd_control: f64,
    pub survival: f64,
}

impl Weights {
    /// Summoner's Rift and every other mode on a map with lanes.
    pub const RIFT: Self = Self {
        kills: 0.12,
        assists: 0.09,
        damage: 0.09,
        tanked: 0.05,
        gold: 0.36,
        minions: 0.06,
        vision: 0.04,
        crowd_control: 0.0,
        survival: 0.19,
    };
    /// ARAM and Hextech ARAM.
    pub const ARAM: Self = Self {
        kills: 0.12,
        assists: 0.12,
        damage: 0.12,
        tanked: 0.09,
        gold: 0.28,
        minions: 0.0,
        vision: 0.0,
        crowd_control: 0.02,
        survival: 0.25,
    };

    /// The weights for a game of `game_mode` (`CLASSIC`, `ARAM`, `KIWI`, …).
    pub fn of(game_mode: &str) -> &'static Self {
        match game_mode.to_ascii_uppercase().as_str() {
            "ARAM" | "KIWI" => &Self::ARAM,
            _ => &Self::RIFT,
        }
    }

    fn parts(&self) -> [f64; 8] {
        [
            self.kills,
            self.assists,
            self.damage,
            self.tanked,
            self.gold,
            self.minions,
            self.vision,
            self.crowd_control,
        ]
    }
}

/// No single part may count for more than three average players.
const CAP: f64 = 3.0;
/// A part whose game average is below this says nothing (vision in ARAM).
const MEANINGFUL: f64 = 1.0;
/// The logistic curve's steepness. Lines in one game sit close together (an ARAM team shares
/// most fights), so the curve is steep around the average and flat at both ends.
const STEEPNESS: f64 = 4.0;
/// `1 − ln(1.5)/k`: puts exactly one average player at 6.0.
const CENTRE: f64 = 1.0 - 0.405_465_108_108_164_4 / STEEPNESS;

fn parts(player: &Contribution) -> [f64; 8] {
    [
        player.kills,
        player.assists,
        player.damage,
        player.tanked,
        player.gold,
        player.minions,
        player.vision,
        player.crowd_control,
    ]
    .map(|value| value as f64)
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Each player's score in one game, in the order given, under `weights`.
///
/// Every part is the player's value over the game's per-player average, capped at [`CAP`]; the
/// weighted mean of the parts is how many "average players" the line was worth, and the logistic
/// `10 / (1 + e^(−k·(x − c)))` maps that onto 0–10: one average at 6.0, 1.25 at 8.0, 1.5 at 9.2,
/// 0.75 at 3.6 and half of one at 1.7. No line can score past 10.
pub fn game_scores(players: &[Contribution], weights: &Weights) -> Vec<f64> {
    if players.is_empty() {
        return Vec::new();
    }
    let count = players.len() as f64;
    let mut averages = [0.0; 8];
    for player in players {
        for (sum, value) in averages.iter_mut().zip(parts(player)) {
            *sum += value / count;
        }
    }
    let average_deaths = players
        .iter()
        .map(|player| player.deaths as f64)
        .sum::<f64>()
        / count;

    players
        .iter()
        .map(|player| {
            let (mut total, mut weight) = (0.0, 0.0);
            for ((value, average), part_weight) in
                parts(player).into_iter().zip(averages).zip(weights.parts())
            {
                if part_weight > 0.0 && average >= MEANINGFUL {
                    total += part_weight * (value / average).min(CAP);
                    weight += part_weight;
                }
            }
            total +=
                weights.survival * ((average_deaths + 1.0) / (player.deaths as f64 + 1.0)).min(CAP);
            weight += weights.survival;
            round1(10.0 / (1.0 + (-STEEPNESS * (total / weight - CENTRE)).exp()))
        })
        .collect()
}

/// The best score of the winning side is the MVP and the best of the losing side the SVP. Needs
/// exactly one winning and one losing side; a draw of scores goes to the earlier line.
pub fn awards(scores: &[f64], won: &[bool]) -> Vec<Option<Award>> {
    let best = |side: bool| {
        (0..scores.len())
            .filter(|&index| won.get(index) == Some(&side))
            .fold(None, |best: Option<usize>, index| match best {
                Some(current) if scores[current] >= scores[index] => Some(current),
                _ => Some(index),
            })
    };
    let mut awards = vec![None; scores.len()];
    if let (Some(mvp), Some(svp)) = (best(true), best(false)) {
        awards[mvp] = Some(Award::Mvp);
        awards[svp] = Some(Award::Svp);
    }
    awards
}

/// 峡谷评级's lower bounds on a form score, S+ to E; anything lower is F. A middling player (half
/// the games won, KDA 3, twenty games) scores about 5.5, a B; half won at KDA 4.3 is 6.0, an A.
pub const FORM_GRADES: [f64; 7] = [7.6, 6.8, 5.9, 5.3, 4.8, 4.3, 3.8];
/// The same for one game's score, where the game's average player stands at 6.0, a B.
pub const GAME_GRADES: [f64; 7] = [9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0];
/// The grades' letters, best first.
pub const GRADE_LETTERS: [&str; 8] = ["S+", "S", "A", "B", "C", "D", "E", "F"];

/// The grade, 0 (S+) to 7 (F), of `score` on `bands`.
pub fn grade(score: f64, bands: &[f64; 7]) -> u8 {
    bands
        .iter()
        .position(|&floor| score >= floor)
        .unwrap_or(bands.len()) as u8
}

/// What recent games say about a player beyond the grade, the most telling first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormTitle {
    /// Three or more wins in a row: 版本答案.
    OnAStreak,
    /// Three or more losses in a row: 排位慈善家.
    GivingAway,
    /// Hardly a kill and many deaths a game: 电竞菩萨.
    Bodhisattva,
    /// KDA of 6 or better: 峡谷永生者.
    Immortal,
    /// Many kills and many deaths a game: 一换一专业户.
    Trader,
    /// Eight or more deaths a game: 黑白电视机资深会员.
    GreyScreen,
    /// Twice as many assists as kills, and plenty of them: 峡谷慈善家.
    Helper,
}

/// The title recent form earns, if any, from the streak first and then the averages. Needs five
/// games: fewer say too little.
pub fn form_title(form: &RecentForm) -> Option<FormTitle> {
    if form.games < 5 {
        return None;
    }
    let kda = (form.kills + form.assists) / form.deaths.max(1.0);
    Some(if form.streak >= 3 {
        FormTitle::OnAStreak
    } else if form.streak <= -3 {
        FormTitle::GivingAway
    } else if form.kills <= 2.0 && form.deaths >= 7.0 {
        FormTitle::Bodhisattva
    } else if kda >= 6.0 {
        FormTitle::Immortal
    } else if form.kills >= 8.0 && form.deaths >= 7.0 {
        FormTitle::Trader
    } else if form.deaths >= 8.0 {
        FormTitle::GreyScreen
    } else if form.assists >= 12.0 && form.assists >= 2.0 * form.kills {
        FormTitle::Helper
    } else {
        return None;
    })
}

/// How many games of evidence weigh as much as the neutral prior in [`form_score`].
const FORM_PRIOR_GAMES: f64 = 5.0;

/// Recent form, 0–10: half win rate, half KDA on `1 − e^(−kda/3)` (3.0 → 0.63, 6.0 → 0.86), then
/// pulled toward a neutral 5.0 by `games / (games + 5)`, so one lucky game cannot outrank twenty
/// good ones. `None` without games.
pub fn form_score(form: &RecentForm) -> Option<f64> {
    if form.games == 0 {
        return None;
    }
    let games = f64::from(form.games);
    let win_rate = f64::from(form.wins) / games;
    let kda = (form.kills + form.assists) / form.deaths.max(1.0);
    let raw = 10.0 * (0.5 * win_rate + 0.5 * (1.0 - (-kda / 3.0).exp()));
    let confidence = games / (games + FORM_PRIOR_GAMES);
    Some(round1(confidence * raw + (1.0 - confidence) * 5.0))
}

/// Splits players into `count` tiers by score, best first (tier 0). Each player stands at the
/// middle of their slice of the ranking, `(place + ½) / n`, and takes the tier that point falls in
/// (a point on a boundary goes to the better tier): five players in three tiers split 2 / 1 / 2,
/// four 1 / 2 / 1, two land top and bottom and one alone in the middle; five in five tiers get one
/// each, four in five skip the middle. Unscored players get no tier; ties keep seat order.
pub fn tiers(scores: &[Option<f64>], count: usize) -> Vec<Option<u8>> {
    let mut ranked: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .filter_map(|(index, score)| score.map(|score| (index, score)))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let (players, count) = (ranked.len() as f64, count.max(1));
    let mut tiers = vec![None; scores.len()];
    for (place, (index, _)) in ranked.iter().enumerate() {
        let point = (place as f64 + 0.5) / players * count as f64;
        tiers[*index] = Some(((point - 1e-9).floor().max(0.0) as usize).min(count - 1) as u8);
    }
    tiers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recent(
        games: u32,
        wins: u32,
        kills: f64,
        deaths: f64,
        assists: f64,
        streak: i32,
    ) -> RecentForm {
        RecentForm {
            games,
            wins,
            kills,
            deaths,
            assists,
            streak,
            ..RecentForm::default()
        }
    }

    #[test]
    fn grades_follow_fixed_bands_from_s_plus_to_f() {
        assert_eq!(grade(7.6, &FORM_GRADES), 0, "S+ starts at 7.6");
        assert_eq!(grade(7.59, &FORM_GRADES), 1);
        assert_eq!(grade(3.79, &FORM_GRADES), 7, "below every band is F");
        assert_eq!(
            grade(6.0, &GAME_GRADES),
            3,
            "the game's average player is a B"
        );
        let middling = form_score(&recent(20, 10, 5.0, 5.0, 10.0, 0)).unwrap();
        assert_eq!(
            (
                middling,
                GRADE_LETTERS[usize::from(grade(middling, &FORM_GRADES))]
            ),
            (5.5, "B")
        );
        let solid = form_score(&recent(18, 9, 9.3, 9.2, 30.7, 0)).unwrap();
        assert_eq!(
            GRADE_LETTERS[usize::from(grade(solid, &FORM_GRADES))],
            "A",
            "half won at KDA 4.3 is a civil servant"
        );
    }

    #[test]
    fn form_titles_name_the_streak_first_then_the_averages() {
        let title = |kills, deaths, assists, streak| {
            form_title(&recent(20, 10, kills, deaths, assists, streak))
        };
        assert_eq!(title(5.0, 5.0, 5.0, 4), Some(FormTitle::OnAStreak));
        assert_eq!(title(5.0, 5.0, 5.0, -3), Some(FormTitle::GivingAway));
        assert_eq!(title(1.5, 8.0, 6.0, 0), Some(FormTitle::Bodhisattva));
        assert_eq!(title(9.0, 2.0, 6.0, 0), Some(FormTitle::Immortal));
        assert_eq!(title(9.0, 8.0, 6.0, 0), Some(FormTitle::Trader));
        assert_eq!(title(4.0, 9.0, 8.0, 0), Some(FormTitle::GreyScreen));
        assert_eq!(title(4.0, 5.0, 14.0, 0), Some(FormTitle::Helper));
        assert_eq!(title(5.0, 5.0, 5.0, 0), None, "nothing stands out");
        assert_eq!(
            form_title(&recent(4, 4, 9.0, 1.0, 9.0, 4)),
            None,
            "four games say too little"
        );
    }

    fn player(kills: i64, deaths: i64, assists: i64, damage: i64) -> Contribution {
        Contribution {
            kills,
            deaths,
            assists,
            damage,
            tanked: 20_000,
            gold: 10_000,
            minions: 0,
            vision: 0,
            crowd_control: 20,
        }
    }

    #[test]
    fn an_average_line_scores_six_and_a_stronger_one_more() {
        let same = vec![player(5, 5, 5, 20_000); 10];
        assert!(
            game_scores(&same, &Weights::RIFT)
                .iter()
                .all(|&score| score == 6.0)
        );

        let mut game = same.clone();
        game[0] = player(15, 2, 10, 45_000);
        game[1] = player(0, 12, 2, 5_000);
        let scores = game_scores(&game, &Weights::RIFT);
        assert!(scores[0] > 8.0 && scores[0] <= 10.0, "{scores:?}");
        assert!(scores[1] < 4.0, "{scores:?}");
        assert!(scores.iter().all(|score| (0.0..=10.0).contains(score)));
    }

    #[test]
    fn a_part_nobody_scored_in_is_left_out() {
        // Vision is zero for everyone (ARAM): it neither helps nor drags anyone.
        let game = vec![player(5, 5, 5, 20_000); 2];
        assert_eq!(game_scores(&game, &Weights::RIFT), vec![6.0, 6.0]);
        assert_eq!(game_scores(&[], &Weights::RIFT), Vec::<f64>::new());
    }

    #[test]
    fn one_number_stops_counting_at_three_averages() {
        let outlier = |damage: i64| {
            let mut game = vec![player(1, 5, 1, 10_000); 5];
            game[0].damage = damage;
            game_scores(&game, &Weights::RIFT)[0]
        };
        assert_eq!(
            outlier(10_000_000),
            outlier(100_000_000),
            "past the cap more damage adds nothing"
        );
        assert!(outlier(10_000_000) < 10.0);
    }

    /// How often winer's MVP and SVP are WeGame's, over the games in `fixtures/wegame/`.
    fn agreement_with_wegame(games: &[Vec<[i64; 11]>], weights: &Weights) -> (f64, f64) {
        let (mut mvp, mut svp) = (0, 0);
        for game in games {
            let players: Vec<Contribution> = game
                .iter()
                .map(|row| Contribution {
                    kills: row[1],
                    deaths: row[2],
                    assists: row[3],
                    damage: row[4],
                    tanked: row[5],
                    gold: row[6],
                    crowd_control: row[7],
                    minions: row[8],
                    vision: row[9],
                })
                .collect();
            let won: Vec<bool> = game.iter().map(|row| row[0] == 1).collect();
            let scores = game_scores(&players, weights);
            for (row, award) in game.iter().zip(awards(&scores, &won)) {
                match award {
                    Some(Award::Mvp) => mvp += i32::from(row[10] == 1),
                    Some(Award::Svp) => svp += i32::from(row[10] == 2),
                    None => {}
                }
            }
        }
        let games = games.len() as f64;
        (f64::from(mvp) / games, f64::from(svp) / games)
    }

    #[test]
    fn mvp_and_svp_mostly_land_where_wegame_puts_them() {
        #[derive(serde::Deserialize)]
        struct Calibration {
            rift: Vec<Vec<[i64; 11]>>,
            aram: Vec<Vec<[i64; 11]>>,
        }
        let games: Calibration = crate::test_support::fixture("wegame/calibration.json");
        assert_eq!((games.rift.len(), games.aram.len()), (126, 139));
        let rift = agreement_with_wegame(&games.rift, Weights::of("CLASSIC"));
        let aram = agreement_with_wegame(&games.aram, Weights::of("KIWI"));
        assert!(
            rift.0 >= 0.84 && rift.1 >= 0.83,
            "Summoner's Rift: {rift:?}"
        );
        assert!(aram.0 >= 0.76 && aram.1 >= 0.70, "Hextech ARAM: {aram:?}");
        // One set of weights for both would give up most of it on one side or the other.
        let swapped = agreement_with_wegame(&games.aram, &Weights::RIFT);
        assert!(swapped.1 < aram.1, "{swapped:?}");
    }

    #[test]
    fn mvp_and_svp_go_to_the_best_of_each_side() {
        let scores = [7.0, 9.1, 5.0, 8.2, 8.2];
        let won = [true, true, true, false, false];
        assert_eq!(
            awards(&scores, &won),
            vec![None, Some(Award::Mvp), None, Some(Award::Svp), None]
        );
        assert_eq!(
            awards(&[6.0, 6.0], &[true, true]),
            vec![None, None],
            "no losing side, no awards"
        );
    }

    fn form(games: u32, wins: u32, kills: f64, deaths: f64, assists: f64) -> RecentForm {
        RecentForm {
            games,
            wins,
            kills,
            deaths,
            assists,
            ..RecentForm::default()
        }
    }

    #[test]
    fn form_rewards_winning_and_kda_and_needs_games() {
        assert_eq!(form_score(&form(0, 0, 0.0, 0.0, 0.0)), None);
        let strong = form_score(&form(20, 14, 9.0, 3.0, 9.0)).unwrap();
        let weak = form_score(&form(20, 6, 3.0, 8.0, 4.0)).unwrap();
        assert!(strong > 7.0 && weak < 4.5, "{strong} {weak}");
        // One lucky game is not a 100% player.
        assert!(form_score(&form(1, 1, 5.0, 1.0, 5.0)).unwrap() < strong);
    }

    #[test]
    fn tiers_split_the_ranking_evenly_and_skip_the_unscored() {
        let five = [Some(5.0), Some(8.0), Some(3.0), Some(6.0), Some(7.0)];
        assert_eq!(
            tiers(&five, 3),
            vec![Some(2), Some(0), Some(2), Some(1), Some(0)],
            "2 / 1 / 2"
        );
        assert_eq!(
            tiers(&five, 5),
            vec![Some(3), Some(0), Some(4), Some(2), Some(1)],
            "one each"
        );
        let four = [Some(4.0), Some(3.0), Some(2.0), Some(1.0)];
        assert_eq!(
            tiers(&four, 3),
            vec![Some(0), Some(1), Some(1), Some(2)],
            "1 / 2 / 1"
        );
        assert_eq!(
            tiers(&four, 5),
            vec![Some(0), Some(1), Some(3), Some(4)],
            "the middle is skipped"
        );
        assert_eq!(
            tiers(&[Some(1.0), None, Some(2.0)], 3),
            vec![Some(2), None, Some(0)]
        );
        assert_eq!(tiers(&[Some(4.0)], 3), vec![Some(1)], "alone in the middle");
        assert_eq!(
            tiers(&[Some(4.0)], 2),
            vec![Some(0)],
            "a boundary goes to the better tier"
        );
        assert_eq!(
            tiers(&[Some(5.0), Some(5.0)], 3),
            vec![Some(0), Some(2)],
            "ties keep seat order"
        );
    }
}
