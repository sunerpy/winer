//! winer's own performance numbers: a 0–10 score for every line of a scoreboard, the MVP and SVP
//! it implies, and a form score that ranks teammates in champ select. Both formulas are written
//! out here and on the site's rating page (`docs/site/rating.md`). WeGame does not publish its
//! rating; the game score's weights were fitted so that its MVP and SVP fall where WeGame's do as
//! often as possible (`Weights`).

use std::{cmp::Ordering, collections::HashMap};

use crate::view::{Award, ModeFamily, Position, RecentForm};

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
    let mut average = Average::default();
    for player in players {
        for (sum, value) in average.parts.iter_mut().zip(parts(player)) {
            *sum += value / count;
        }
        average.deaths += player.deaths as f64 / count;
    }
    players
        .iter()
        .map(|player| round1(line_score(player, &average, scoring)))
        .collect()
}

/// The average player a line is set against: the eight parts of [`Weights`] and deaths.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Average {
    parts: [f64; 8],
    deaths: f64,
}

/// One line's score against `average` under `scoring`, unrounded (see [`game_scores`]).
fn line_score(player: &Contribution, average: &Average, scoring: &Scoring) -> f64 {
    let (own, base) = (scoring.weights(player.role), &scoring.base);
    let (mut total, mut weight) = (0.0, 0.0);
    for (((value, usual), part_weight), base_weight) in parts(player)
        .into_iter()
        .zip(average.parts)
        .zip(own.parts())
        .zip(base.parts())
    {
        if base_weight > 0.0 && usual >= MEANINGFUL {
            total += part_weight * (value / usual).min(CAP);
            weight += base_weight;
        }
    }
    total += own.survival * ((average.deaths + 1.0) / (player.deaths as f64 + 1.0)).min(CAP);
    weight += base.survival;
    10.0 / (1.0 + (-STEEPNESS * (total / weight - CENTRE)).exp())
}

/// What the average player of a kind of game does a minute: kills, assists, damage to champions,
/// damage taken, gold, minions and monsters, vision score and crowd control seconds, then deaths.
/// Measured on real players' games from the shard's server (`fixtures/sgp/`, October 2026): 4,590
/// Rift lines and 8,370 of the two ARAMs. Arena and the rotating modes have none.
fn per_minute(family: ModeFamily) -> Option<Average> {
    match family {
        ModeFamily::Rift => Some(Average {
            parts: [0.240, 0.289, 779.3, 960.7, 419.3, 5.51, 0.975, 0.999],
            deaths: 0.241,
        }),
        ModeFamily::Aram => Some(Average {
            parts: [0.658, 1.513, 2204.8, 2426.5, 956.8, 2.24, 0.005, 1.880],
            deaths: 0.660,
        }),
        ModeFamily::Arena | ModeFamily::Other => None,
    }
}

/// One line's score where only the player's own row is known, as in the client's own list: set
/// against the average player of its kind of game over the same `seconds` ([`per_minute`]) instead
/// of the game's other players, with the same weights. It cannot tell a bloody game from a quiet
/// one, so it agrees less with WeGame than [`game_scores`] does. `None` for a kind of game with no
/// average, or a game with no length.
pub fn lite_score(player: &Contribution, game_mode: &str, seconds: i64) -> Option<f64> {
    let per_minute = per_minute(ModeFamily::of(game_mode))?;
    if seconds <= 0 {
        return None;
    }
    let minutes = seconds as f64 / 60.0;
    let average = Average {
        parts: per_minute.parts.map(|value| value * minutes),
        deaths: per_minute.deaths * minutes,
    };
    Some(round1(line_score(player, &average, Scoring::of(game_mode))))
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

/// 峡谷评级's lower bounds on a recent strength, S+ to E; anything lower is F. A strength is the
/// share of players below it, in tenths ([`strength`]), so each band holds a fixed share of them:
/// S+ the best 5%, S the next 10%, A 15%, B and C 20% each either side of the middle, D 15%, E 10%
/// and F the last 5%.
pub const FORM_GRADES: [f64; 7] = [9.5, 8.5, 7.0, 5.0, 3.0, 1.5, 0.5];
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
    /// The games' scores scatter at most 0.8 times as far as an ordinary player's, over eight scored
    /// games, above the middle: 定海神针.
    RockSolid,
    /// Above the middle with nothing else standing out: 靠谱队友.
    Reliable,
    // ---- Any tier ----
    /// Kills and deaths both at least 1.2 times the average: 一换一专业户.
    Trader,
    /// Assists at least 1.2 times the average, kills at most 0.85: 峡谷慈善家.
    Helper,
    /// The games' scores scatter at least 1.15 times as far as an ordinary player's, over eight
    /// scored games, at or below the middle: 峡谷老虎机.
    SlotMachine,
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
/// leaning's own, then of those any tier may have, then of how steady the games were, else the
/// leaning's plain one. Kills, deaths and assists are read against each game's mode ([`Pace`]), so
/// an ARAM player's ten deaths are an ordinary game, and a tier above the middle never gets a title
/// below it, nor the other way round. Needs five games, five with a mode's average for what the
/// counts say, and eight scored ones for how steady they were (`RecentForm::spread`).
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
    let spread = form.spread;
    let steadiness = match lean {
        Lean::Above => (
            spread.is_some_and(|spread| spread <= STEADY_SPREAD),
            RockSolid,
        ),
        Lean::Middle | Lean::Below => (
            spread.is_some_and(|spread| spread >= WILD_SPREAD),
            SlotMachine,
        ),
    };
    let plain = match lean {
        Lean::Above => Reliable,
        Lean::Middle => Steady,
        Lean::Below => AlongForTheRide,
    };
    Some(
        own.into_iter()
            .chain(any)
            .chain([steadiness])
            .find_map(|(fits, title)| fits.then_some(title))
            .unwrap_or(plain),
    )
}

/// A spread ([`Strength::spread`]) at or below this is steady enough for 定海神针, at or above
/// [`WILD_SPREAD`] wild enough for 峡谷老虎机: about the steadiest and the wildest sixth of the
/// sampled players.
pub const STEADY_SPREAD: f64 = 0.8;
pub const WILD_SPREAD: f64 = 1.15;

// ---- Recent strength: the games' scores, weighed and read against the players winer measured ----

/// Each game one older counts this much less, so about ten games halve a game's weight.
pub const RECENCY: f64 = 0.93;
/// A game in which someone else left or idled counts this much: four played against five, or five
/// against four, say little about the player.
pub const AWAY_WEIGHT: f64 = 0.4;
/// The win rate's share. A game's score already holds most of what wins games, so the result
/// itself counts little, and only after five wins and five losses are added to it.
pub const WIN_SHARE: f64 = 0.05;
const WIN_PRIOR: (f64, f64) = (5.0, 10.0);
/// A game read against the mode's average instead of its own players says less: its prior weighs
/// this many times as many games (measured 1.35 on the Rift, 1.45 in ARAM).
const LITE_PRIOR: f64 = 1.4;
/// The newest five games against the next fifteen nudge the strength by a tenth of the difference,
/// never more than 0.2 either way, once ten games have scores.
const TREND_GAMES: usize = 10;
const TREND_RECENT: usize = 5;
const TREND_GAIN: f64 = 0.1;
const TREND_CAP: f64 = 0.2;
/// A spread is read from this many scored games.
const SPREAD_GAMES: usize = 8;

/// What the recent strength knows of a kind of game, measured on real players' newest twenty games
/// from the shard's server (`fixtures/sgp/`, October 2026): 30 players met on the Rift, 40 in
/// Hextech ARAM.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Model {
    /// The average line's score once set against its position or role ([`par`]): where few games
    /// leave a player, and where a strength of 5.0 stands.
    baseline: f64,
    /// How many games the average weighs as before a player's own pull them away from it: a game's
    /// score scatters around the player's own level (1.81 on the Rift, 1.20 in ARAM) far more than
    /// the players' levels do around each other (0.34, 0.41), so twenty Rift games say less than
    /// twenty ARAM games.
    prior: f64,
    /// The standard deviation of the sampled players' raw strengths, their games scored against
    /// each game's players and against the mode's average.
    spread_of_players: [f64; 2],
    /// The sampled players' median spread of game scores, both ways.
    spread: [f64; 2],
}

const RIFT_MODEL: Model = Model {
    baseline: 6.04,
    prior: 25.0,
    spread_of_players: [0.232, 0.189],
    spread: [1.85, 1.815],
};
const ARAM_MODEL: Model = Model {
    baseline: 5.83,
    prior: 10.0,
    spread_of_players: [0.300, 0.255],
    spread: [1.134, 1.298],
};
/// Arena and the rotating modes were not measured: between the two.
const OTHER_MODEL: Model = Model {
    baseline: 5.93,
    prior: 15.0,
    spread_of_players: [0.27, 0.22],
    spread: [1.5, 1.55],
};

impl Model {
    fn of(family: ModeFamily) -> &'static Self {
        match family {
            ModeFamily::Rift => &RIFT_MODEL,
            ModeFamily::Aram => &ARAM_MODEL,
            ModeFamily::Arena | ModeFamily::Other => &OTHER_MODEL,
        }
    }
}

/// What a game's score is set against for the strength: the average line of the player's position
/// on the Rift, of the champion's role in ARAM. A support's line on the Rift scores 5.24 on
/// average, a jungler's 6.49, though neither is the better player for it; in ARAM, where the
/// champion is drawn, a support's scores 6.44 and an assassin's 5.69. The offset brings each to the
/// mode's average line (measured on the sampled games); nothing where the position or role is
/// unknown.
pub fn par(family: ModeFamily, position: Option<Position>, role: Option<Role>) -> f64 {
    match family {
        ModeFamily::Rift => match position {
            Some(Position::Top) => 0.16,
            Some(Position::Jungle) => -0.46,
            Some(Position::Middle) => -0.07,
            Some(Position::Bottom) => -0.41,
            Some(Position::Utility) => 0.79,
            None => 0.0,
        },
        ModeFamily::Aram => match role {
            Some(Role::Assassin) => 0.14,
            Some(Role::Fighter) => 0.05,
            Some(Role::Mage) => -0.15,
            Some(Role::Marksman) => 0.13,
            Some(Role::Support) => -0.61,
            Some(Role::Tank) => 0.08,
            None => 0.0,
        },
        ModeFamily::Arena | ModeFamily::Other => 0.0,
    }
}

/// One counted game as [`strength`] reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Played {
    /// The game's score set against its position or role ([`par`]): the score against its other
    /// players ([`game_scores`]), or against the mode's average player where only this player's row
    /// is known (`lite`, [`lite_score`]); `None` where neither can be had.
    pub score: Option<f64>,
    pub lite: bool,
    pub win: bool,
    /// Someone else in the game left or idled.
    pub away: bool,
    pub family: Option<ModeFamily>,
}

/// A player's recent strength.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strength {
    /// 0–10, to a tenth: the share of players whose raw strength is lower, in tenths, so 5.0 is a
    /// player with an average line in every game and 9.0 better than nine in ten; then the trend.
    pub score: f64,
    /// How far the games' scores scatter around their weighted mean, against how far an ordinary
    /// player's do (1.0); from [`SPREAD_GAMES`] scored games.
    pub spread: Option<f64>,
}

/// The recent strength of `games`, newest first; `None` without a game. The kind of game most of
/// them are decides what they are read against ([`Model`]).
///
/// - **Performance**: the games' scores, set against the player's position or role ([`par`]), each
///   weighed `0.93^i` for the `i`-th newest and by [`AWAY_WEIGHT`] where someone else left, then
///   averaged; then pulled toward the average line by `c = n / (n + prior)`, where `n` is how many
///   games the weights add up to (`(Σw)² / Σw²`, 17 for twenty games) and the prior is 25 games on
///   the Rift and 10 in ARAM (1.4 times that for games read against the mode's average).
/// - **Win score**: `10 × (wins + 5) / (games + 10)` over every counted game.
/// - **Raw strength**: `0.95 × performance + 0.05 × win score`, read on a normal curve around a
///   player with an average line in every game and half of them won, as wide as the sampled
///   players' raw strengths spread: the share of players below, in tenths, so that player is 5.0.
/// - **Trend**: the newest five scores against the next fifteen, a tenth of the difference, at most
///   ±0.2, once ten games have scores.
pub fn strength(games: &[Played]) -> Option<Strength> {
    let reading = reading(games)?;
    let model = reading.model;
    // An average line in every game, half of them won.
    let middle = (1.0 - WIN_SHARE) * model.baseline + WIN_SHARE * 5.0;
    let sd = model.spread_of_players[usize::from(reading.lite)];
    let share = 10.0 * normal_cdf((reading.raw - middle) / sd);
    Some(Strength {
        score: round1((share + reading.trend).clamp(0.0, 10.0)),
        spread: reading.spread,
    })
}

/// What [`strength`] reads from `games` before setting it against the sampled players.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    /// `0.95 × performance + 0.05 × win score`.
    raw: f64,
    /// Some scored game was read against its mode's average.
    lite: bool,
    model: &'static Model,
    spread: Option<f64>,
    trend: f64,
}

fn reading(games: &[Played]) -> Option<Reading> {
    if games.is_empty() {
        return None;
    }
    let scored: Vec<(f64, f64)> = games
        .iter()
        .filter_map(|game| game.score.map(|score| (score, game)))
        .enumerate()
        .map(|(index, (score, game))| {
            let away = if game.away { AWAY_WEIGHT } else { 1.0 };
            (score, RECENCY.powi(index as i32) * away)
        })
        .collect();
    let lite = games.iter().any(|game| game.lite && game.score.is_some());
    let model = Model::of(main_family(games));
    let total: f64 = scored.iter().map(|(_, weight)| weight).sum();
    let mean = (total > 0.0).then(|| {
        scored
            .iter()
            .map(|(score, weight)| score * weight)
            .sum::<f64>()
            / total
    });
    let performance = mean.map_or(model.baseline, |mean| {
        let squares: f64 = scored.iter().map(|(_, weight)| weight * weight).sum();
        let games = total * total / squares;
        let prior = model.prior * if lite { LITE_PRIOR } else { 1.0 };
        let confidence = games / (games + prior);
        confidence * mean + (1.0 - confidence) * model.baseline
    });
    let wins = games.iter().filter(|game| game.win).count() as f64;
    let win_score = 10.0 * (wins + WIN_PRIOR.0) / (games.len() as f64 + WIN_PRIOR.1);
    let spread = mean.filter(|_| scored.len() >= SPREAD_GAMES).map(|mean| {
        let variance = scored
            .iter()
            .map(|(score, weight)| weight * (score - mean).powi(2))
            .sum::<f64>()
            / total;
        variance.sqrt() / model.spread[usize::from(lite)]
    });
    Some(Reading {
        raw: (1.0 - WIN_SHARE) * performance + WIN_SHARE * win_score,
        lite,
        model,
        spread,
        trend: trend(&scored),
    })
}

/// The kind of game most of `games` are, the newest game's of those tied; the Rift's for none.
fn main_family(games: &[Played]) -> ModeFamily {
    let count = |family| {
        games
            .iter()
            .filter(|game| game.family == Some(family))
            .count()
    };
    let mut main: Option<(ModeFamily, usize)> = None;
    for family in games.iter().filter_map(|game| game.family) {
        let games = count(family);
        if main.is_none_or(|(_, most)| games > most) {
            main = Some((family, games));
        }
    }
    main.map_or(ModeFamily::Rift, |(family, _)| family)
}

/// The newest [`TREND_RECENT`] scores against the next fifteen: a tenth of the difference, at most
/// [`TREND_CAP`] either way; nothing before [`TREND_GAMES`] games have scores.
fn trend(scored: &[(f64, f64)]) -> f64 {
    if scored.len() < TREND_GAMES {
        return 0.0;
    }
    let mean = |games: &[(f64, f64)]| {
        games.iter().map(|(score, _)| score).sum::<f64>() / games.len() as f64
    };
    let (recent, before) = scored.split_at(TREND_RECENT);
    let before = &before[..before.len().min(15)];
    (TREND_GAIN * (mean(recent) - mean(before))).clamp(-TREND_CAP, TREND_CAP)
}

/// The standard normal distribution's cumulative probability at `z` (Abramowitz and Stegun 7.1.26,
/// within 1.5e-7).
fn normal_cdf(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 {
        0.5 * (1.0 + erf)
    } else {
        0.5 * (1.0 - erf)
    }
}

/// A recent strength, 0–10 ([`strength`]); `None` without a counted game.
pub fn form_score(form: &RecentForm) -> Option<f64> {
    form.score
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
        assert_eq!(grade(9.5, &FORM_GRADES), 0, "S+ starts at 9.5");
        assert_eq!(grade(9.49, &FORM_GRADES), 1);
        assert_eq!(grade(0.49, &FORM_GRADES), 7, "below every band is F");
        assert_eq!(
            grade(6.0, &GAME_GRADES),
            3,
            "the game's average player is a B"
        );
        let letter = |score| GRADE_LETTERS[usize::from(grade(score, &FORM_GRADES))];
        assert_eq!(
            (letter(5.0), letter(4.9)),
            ("B", "C"),
            "the middle player is between B and C"
        );
        let middling = strength(&played(&[ARAM_MODEL.baseline; 20], 10, ModeFamily::Aram));
        assert_eq!(middling.map(|strength| strength.score), Some(5.0));
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
    fn how_steady_the_games_were_titles_only_where_the_tier_allows() {
        let with = |spread, pace: RecentForm| RecentForm {
            spread: Some(spread),
            ..pace
        };
        let steady = with(0.7, paced(1.0, 1.0, 1.0, 0));
        assert_eq!(form_title(&steady, Lean::Above), Some(FormTitle::RockSolid));
        assert_eq!(form_title(&steady, Lean::Middle), Some(FormTitle::Steady));
        let wild = with(1.3, paced(1.0, 1.0, 1.0, 0));
        assert_eq!(form_title(&wild, Lean::Below), Some(FormTitle::SlotMachine));
        assert_eq!(
            form_title(&wild, Lean::Middle),
            Some(FormTitle::SlotMachine)
        );
        assert_eq!(
            form_title(&wild, Lean::Above),
            Some(FormTitle::Reliable),
            "a good tier is never a slot machine"
        );
        let dying = with(1.3, paced(1.0, 1.5, 1.0, 0));
        assert_eq!(
            form_title(&dying, Lean::Below),
            Some(FormTitle::GreyScreen),
            "what the counts say comes first"
        );
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

    /// `scores`, newest first, of one kind of game, the newest `wins` of them won.
    fn played(scores: &[f64], wins: usize, family: ModeFamily) -> Vec<Played> {
        scores
            .iter()
            .enumerate()
            .map(|(index, &score)| Played {
                score: Some(score),
                lite: false,
                win: index < wins,
                away: false,
                family: Some(family),
            })
            .collect()
    }

    fn score_of(games: &[Played]) -> f64 {
        strength(games).unwrap().score
    }

    #[test]
    fn strength_reads_every_games_score_and_needs_games() {
        assert_eq!(strength(&[]), None);
        let rift = |score, wins| score_of(&played(&[score; 20], wins, ModeFamily::Rift));
        assert!(
            rift(7.0, 10) > 9.0 && rift(5.0, 10) < 1.0,
            "{} {}",
            rift(7.0, 10),
            rift(5.0, 10)
        );
        // Winning counts a twentieth: ten more wins of twenty count for less than half a point
        // more in every game.
        let (fewer, more) = (rift(6.04, 5), rift(6.04, 15));
        assert!(more > fewer && more < rift(6.6, 5), "{fewer} {more}");
        // Two great games are not twenty good ones.
        let two = score_of(&played(&[9.0, 9.0], 2, ModeFamily::Aram));
        let twenty = score_of(&played(&[7.0; 20], 10, ModeFamily::Aram));
        assert!(two < twenty, "{two} {twenty}");
    }

    #[test]
    fn newer_games_count_more_and_a_game_someone_left_less() {
        let mut games = played(&[6.0; 20], 10, ModeFamily::Aram);
        games[0].score = Some(9.0);
        let newest = score_of(&games);
        games.swap(0, 19);
        let oldest = score_of(&games);
        assert!(newest > oldest, "{newest} {oldest}");
        games.swap(0, 19);
        games[0].away = true;
        let left = score_of(&games);
        assert!(left < newest && left > score_of(&played(&[6.0; 20], 10, ModeFamily::Aram)));
    }

    #[test]
    fn a_trend_moves_the_strength_at_most_two_tenths() {
        let rising: Vec<f64> = (0..20)
            .map(|index| if index < 5 { 9.0 } else { 5.0 })
            .collect();
        let reading = reading(&played(&rising, 10, ModeFamily::Aram)).unwrap();
        assert_eq!(reading.trend, TREND_CAP);
        let falling: Vec<f64> = (0..20)
            .map(|index| if index < 5 { 3.0 } else { 7.0 })
            .collect();
        assert_eq!(
            reading_of(&falling).trend,
            -TREND_CAP,
            "the newest five are the low ones"
        );
        assert_eq!(reading_of(&[6.0; 9]).trend, 0.0, "nine games show no trend");
        let gentle: Vec<f64> = (0..20)
            .map(|index| if index < 5 { 6.5 } else { 6.0 })
            .collect();
        assert!((reading_of(&gentle).trend - 0.05).abs() < 1e-9);
    }

    fn reading_of(scores: &[f64]) -> Reading {
        reading(&played(scores, scores.len() / 2, ModeFamily::Aram)).unwrap()
    }

    #[test]
    fn games_read_against_the_mode_pull_toward_the_average_harder() {
        let full = played(&[7.0; 20], 10, ModeFamily::Aram);
        let lite: Vec<Played> = full
            .iter()
            .map(|game| Played {
                lite: true,
                ..*game
            })
            .collect();
        let (full, lite) = (reading(&full).unwrap(), reading(&lite).unwrap());
        assert!(lite.lite && lite.raw < full.raw, "{lite:?} {full:?}");
    }

    #[test]
    fn the_most_played_kind_of_game_decides_the_model_and_a_tie_the_newest() {
        let mut games = played(&[6.0; 4], 2, ModeFamily::Rift);
        games.extend(played(&[6.0; 3], 1, ModeFamily::Aram));
        assert_eq!(main_family(&games), ModeFamily::Rift);
        games.rotate_left(4);
        games.extend(played(&[6.0], 0, ModeFamily::Aram));
        assert_eq!(
            main_family(&games),
            ModeFamily::Aram,
            "four each, ARAM newest"
        );
        assert_eq!(main_family(&[]), ModeFamily::Rift);
    }

    #[test]
    fn a_spread_reads_against_an_ordinary_players() {
        let steady = played(
            &[6.0, 6.2, 5.8, 6.1, 5.9, 6.0, 6.1, 5.9],
            4,
            ModeFamily::Aram,
        );
        assert!(strength(&steady).unwrap().spread.unwrap() < STEADY_SPREAD);
        let wild = played(
            &[9.5, 2.5, 9.0, 3.0, 8.5, 3.5, 9.0, 2.0],
            4,
            ModeFamily::Aram,
        );
        assert!(strength(&wild).unwrap().spread.unwrap() > WILD_SPREAD);
        assert_eq!(
            strength(&steady[..7]).unwrap().spread,
            None,
            "seven games say nothing of it"
        );
    }

    #[test]
    fn the_normal_curve_is_the_standard_one() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((normal_cdf(1.959_964) - 0.975).abs() < 1e-6);
        assert!((normal_cdf(-1.0) - 0.158_655_25).abs() < 1e-6);
    }

    #[test]
    fn a_position_or_a_role_sets_what_a_line_is_held_to() {
        assert_eq!(
            par(
                ModeFamily::Rift,
                Some(Position::Utility),
                Some(Role::Support)
            ),
            0.79
        );
        assert_eq!(
            par(ModeFamily::Rift, None, Some(Role::Support)),
            0.0,
            "the Rift reads positions"
        );
        assert_eq!(
            par(
                ModeFamily::Aram,
                Some(Position::Utility),
                Some(Role::Support)
            ),
            -0.61
        );
        assert_eq!(
            par(ModeFamily::Arena, Some(Position::Top), Some(Role::Tank)),
            0.0
        );
    }

    #[test]
    fn a_line_alone_is_read_against_its_modes_average_player() {
        let average = |minutes: f64| {
            let per = per_minute(ModeFamily::Aram).unwrap();
            let part = |index: usize| (per.parts[index] * minutes).round() as i64;
            Contribution {
                kills: part(0),
                assists: part(1),
                damage: part(2),
                tanked: part(3),
                gold: part(4),
                minions: part(5),
                vision: part(6),
                crowd_control: part(7),
                deaths: (per.deaths * minutes).round() as i64,
                role: None,
            }
        };
        let ordinary = lite_score(&average(20.0), "KIWI", 1200).unwrap();
        assert!((ordinary - 6.0).abs() <= 0.3, "{ordinary}");
        let mut better = average(20.0);
        better.gold *= 2;
        assert!(lite_score(&better, "ARAM", 1200).unwrap() > ordinary);
        let longer = lite_score(&average(40.0), "KIWI", 2400).unwrap();
        assert!(
            (longer - ordinary).abs() <= 0.2
                && lite_score(&average(20.0), "KIWI", 2400).unwrap() < 5.0,
            "a longer game expects more: {longer}"
        );
        assert_eq!(
            lite_score(&better, "CHERRY", 1200),
            None,
            "Arena has no average"
        );
        assert_eq!(lite_score(&better, "KIWI", 0), None);
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
        assert_eq!(
            tier_of_grade(grade(5.0, &FORM_GRADES), 5),
            2,
            "the middle player is the middle of five"
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

    // ---- Calibration: the constants above against the games they were measured on ----

    /// Spearman's rank correlation, ties ranked at their mean.
    fn spearman(a: &[f64], b: &[f64]) -> f64 {
        let ranks = |values: &[f64]| {
            let mut order: Vec<usize> = (0..values.len()).collect();
            order.sort_by(|&x, &y| values[x].total_cmp(&values[y]));
            let mut ranks = vec![0.0; values.len()];
            let mut start = 0;
            while start < order.len() {
                let mut end = start;
                while end + 1 < order.len() && values[order[end + 1]] == values[order[start]] {
                    end += 1;
                }
                for &index in &order[start..=end] {
                    ranks[index] = (start + end) as f64 / 2.0;
                }
                start = end + 1;
            }
            ranks
        };
        let (a, b) = (ranks(a), ranks(b));
        let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
        let (ma, mb) = (mean(&a), mean(&b));
        let cov: f64 = a.iter().zip(&b).map(|(x, y)| (x - ma) * (y - mb)).sum();
        let var = |values: &[f64], m: f64| values.iter().map(|x| (x - m).powi(2)).sum::<f64>();
        cov / (var(&a, ma) * var(&b, mb)).sqrt()
    }

    /// Each champion's first role, from the client's list (16.19).
    fn live_roles() -> Roles {
        let champions: Vec<crate::model::ChampionSummary> =
            crate::test_support::fixture("live/static/champion-summary.json");
        champions
            .iter()
            .filter_map(|champion| Some((champion.id, Role::parse(champion.roles.first()?)?)))
            .collect()
    }

    #[derive(serde::Deserialize)]
    struct WegameAram {
        games: Vec<WegameGame>,
    }

    #[derive(serde::Deserialize)]
    struct WegameGame {
        secs: i64,
        /// won, championId, score, kills, deaths, assists, damage, taken, gold, minions, vision,
        /// cc, healMates, exp, deadSecs, award, afk.
        players: Vec<[f64; 17]>,
    }

    fn wegame_line(row: &[f64; 17], roles: &Roles) -> Contribution {
        Contribution {
            kills: row[3] as i64,
            deaths: row[4] as i64,
            assists: row[5] as i64,
            damage: row[6] as i64,
            tanked: row[7] as i64,
            gold: row[8] as i64,
            minions: row[9] as i64,
            vision: row[10] as i64,
            crowd_control: row[11] as i64,
            role: roles.get(&(row[1] as i64)).copied(),
        }
    }

    #[test]
    fn game_scores_rank_each_aram_game_much_as_wegame_does() {
        let fixture: WegameAram = crate::test_support::fixture("wegame/aram-scores.json");
        assert_eq!(fixture.games.len(), 155);
        let roles = live_roles();
        let (mut within, mut mvp, mut svp) = (0.0, 0, 0);
        let (mut ours, mut lite, mut theirs) = (Vec::new(), Vec::new(), Vec::new());
        for game in &fixture.games {
            let lines: Vec<Contribution> = game
                .players
                .iter()
                .map(|row| wegame_line(row, &roles))
                .collect();
            let scores = game_scores(&lines, Scoring::of("KIWI"));
            let wegame: Vec<f64> = game.players.iter().map(|row| row[2]).collect();
            within += spearman(&scores, &wegame);
            let won: Vec<bool> = game.players.iter().map(|row| row[0] == 1.0).collect();
            for (row, award) in game.players.iter().zip(awards(&scores, &won)) {
                mvp += i32::from(award == Some(Award::Mvp) && row[15] == 1.0);
                svp += i32::from(award == Some(Award::Svp) && row[15] == 2.0);
            }
            ours.extend(&scores);
            theirs.extend(&wegame);
            lite.extend(
                lines
                    .iter()
                    .map(|line| lite_score(line, "KIWI", game.secs).unwrap()),
            );
        }
        let games = fixture.games.len() as f64;
        let within = within / games;
        let (mvp, svp) = (f64::from(mvp) / games, f64::from(svp) / games);
        assert!(within >= 0.84, "within a game: {within}");
        assert!(mvp >= 0.76 && svp >= 0.70, "MVP {mvp}, SVP {svp}");
        // Across games too, and a line alone against the mode's average agrees less.
        let (across, alone) = (spearman(&ours, &theirs), spearman(&lite, &theirs));
        assert!(
            across >= 0.81 && alone >= 0.65 && alone < across,
            "{across} {alone}"
        );
    }

    #[derive(serde::Deserialize)]
    struct Sampled {
        aram: Vec<Vec<SampledGame>>,
        rift: Vec<Vec<SampledGame>>,
    }

    #[derive(serde::Deserialize)]
    struct SampledGame {
        mode: String,
        kind: String,
        secs: i64,
        me: usize,
        lines: Vec<SampledLine>,
    }

    /// team, win, championId, kills, deaths, assists, damage, taken, gold, minions, vision, cc,
    /// afk, earlySurrender, position.
    type SampledLine = (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        String,
    );

    fn sampled_line(line: &SampledLine, roles: &Roles) -> Contribution {
        Contribution {
            kills: line.3,
            deaths: line.4,
            assists: line.5,
            damage: line.6,
            tanked: line.7,
            gold: line.8,
            minions: line.9,
            vision: line.10,
            crowd_control: line.11,
            role: roles.get(&line.2).copied(),
        }
    }

    impl SampledGame {
        fn counted(&self, family: ModeFamily) -> bool {
            self.kind == "matched"
                && ModeFamily::of(&self.mode) == family
                && self.lines[self.me].13 == 0
                && self.secs >= 240
        }

        /// Every line's score against the game's players and alone against the mode, each set
        /// against its position or role.
        fn scores(&self, roles: &Roles) -> Vec<(f64, f64)> {
            let lines: Vec<Contribution> = self
                .lines
                .iter()
                .map(|line| sampled_line(line, roles))
                .collect();
            let family = ModeFamily::of(&self.mode);
            game_scores(&lines, Scoring::of(&self.mode))
                .into_iter()
                .zip(&lines)
                .zip(&self.lines)
                .map(|((full, contribution), line)| {
                    let par = par(family, Position::parse(&line.14), contribution.role);
                    let lite = lite_score(contribution, &self.mode, self.secs).unwrap();
                    ((full + par).clamp(0.0, 10.0), (lite + par).clamp(0.0, 10.0))
                })
                .collect()
        }

        fn played(&self, roles: &Roles, lite: bool) -> Played {
            let (full, alone) = self.scores(roles)[self.me];
            Played {
                score: Some(if lite { alone } else { full }),
                lite,
                win: self.lines[self.me].1 == 1,
                away: self
                    .lines
                    .iter()
                    .enumerate()
                    .any(|(index, line)| index != self.me && line.12 == 1),
                family: Some(ModeFamily::of(&self.mode)),
            }
        }
    }

    /// Each sampled player's newest twenty counted games of `family`, for those with ten.
    fn histories(players: &[Vec<SampledGame>], family: ModeFamily) -> Vec<Vec<&SampledGame>> {
        players
            .iter()
            .map(|games| {
                games
                    .iter()
                    .filter(|game| game.counted(family))
                    .take(20)
                    .collect::<Vec<_>>()
            })
            .filter(|games| games.len() >= 10)
            .collect()
    }

    fn mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn sd(values: &[f64]) -> f64 {
        let m = mean(values);
        (values.iter().map(|value| (value - m).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
    }

    fn median(values: &[f64]) -> f64 {
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            (sorted[middle - 1] + sorted[middle]) / 2.0
        } else {
            sorted[middle]
        }
    }

    #[test]
    fn the_models_are_what_the_sampled_players_measure() {
        let sampled: Sampled = crate::test_support::fixture("sgp/players.json");
        assert_eq!((sampled.aram.len(), sampled.rift.len()), (40, 30));
        let roles = live_roles();
        for (family, players, model) in [
            (ModeFamily::Aram, &sampled.aram, &ARAM_MODEL),
            (ModeFamily::Rift, &sampled.rift, &RIFT_MODEL),
        ] {
            let histories = histories(players, family);
            for lite in [false, true] {
                let readings: Vec<Reading> = histories
                    .iter()
                    .map(|games| {
                        let played: Vec<Played> =
                            games.iter().map(|game| game.played(&roles, lite)).collect();
                        reading(&played).unwrap()
                    })
                    .collect();
                let raw: Vec<f64> = readings.iter().map(|reading| reading.raw).collect();
                let expected = model.spread_of_players[usize::from(lite)];
                assert!(
                    (sd(&raw) / expected - 1.0).abs() < 0.1,
                    "{family:?} lite={lite}: raw strengths spread {} against {expected}",
                    sd(&raw)
                );
                let spreads: Vec<f64> = readings
                    .iter()
                    .filter_map(|reading| reading.spread)
                    .collect();
                assert!(
                    (median(&spreads) - 1.0).abs() < 0.05,
                    "{family:?} lite={lite}: median spread {}",
                    median(&spreads)
                );
            }
        }
    }

    #[test]
    fn set_against_position_or_role_the_average_line_is_the_models() {
        let sampled: Sampled = crate::test_support::fixture("sgp/players.json");
        let roles = live_roles();
        for (family, players, model) in [
            (ModeFamily::Aram, &sampled.aram, &ARAM_MODEL),
            (ModeFamily::Rift, &sampled.rift, &RIFT_MODEL),
        ] {
            // Each game once, though several sampled players played it.
            let mut seen = std::collections::HashSet::new();
            let mut lines: Vec<(f64, f64, String)> = Vec::new();
            for game in players.iter().flatten().filter(|game| game.counted(family)) {
                let key: Vec<i64> = game.lines.iter().map(|line| line.6).collect();
                if !seen.insert((game.secs, key)) {
                    continue;
                }
                for ((full, lite), line) in game.scores(&roles).into_iter().zip(&game.lines) {
                    lines.push((full, lite, line.14.clone()));
                }
            }
            let full: Vec<f64> = lines.iter().map(|line| line.0).collect();
            let lite: Vec<f64> = lines.iter().map(|line| line.1).collect();
            assert!(
                (mean(&full) - model.baseline).abs() < 0.03,
                "{family:?}: {}",
                mean(&full)
            );
            assert!(
                (mean(&lite) - model.baseline).abs() < 0.05,
                "{family:?}: {}",
                mean(&lite)
            );
            if family == ModeFamily::Rift {
                for position in ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"] {
                    let at: Vec<f64> = lines
                        .iter()
                        .filter(|line| line.2 == position)
                        .map(|line| line.0)
                        .collect();
                    assert!(
                        (mean(&at) - model.baseline).abs() < 0.1,
                        "{position}: {}",
                        mean(&at)
                    );
                }
            }
        }
    }

    #[test]
    fn a_line_alone_ranks_the_sampled_players_much_as_their_whole_games_do() {
        let sampled: Sampled = crate::test_support::fixture("sgp/players.json");
        let roles = live_roles();
        for (family, players) in [
            (ModeFamily::Aram, &sampled.aram),
            (ModeFamily::Rift, &sampled.rift),
        ] {
            let both: Vec<(f64, f64)> = histories(players, family)
                .iter()
                .map(|games| {
                    let read = |lite| {
                        let played: Vec<Played> =
                            games.iter().map(|game| game.played(&roles, lite)).collect();
                        reading(&played).unwrap().raw
                    };
                    (read(false), read(true))
                })
                .collect();
            let full: Vec<f64> = both.iter().map(|pair| pair.0).collect();
            let lite: Vec<f64> = both.iter().map(|pair| pair.1).collect();
            let agreement = spearman(&full, &lite);
            assert!(agreement >= 0.8, "{family:?}: {agreement}");
        }
    }

    #[test]
    fn the_per_minute_averages_are_the_sampled_lines() {
        let sampled: Sampled = crate::test_support::fixture("sgp/players.json");
        for (family, players) in [
            (ModeFamily::Aram, sampled.aram.iter().chain(&sampled.rift)),
            (ModeFamily::Rift, sampled.aram.iter().chain(&sampled.rift)),
        ] {
            let (mut sums, mut lines) = ([0.0; 9], 0.0);
            for game in players.flatten() {
                if game.kind != "matched" || ModeFamily::of(&game.mode) != family || game.secs < 240
                {
                    continue;
                }
                let minutes = game.secs as f64 / 60.0;
                for line in &game.lines {
                    let values = [
                        line.3, line.5, line.6, line.7, line.8, line.9, line.10, line.11, line.4,
                    ];
                    for (sum, value) in sums.iter_mut().zip(values) {
                        *sum += value as f64 / minutes;
                    }
                    lines += 1.0;
                }
            }
            let expected = per_minute(family).unwrap();
            let measured = sums.map(|sum| sum / lines);
            for (index, want) in expected.parts.iter().chain([&expected.deaths]).enumerate() {
                assert!(
                    (measured[index] / want - 1.0).abs() < 0.05,
                    "{family:?} part {index}: {} against {want}",
                    measured[index]
                );
            }
        }
    }
}
