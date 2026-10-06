//! winer's own performance numbers: a 0–10 score for every line of a scoreboard, the MVP and SVP
//! it implies, and a form score that ranks teammates in champ select. Both formulas are written
//! out here and on the site's rating page (`docs/site/rating.md`). WeGame does not publish its
//! rating; the game score's weights were fitted so that its MVP and SVP fall where WeGame's do as
//! often as possible (`Weights`).

use std::{cmp::Ordering, collections::HashMap};

use crate::view::{Award, RecentForm};

/// What a champion is for, as the client's champion list names it first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Tank,
    Support,
    Mage,
    Assassin,
    Marksman,
    Fighter,
}

impl Role {
    /// The client's own word (`tank`, `marksman`, …).
    pub fn parse(role: &str) -> Option<Self> {
        match role.to_ascii_lowercase().as_str() {
            "tank" => Some(Self::Tank),
            "support" => Some(Self::Support),
            "mage" => Some(Self::Mage),
            "assassin" => Some(Self::Assassin),
            "marksman" => Some(Self::Marksman),
            "fighter" => Some(Self::Fighter),
            _ => None,
        }
    }
}

/// Each champion's first role, by champion id, from the client's champion list.
pub type Roles = HashMap<i64, Role>;

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
    /// The champion's role, where the client's champion list names one.
    pub role: Option<Role>,
}

/// How much each part of a line counts. Every part but survival is the line's value over the
/// game's per-player average; survival is dying less than that average player.
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

/// How one kind of game is scored: a set of weights for any line, and where the champion's role
/// changes what an ordinary line looks like, a set per role.
///
/// Fitted on games WeGame scored (October 2026, `fixtures/wegame/`), so that the best line of each
/// side is the one WeGame names MVP or SVP. On those games winer's MVP and SVP are WeGame's in 85%
/// and 83% of 126 Summoner's Rift games and in 80% and 73% of 139 Hextech ARAM games; the previous
/// weights, one set for every mode, managed 68%/58% and 68%/56%. The two kinds of game want
/// different weights: farming and vision only exist on a map with lanes, and gold says more where
/// nobody farms. WeGame is said to compare each player with others on the same champion; in ARAM,
/// where the champion is random, weighing each role against its own ordinary line (a tank's kills
/// count more, its damage taken less) gained most of what that could be measured to give. On the
/// Rift the role added nothing that would show.
#[derive(Debug, PartialEq)]
pub struct Scoring {
    /// For a champion without a known role, and the yardstick every line is divided by, so that an
    /// ordinary line of any role still scores about 6.0.
    pub base: Weights,
    pub roles: &'static [(Role, Weights)],
}

/// One role's ARAM weights, in the order kills, assists, damage, damage taken, gold, crowd control
/// and survival.
const fn aram(
    kills: f64,
    assists: f64,
    damage: f64,
    tanked: f64,
    gold: f64,
    crowd_control: f64,
    survival: f64,
) -> Weights {
    Weights {
        kills,
        assists,
        damage,
        tanked,
        gold,
        minions: 0.0,
        vision: 0.0,
        crowd_control,
        survival,
    }
}

const ARAM_ROLES: [(Role, Weights); 6] = [
    (Role::Tank, aram(0.12, 0.09, 0.11, 0.06, 0.35, 0.0, 0.21)),
    (Role::Support, aram(0.14, 0.09, 0.13, 0.10, 0.35, 0.0, 0.20)),
    (Role::Mage, aram(0.10, 0.09, 0.09, 0.10, 0.34, 0.0, 0.21)),
    (
        Role::Assassin,
        aram(0.07, 0.08, 0.09, 0.09, 0.34, 0.02, 0.22),
    ),
    (
        Role::Marksman,
        aram(0.07, 0.08, 0.09, 0.11, 0.33, 0.02, 0.22),
    ),
    (
        Role::Fighter,
        aram(0.08, 0.08, 0.10, 0.08, 0.34, 0.01, 0.22),
    ),
];

impl Scoring {
    /// Summoner's Rift and every other mode on a map with lanes.
    pub const RIFT: Self = Self {
        base: Weights {
            kills: 0.12,
            assists: 0.09,
            damage: 0.09,
            tanked: 0.05,
            gold: 0.36,
            minions: 0.06,
            vision: 0.04,
            crowd_control: 0.0,
            survival: 0.19,
        },
        roles: &[],
    };
    /// ARAM and Hextech ARAM.
    pub const ARAM: Self = Self {
        base: aram(0.09, 0.09, 0.10, 0.09, 0.34, 0.01, 0.22),
        roles: &ARAM_ROLES,
    };

    /// How a game of `game_mode` (`CLASSIC`, `ARAM`, `KIWI`, …) is scored.
    pub fn of(game_mode: &str) -> &'static Self {
        match game_mode.to_ascii_uppercase().as_str() {
            "ARAM" | "KIWI" => &Self::ARAM,
            _ => &Self::RIFT,
        }
    }

    fn weights(&self, role: Option<Role>) -> &Weights {
        role.and_then(|role| self.roles.iter().find(|(known, _)| *known == role))
            .map_or(&self.base, |(_, weights)| weights)
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

/// Each player's score in one game, in the order given, under `scoring`.
///
/// Every part is the player's value over the game's per-player average, capped at [`CAP`]; the
/// weighted sum of the parts, with the weights of the player's role, over the sum of the base
/// weights is how many "average players" the line was worth, and the logistic
/// `10 / (1 + e^(−k·(x − c)))` maps that onto 0–10: one average at 6.0, 1.25 at 8.0, 1.5 at 9.2,
/// 0.75 at 3.6 and half of one at 1.7. No line can score past 10.
pub fn game_scores(players: &[Contribution], scoring: &Scoring) -> Vec<f64> {
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
    let base = &scoring.base;

    players
        .iter()
        .map(|player| {
            let own = scoring.weights(player.role);
            let (mut total, mut weight) = (0.0, 0.0);
            for (((value, average), part_weight), base_weight) in parts(player)
                .into_iter()
                .zip(averages)
                .zip(own.parts())
                .zip(base.parts())
            {
                if base_weight > 0.0 && average >= MEANINGFUL {
                    total += part_weight * (value / average).min(CAP);
                    weight += base_weight;
                }
            }
            total +=
                own.survival * ((average_deaths + 1.0) / (player.deaths as f64 + 1.0)).min(CAP);
            weight += base.survival;
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

/// Where a tier stands against the middle of its scheme: a title never says the opposite of the
/// tier beside it, and the in-game callout names the enemies either side of it. A ranking splits
/// around its middle tier (of five, the first two are above and the last two below; of two,
/// neither is the middle); of the eight grades, B and C, the bands either side of an ordinary
/// player's form, are the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lean {
    Above,
    Middle,
    Below,
}

/// The [`Lean`] of `tier` of `tiers`, or of `grade` (0 S+ to 7 F) where the scheme grades.
pub fn lean(tier: u8, tiers: u8, grade: Option<u8>) -> Lean {
    let order = match grade {
        Some(grade) if grade <= 2 => Ordering::Less,
        Some(grade) if grade >= 5 => Ordering::Greater,
        Some(_) => Ordering::Equal,
        None => (2 * u16::from(tier) + 1).cmp(&u16::from(tiers)),
    };
    match order {
        Ordering::Less => Lean::Above,
        Ordering::Equal => Lean::Middle,
        Ordering::Greater => Lean::Below,
    }
}

/// The average player's kills, deaths and assists a game in `game_mode`, measured on the games
/// WeGame scored (`fixtures/wegame/calibration.json`: 1,260 Rift lines, 1,390 ARAM lines). An
/// ARAM game holds about twice the Rift's kills and deaths and over three times its assists, so a
/// count says little until it is set against its mode. Other modes (Arena, URF, …) have none.
pub fn average_line(game_mode: &str) -> Option<[f64; 3]> {
    match game_mode {
        "CLASSIC" => Some([5.1, 5.2, 7.5]),
        "ARAM" | "KIWI" => Some([11.1, 11.1, 25.6]),
        _ => None,
    }
}

/// A player's kills, deaths and assists against the average player's of each game's mode, where
/// 1.0 is that average: over the `games` whose mode has one ([`average_line`]), totals against
/// totals.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pace {
    pub games: u32,
    pub kills: f64,
    pub deaths: f64,
    pub assists: f64,
}

impl Pace {
    /// From each game's kills, deaths and assists and its mode's average line; `None` without a
    /// game.
    pub fn of(lines: impl IntoIterator<Item = ([i64; 3], [f64; 3])>) -> Option<Self> {
        let (mut games, mut own, mut usual) = (0u32, [0.0; 3], [0.0; 3]);
        for (line, average) in lines {
            games += 1;
            for stat in 0..3 {
                own[stat] += line[stat] as f64;
                usual[stat] += average[stat];
            }
        }
        (games > 0).then(|| Self {
            games,
            kills: own[0] / usual[0],
            deaths: own[1] / usual[1],
            assists: own[2] / usual[2],
        })
    }
}

/// What recent games say about a player beyond the tier: one title, of the tier's own leaning
/// ([`form_title`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormTitle {
    // ---- Above the middle ----
    /// Three or more wins in a row: 版本答案.
    OnAStreak,
    /// Dies at most 0.65 times as often as the mode's average player: 峡谷永生者.
    Immortal,
    /// Kills at least 1.35 times the average: 人头收割机.
    Reaper,
    /// Assists at least 1.3 times the average: 团战发动机.
    Playmaker,
    /// Two games in three won, over eight games or more: 常胜将军.
    Winner,
    /// Above the middle with nothing else standing out: 靠谱队友.
    Reliable,
    // ---- Any tier ----
    /// Kills and deaths both at least 1.2 times the average: 一换一专业户.
    Trader,
    /// Assists at least 1.2 times the average, kills at most 0.85: 峡谷慈善家.
    Helper,
    /// At the middle with nothing standing out: 正常发挥.
    Steady,
    // ---- Below the middle ----
    /// Three or more losses in a row: 排位慈善家.
    GivingAway,
    /// Kills at most 0.55 times the average and deaths at least 1.1: 电竞菩萨.
    Bodhisattva,
    /// Deaths at least 1.3 times the average: 黑白电视机资深会员.
    GreyScreen,
    /// Kills and assists both at most 0.65 times the average: 团战观众.
    Spectator,
    /// A third of the games won or fewer, over eight games or more: 峡谷观光客.
    Tourist,
    /// Below the middle with nothing else standing out: 陪跑选手.
    AlongForTheRide,
}

/// The title recent form earns beside a tier that leans `lean`: the first that fits of the
/// leaning's own, then of those any tier may have, else the leaning's plain one. Kills, deaths and
/// assists are read against each game's mode ([`Pace`]), so an ARAM player's ten deaths are an
/// ordinary game, and a tier above the middle never gets a title below it, nor the other way
/// round. Needs five games, and five with a mode's average for what the counts say.
pub fn form_title(form: &RecentForm, lean: Lean) -> Option<FormTitle> {
    use FormTitle::*;
    if form.games < 5 {
        return None;
    }
    let pace = form.pace.filter(|pace| pace.games >= 5);
    let paced = |test: fn(&Pace) -> bool| pace.as_ref().is_some_and(test);
    let won = f64::from(form.wins) / f64::from(form.games);
    let many = form.games >= 8;
    let own: Vec<(bool, FormTitle)> = match lean {
        Lean::Above => vec![
            (form.streak >= 3, OnAStreak),
            (paced(|pace| pace.deaths <= 0.65), Immortal),
            (paced(|pace| pace.kills >= 1.35), Reaper),
            (paced(|pace| pace.assists >= 1.3), Playmaker),
            (many && won >= 2.0 / 3.0, Winner),
        ],
        Lean::Middle => vec![
            (form.streak >= 3, OnAStreak),
            (form.streak <= -3, GivingAway),
        ],
        Lean::Below => vec![
            (form.streak <= -3, GivingAway),
            (
                paced(|pace| pace.kills <= 0.55 && pace.deaths >= 1.1),
                Bodhisattva,
            ),
            (paced(|pace| pace.deaths >= 1.3), GreyScreen),
            (
                paced(|pace| pace.kills <= 0.65 && pace.assists <= 0.65),
                Spectator,
            ),
            (many && won <= 1.0 / 3.0, Tourist),
        ],
    };
    let any = [
        (
            paced(|pace| pace.kills >= 1.2 && pace.deaths >= 1.2),
            Trader,
        ),
        (
            paced(|pace| pace.assists >= 1.2 && pace.kills <= 0.85),
            Helper,
        ),
    ];
    let plain = match lean {
        Lean::Above => Reliable,
        Lean::Middle => Steady,
        Lean::Below => AlongForTheRide,
    };
    Some(
        own.into_iter()
            .chain(any)
            .find_map(|(fits, title)| fits.then_some(title))
            .unwrap_or(plain),
    )
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

/// The tier of `count`, best first, that the fixed grade `grade` (0 S+ to 7 F, [`FORM_GRADES`])
/// falls in when the eight grades are spread over the tiers the way [`tiers`] spreads a team: each
/// grade stands at the middle of its slice, `(grade + ½) / 8`. Five tiers take S+ and S, A, B and C,
/// D, E and F; eight are the grades themselves. For one player on their own, where there is no team
/// to rank against.
pub fn tier_of_grade(grade: u8, count: usize) -> u8 {
    let grades = GRADE_LETTERS.len() as f64;
    let count = count.max(1);
    let point = (f64::from(grade.min(7)) + 0.5) / grades * count as f64;
    ((point - 1e-9).floor().max(0.0) as usize).min(count - 1) as u8
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

    /// Twenty games, ten won, at `pace` against the mode's average, `streak` the run at the top.
    fn paced(kills: f64, deaths: f64, assists: f64, streak: i32) -> RecentForm {
        RecentForm {
            pace: Some(Pace {
                games: 20,
                kills,
                deaths,
                assists,
            }),
            ..recent(20, 10, 5.0, 5.0, 5.0, streak)
        }
    }

    #[test]
    fn a_tier_leans_above_at_or_below_the_middle_of_its_scheme() {
        let five: Vec<Lean> = (0..5).map(|tier| lean(tier, 5, None)).collect();
        assert_eq!(
            five,
            [
                Lean::Above,
                Lean::Above,
                Lean::Middle,
                Lean::Below,
                Lean::Below
            ]
        );
        assert_eq!(
            (lean(0, 2, None), lean(1, 2, None)),
            (Lean::Above, Lean::Below),
            "two tiers have no middle"
        );
        let grades: Vec<Lean> = (0..8).map(|grade| lean(grade, 8, Some(grade))).collect();
        assert_eq!(
            grades,
            [
                Lean::Above,
                Lean::Above,
                Lean::Above,
                Lean::Middle,
                Lean::Middle,
                Lean::Below,
                Lean::Below,
                Lean::Below
            ],
            "S+, S, A above; B, C the middle"
        );
    }

    #[test]
    fn counts_are_read_against_each_games_mode() {
        let rift = average_line("CLASSIC").unwrap();
        let aram = average_line("KIWI").unwrap();
        assert_eq!(average_line("ARAM"), Some(aram));
        assert_eq!(average_line("CHERRY"), None, "Arena has no average here");
        // Ten deaths a game: twice the Rift's average, under ARAM's.
        let ten = Pace::of([([5, 10, 10], rift), ([5, 10, 10], rift)]).unwrap();
        assert!((ten.deaths - 10.0 / 5.2).abs() < 1e-9, "{ten:?}");
        let in_aram = Pace::of([([11, 10, 26], aram)]).unwrap();
        assert!(in_aram.deaths < 1.0 && in_aram.kills < 1.0 && in_aram.assists > 1.0);
        // Games of two modes: totals against totals.
        let both = Pace::of([([0, 0, 0], rift), ([0, 21, 0], aram)]).unwrap();
        assert_eq!(both.games, 2);
        assert!((both.deaths - 21.0 / (5.2 + 11.1)).abs() < 1e-9);
        assert_eq!(Pace::of([]), None);
    }

    #[test]
    fn a_title_never_says_the_opposite_of_its_tier() {
        // Dies half as often again as the mode's average, nothing else out of the ordinary.
        let dying = paced(1.0, 1.5, 1.0, 0);
        assert_eq!(form_title(&dying, Lean::Below), Some(FormTitle::GreyScreen));
        assert_eq!(
            form_title(&dying, Lean::Above),
            Some(FormTitle::Reliable),
            "a good tier is never a grey screen"
        );
        assert_eq!(form_title(&dying, Lean::Middle), Some(FormTitle::Steady));
        // Three wins in a row: a streak above or at the middle, never below.
        let winning = paced(1.0, 1.0, 1.0, 3);
        assert_eq!(
            form_title(&winning, Lean::Above),
            Some(FormTitle::OnAStreak)
        );
        assert_eq!(
            form_title(&winning, Lean::Middle),
            Some(FormTitle::OnAStreak)
        );
        assert_eq!(
            form_title(&winning, Lean::Below),
            Some(FormTitle::AlongForTheRide)
        );
        let losing = paced(1.0, 1.0, 1.0, -3);
        assert_eq!(form_title(&losing, Lean::Above), Some(FormTitle::Reliable));
        assert_eq!(
            form_title(&losing, Lean::Below),
            Some(FormTitle::GivingAway)
        );
    }

    #[test]
    fn each_leaning_has_its_titles_the_most_telling_first() {
        use FormTitle::*;
        let above =
            |kills, deaths, assists| form_title(&paced(kills, deaths, assists, 0), Lean::Above);
        assert_eq!(above(1.0, 0.6, 1.0), Some(Immortal));
        assert_eq!(
            above(1.4, 0.6, 1.0),
            Some(Immortal),
            "rarely dying comes first"
        );
        assert_eq!(above(1.4, 1.0, 1.0), Some(Reaper));
        assert_eq!(above(1.0, 1.0, 1.3), Some(Playmaker));
        assert_eq!(
            above(1.25, 1.25, 1.0),
            Some(Trader),
            "then what any tier may be"
        );
        assert_eq!(above(0.8, 1.0, 1.25), Some(Helper));
        assert_eq!(above(1.0, 1.0, 1.0), Some(Reliable));
        let winner = RecentForm {
            wins: 14,
            ..paced(1.0, 1.0, 1.0, 0)
        };
        assert_eq!(form_title(&winner, Lean::Above), Some(Winner));

        let below =
            |kills, deaths, assists| form_title(&paced(kills, deaths, assists, 0), Lean::Below);
        assert_eq!(below(0.5, 1.2, 1.0), Some(Bodhisattva));
        assert_eq!(below(0.8, 1.3, 1.0), Some(GreyScreen));
        assert_eq!(below(0.6, 1.0, 0.6), Some(Spectator));
        assert_eq!(below(1.25, 1.25, 1.0), Some(Trader));
        assert_eq!(below(1.0, 1.0, 1.0), Some(AlongForTheRide));
        let tourist = RecentForm {
            wins: 6,
            ..paced(1.0, 1.0, 1.0, 0)
        };
        assert_eq!(form_title(&tourist, Lean::Below), Some(Tourist));

        let middle =
            |kills, deaths, assists| form_title(&paced(kills, deaths, assists, 0), Lean::Middle);
        assert_eq!(middle(1.25, 1.25, 1.0), Some(Trader));
        assert_eq!(middle(0.8, 1.0, 1.25), Some(Helper));
        assert_eq!(
            middle(1.4, 0.5, 1.4),
            Some(Steady),
            "no praise at the middle"
        );
        assert_eq!(middle(0.5, 1.5, 0.5), Some(Steady), "nor a roast");
    }

    #[test]
    fn titles_need_five_games_and_counts_need_five_with_a_mode() {
        assert_eq!(
            form_title(&recent(4, 4, 9.0, 1.0, 9.0, 4), Lean::Above),
            None,
            "four games say too little"
        );
        // Eight games, mostly Arena: three with a mode's average are too few to read counts from,
        // so only the streak and the win rate speak.
        let arena = RecentForm {
            pace: Some(Pace {
                games: 3,
                kills: 2.0,
                deaths: 0.1,
                assists: 2.0,
            }),
            ..recent(8, 4, 9.0, 1.0, 9.0, 0)
        };
        assert_eq!(form_title(&arena, Lean::Above), Some(FormTitle::Reliable));
        assert_eq!(
            form_title(&recent(20, 10, 4.0, 12.0, 5.0, 0), Lean::Below),
            Some(FormTitle::AlongForTheRide),
            "no pace, no reading of the counts"
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
            role: None,
        }
    }

    #[test]
    fn an_average_line_scores_six_and_a_stronger_one_more() {
        let same = vec![player(5, 5, 5, 20_000); 10];
        assert!(
            game_scores(&same, &Scoring::RIFT)
                .iter()
                .all(|&score| score == 6.0)
        );

        let mut game = same.clone();
        game[0] = player(15, 2, 10, 45_000);
        game[1] = player(0, 12, 2, 5_000);
        let scores = game_scores(&game, &Scoring::RIFT);
        assert!(scores[0] > 8.0 && scores[0] <= 10.0, "{scores:?}");
        assert!(scores[1] < 4.0, "{scores:?}");
        assert!(scores.iter().all(|score| (0.0..=10.0).contains(score)));
    }

    #[test]
    fn a_part_nobody_scored_in_is_left_out() {
        // Vision is zero for everyone (ARAM): it neither helps nor drags anyone.
        let game = vec![player(5, 5, 5, 20_000); 2];
        assert_eq!(game_scores(&game, &Scoring::RIFT), vec![6.0, 6.0]);
        assert_eq!(game_scores(&[], &Scoring::RIFT), Vec::<f64>::new());
    }

    #[test]
    fn one_number_stops_counting_at_three_averages() {
        let outlier = |damage: i64| {
            let mut game = vec![player(1, 5, 1, 10_000); 5];
            game[0].damage = damage;
            game_scores(&game, &Scoring::RIFT)[0]
        };
        assert_eq!(
            outlier(10_000_000),
            outlier(100_000_000),
            "past the cap more damage adds nothing"
        );
        assert!(outlier(10_000_000) < 10.0);
    }

    /// How often winer's MVP and SVP are WeGame's, over the games in `fixtures/wegame/`.
    fn agreement_with_wegame(games: &[Vec<[i64; 12]>], scoring: &Scoring) -> (f64, f64) {
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
                    role: [
                        None,
                        Some(Role::Tank),
                        Some(Role::Support),
                        Some(Role::Mage),
                        Some(Role::Assassin),
                        Some(Role::Marksman),
                        Some(Role::Fighter),
                    ][row[11] as usize],
                })
                .collect();
            let won: Vec<bool> = game.iter().map(|row| row[0] == 1).collect();
            let scores = game_scores(&players, scoring);
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
            rift: Vec<Vec<[i64; 12]>>,
            aram: Vec<Vec<[i64; 12]>>,
        }
        let games: Calibration = crate::test_support::fixture("wegame/calibration.json");
        assert_eq!((games.rift.len(), games.aram.len()), (126, 139));
        let rift = agreement_with_wegame(&games.rift, Scoring::of("CLASSIC"));
        let aram = agreement_with_wegame(&games.aram, Scoring::of("KIWI"));
        assert!(
            rift.0 >= 0.84 && rift.1 >= 0.83,
            "Summoner's Rift: {rift:?}"
        );
        assert!(aram.0 >= 0.79 && aram.1 >= 0.72, "Hextech ARAM: {aram:?}");
        // One set of weights for both would give up most of it on one side or the other.
        let swapped = agreement_with_wegame(&games.aram, &Scoring::RIFT);
        assert!(swapped.1 < aram.1, "{swapped:?}");
        // Without the champions' roles ARAM falls back to its base weights, and to fewer matches.
        let unknown: Vec<Vec<[i64; 12]>> = games
            .aram
            .iter()
            .map(|game| {
                game.iter()
                    .map(|row| {
                        let mut row = *row;
                        row[11] = 0;
                        row
                    })
                    .collect()
            })
            .collect();
        let roleless = agreement_with_wegame(&unknown, Scoring::of("KIWI"));
        assert!(roleless.0 < aram.0, "{roleless:?} against {aram:?}");
    }

    #[test]
    fn in_aram_a_line_is_weighed_against_its_role() {
        let mut game = vec![player(5, 5, 10, 20_000); 10];
        game[0].role = Some(Role::Tank);
        game[1].role = Some(Role::Marksman);
        for line in &mut game[..2] {
            line.kills = 12;
        }
        let scores = game_scores(&game, Scoring::of("KIWI"));
        assert!(
            scores[0] > scores[1],
            "twelve kills say more of a tank: {scores:?}"
        );
        let rift = game_scores(&game, Scoring::of("CLASSIC"));
        assert_eq!(rift[0], rift[1], "the Rift weighs no role: {rift:?}");
        assert_eq!(Role::parse("Marksman"), Some(Role::Marksman));
        assert_eq!(Role::parse("unknown"), None);
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
    fn one_players_grade_spreads_over_a_schemes_tiers_like_a_team() {
        let spread = |count| {
            (0..8)
                .map(|grade| tier_of_grade(grade, count))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            spread(5),
            [0, 0, 1, 2, 2, 3, 4, 4],
            "S+ S | A | B C | D | E F"
        );
        assert_eq!(
            spread(8),
            [0, 1, 2, 3, 4, 5, 6, 7],
            "峡谷八档 is the grades"
        );
        assert_eq!(spread(3), [0, 0, 0, 1, 1, 2, 2, 2]);
        assert_eq!(spread(7), [0, 1, 2, 3, 3, 4, 5, 6]);
        let middling = form_score(&recent(20, 10, 5.0, 5.0, 10.0, 0)).unwrap();
        assert_eq!(
            tier_of_grade(grade(middling, &FORM_GRADES), 5),
            2,
            "a middling player is the middle of five"
        );
        assert_eq!(tier_of_grade(9, 5), 4, "past F is still the last tier");
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
