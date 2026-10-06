//! The callout: in champ select every rated teammate's standing and recent form, one chat line
//! each; in the game, the enemy to watch and the one to go after. Rating the seats is `live`'s job;
//! this module only names the standings and writes the lines.

use std::cmp::Ordering;

use crate::{
    rating::{self, FormTitle},
    settings::{CalloutRule, General, Language, TierSet},
    view::{
        CalloutSkip, ChampSelectView, GameView, Phase, PlayerStats, PlayerSummary, Seat,
        SeatRating, Side, TimerView,
    },
};

/// The name every callout carries on its first line, after the side.
pub fn signature(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "winer 战绩鉴定",
        Language::En => "winer rating",
    }
}

/// The side as the callout's first line names it.
pub fn side_tag(side: Side, language: Language) -> &'static str {
    match (side, language) {
        (Side::Blue, Language::ZhCn) => "【蓝色方】",
        (Side::Red, Language::ZhCn) => "【红色方】",
        (Side::Blue, Language::En) => "[Blue side]",
        (Side::Red, Language::En) => "[Red side]",
    }
}

/// The line written for each player when the user has not written their own. It names the seat,
/// not the champion: champions change during champ select, seats do not. A comma keeps the name
/// apart from the numbers after it, and the score is called what the window calls it, 战力 (the
/// form score), not 评分, which is a game's.
pub fn template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}"
        }
        Language::En => {
            "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        }
    }
}

/// The default lines of earlier versions: up to 0.0.2 they named the champion where the seat now
/// stands, up to 0.0.3 they called the form score 评分 and ran the name into the numbers.
const FORMER_TEMPLATES: [(Language, &str); 4] = [
    (
        Language::ZhCn,
        "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
    ),
    (
        Language::En,
        "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
    ),
    (
        Language::ZhCn,
        "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
    ),
    (
        Language::En,
        "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
    ),
];

/// The language whose former default line `template` is, character for character.
pub(crate) fn former_default(template: &str) -> Option<Language> {
    FORMER_TEMPLATES
        .iter()
        .find(|(_, former)| *former == template)
        .map(|(language, _)| *language)
}

/// A player's place in their team as champ select lists it, counted from 1: `1L` in Chinese (the
/// players' own shorthand for the list's first row), `P1` in English.
pub fn seat_label(seat: usize, language: Language) -> String {
    match language {
        Language::ZhCn => format!("{seat}L"),
        Language::En => format!("P{seat}"),
    }
}

fn preset(set: TierSet, language: Language) -> &'static [&'static str] {
    match (set, language) {
        (TierSet::RiftFive, Language::ZhCn) => &[
            "峡谷通天代",
            "人形防御塔",
            "峡谷公务员",
            "移动眼位",
            "纯正牛马",
        ],
        (TierSet::RiftFive, Language::En) => &[
            "Rift Demigod",
            "Human Turret",
            "Rift Civil Servant",
            "Walking Ward",
            "Pure Workhorse",
        ],
        (TierSet::Grades, Language::ZhCn) => &[
            "峡谷通天代",
            "人形防御塔",
            "峡谷公务员",
            "有用之人",
            "移动眼位",
            "峡谷提款机",
            "泉水观察员",
            "纯正牛马",
        ],
        (TierSet::Grades, Language::En) => &[
            "Rift Demigod",
            "Human Turret",
            "Rift Civil Servant",
            "Useful Person",
            "Walking Ward",
            "Rift ATM",
            "Fountain Watcher",
            "Pure Workhorse",
        ],
        (TierSet::HorseUniverse, Language::ZhCn) => &[
            "独角兽",
            "千里马",
            "汗血宝马",
            "峡谷骡子",
            "跛脚马",
            "纯牛马",
            "赛博牛马",
        ],
        (TierSet::HorseUniverse, Language::En) => &[
            "Unicorn",
            "Thousand-li Steed",
            "Blood-sweating Horse",
            "Rift Mule",
            "Lame Horse",
            "Pure Workhorse",
            "Cyber Workhorse",
        ],
        (TierSet::Horses | TierSet::Custom, Language::ZhCn) => &["上等马", "中等马", "下等马"],
        (TierSet::Horses | TierSet::Custom, Language::En) => {
            &["Top horse", "Middle horse", "Bottom horse"]
        }
        (TierSet::HorsesFive, Language::ZhCn) => {
            &["独角马", "上等马", "中等马", "下等马", "纯牛马"]
        }
        (TierSet::HorsesFive, Language::En) => &[
            "Unicorn",
            "Top horse",
            "Middle horse",
            "Bottom horse",
            "Pack mule",
        ],
        (TierSet::Rift, Language::ZhCn) => &["峡谷之王", "大腿", "正常发挥", "混子", "提款机"],
        (TierSet::Rift, Language::En) => &[
            "King of the Rift",
            "Carry",
            "Holding up",
            "Passenger",
            "Walking ATM",
        ],
    }
}

/// The tiers' names, best first: the chosen set's, or the user's own when they wrote at least two
/// (fewer cannot rank anyone, so the three horses stand in).
pub fn tier_names(rule: &CalloutRule, language: Language) -> Vec<String> {
    if rule.tiers == TierSet::Custom {
        let own: Vec<String> = rule
            .custom_tiers
            .iter()
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        if own.len() >= 2 {
            return own;
        }
    }
    preset(rule.tiers, language)
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

/// What the chat says about each tier, a few ways for the tiers of the default set so a line does
/// not repeat itself game after game; one for the other sets and none for the user's own names.
fn quips(set: TierSet, language: Language) -> &'static [&'static [&'static str]] {
    match (set, language) {
        (TierSet::RiftFive, Language::ZhCn) => &[
            &[
                "对面五个人准备举报代练",
                "这不是队友，这是系统派来的补偿机制",
                "建议对面直接投降，省点时间",
            ],
            &[
                "稳得离谱，能C还能活",
                "塔在人在，人在塔也在",
                "伤害吃满，血条还剩一半",
            ],
            &[
                "不一定惊艳，但该干的活全干了",
                "按时上班，准时打卡",
                "无功无过，绩效合格",
            ],
            &[
                "活着最大的价值是提供视野",
                "照亮了队友前进的方向，然后倒下",
                "站在哪里，哪里就有视野",
            ],
            &[
                "队友看完战绩陷入沉思",
                "勤勤恳恳地给对面创造游戏体验",
                "这把要是赢了，全靠队友",
            ],
        ],
        (TierSet::RiftFive, Language::En) => &[
            &["the other team is filing a boosting report"],
            &["absurdly steady: carries and survives"],
            &["not flashy, but every job got done"],
            &["their best contribution is vision"],
            &["teammates read the stats and fall silent"],
        ],
        (TierSet::Grades, Language::ZhCn) => &[
            &["对面五个人举报代练的水平"],
            &["稳得离谱，能C还能活"],
            &["不一定惊艳，但该干的活全干了"],
            &["偶尔犯病，总体还能抢救"],
            &["活着最大的价值是提供视野"],
            &["对面经济主要来源"],
            &["黑白屏时间比打游戏时间长"],
            &["队友看完战绩陷入沉思"],
        ],
        (TierSet::Grades, Language::En) => &[
            &["plays like the other side's boosting report"],
            &["absurdly steady: carries and survives"],
            &["not flashy, but every job got done"],
            &["has episodes, but can be saved"],
            &["their best contribution is vision"],
            &["the enemy's main source of gold"],
            &["more grey screen than game"],
            &["teammates read the stats and fall silent"],
        ],
        (TierSet::HorseUniverse, Language::ZhCn) => &[
            &["这不是队友，这是系统派来的补偿机制"],
            &["给点资源真能跑起来"],
            &["兢兢业业，偶尔还能C"],
            &["能干活，但你最好别对他有太多期待"],
            &["理论上能跑，实际上三步一送"],
            &["勤勤恳恳地给对面创造游戏体验"],
            &["疑似脚本，但脚本可能都比他强"],
        ],
        (TierSet::HorseUniverse, Language::En) => &[
            &["not a teammate: the system's compensation"],
            &["give them resources and they run"],
            &["diligent, and carries now and then"],
            &["works, but expect little"],
            &["could run in theory; feeds in practice"],
            &["works hard at giving the enemy a good time"],
            &["looks scripted, and a script would do better"],
        ],
        (TierSet::HorsesFive, Language::ZhCn) => &[
            &["稀有物种，抽到就是赚到"],
            &["田忌赛马里最贵的那匹"],
            &["不功不过，稳定拉磨"],
            &["能不能赢，全看对面更下等"],
            &["拉磨的间隙顺手送了几个人头"],
        ],
        (TierSet::HorsesFive, Language::En) => &[
            &["a rare breed: lucky to have one"],
            &["the priciest horse in the race"],
            &["neither good nor bad: steady work"],
            &["wins only if theirs is worse"],
            &["fed a few kills between chores"],
        ],
        (TierSet::Horses, Language::ZhCn) => &[
            &["田忌赛马里最贵的那匹"],
            &["不功不过，稳定拉磨"],
            &["能不能赢，全看对面更下等"],
        ],
        (TierSet::Horses, Language::En) => &[
            &["the priciest horse in the race"],
            &["neither good nor bad: steady work"],
            &["wins only if theirs is worse"],
        ],
        (TierSet::Rift, Language::ZhCn) => &[
            &["峡谷是他家后花园"],
            &["抱紧了，这把能赢"],
            &["发挥稳定，没什么好说的"],
            &["躺赢的姿势很标准"],
            &["对面的经济全靠他接济"],
        ],
        (TierSet::Rift, Language::En) => &[
            &["the Rift is their back garden"],
            &["hold on tight: this one wins"],
            &["steady, nothing to add"],
            &["a textbook passenger"],
            &["the enemy's gold comes from here"],
        ],
        (TierSet::Custom, _) => &[],
    }
}

/// One of `tier`'s quips, the same for a player throughout one champ select and likely another
/// in the next game.
pub fn quip(
    set: TierSet,
    language: Language,
    tier: usize,
    puuid: &str,
    game_id: i64,
) -> Option<&'static str> {
    let options = quips(set, language).get(tier)?;
    // FNV-1a: stable across runs and platforms, unlike the standard hasher.
    let seed = puuid
        .bytes()
        .chain(game_id.to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    options.get((seed % options.len() as u64) as usize).copied()
}

/// How a team is rated: the tier names, best first; whether they are absolute grades (a score maps
/// to one on its own) or ranks within the team; and the titles' language, while titles are on.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranking {
    pub set: TierSet,
    pub names: Vec<String>,
    pub absolute: bool,
    pub language: Language,
    pub titles: bool,
}

/// The ranking `rule` and the general settings ask for.
pub fn ranking(rule: &CalloutRule, general: &General) -> Ranking {
    let names = tier_names(rule, general.language);
    Ranking {
        // The user's own names replace a set only once there are enough of them.
        set: if rule.tiers == TierSet::Custom && names.len() >= 2 {
            TierSet::Custom
        } else if rule.tiers == TierSet::Custom {
            TierSet::Horses
        } else {
            rule.tiers
        },
        absolute: rule.tiers.absolute(),
        names,
        language: general.language,
        titles: general.titles,
    }
}

impl Ranking {
    /// What the chat says about `tier` for this player in this game, if the set has words for it.
    pub fn quip(&self, tier: u8, puuid: &str, game_id: i64) -> Option<String> {
        quip(self.set, self.language, usize::from(tier), puuid, game_id).map(str::to_owned)
    }
}

/// A form title as the window and the chat say it.
pub fn title_name(title: FormTitle, language: Language) -> &'static str {
    match (title, language) {
        (FormTitle::OnAStreak, Language::ZhCn) => "版本答案",
        (FormTitle::OnAStreak, Language::En) => "Patch Champion",
        (FormTitle::GivingAway, Language::ZhCn) => "排位慈善家",
        (FormTitle::GivingAway, Language::En) => "Ranked Philanthropist",
        (FormTitle::Bodhisattva, Language::ZhCn) => "电竞菩萨",
        (FormTitle::Bodhisattva, Language::En) => "Esports Bodhisattva",
        (FormTitle::Immortal, Language::ZhCn) => "峡谷永生者",
        (FormTitle::Immortal, Language::En) => "Rift Immortal",
        (FormTitle::Trader, Language::ZhCn) => "一换一专业户",
        (FormTitle::Trader, Language::En) => "One-for-one Trader",
        (FormTitle::GreyScreen, Language::ZhCn) => "黑白电视机资深会员",
        (FormTitle::GreyScreen, Language::En) => "Grey-screen Regular",
        (FormTitle::Helper, Language::ZhCn) => "峡谷慈善家",
        (FormTitle::Helper, Language::En) => "Rift Philanthropist",
    }
}

/// Every teammate's stats are settled and someone was rated: the lines will not change any more.
pub fn ready(view: &ChampSelectView) -> bool {
    view.my_team
        .iter()
        .all(|seat| !matches!(seat.stats, PlayerStats::Loading))
        && view.my_team.iter().any(|seat| seat.rating.is_some())
}

/// The first line, then one line per rated teammate, the best standing first and by score within
/// a standing. The first line is the side, winer's name and the user's opening line, if any.
/// A teammate's seat is their place in `view.my_team`, which lists the team as champ select does.
/// `champion` names a champion id the way players call it (`安妮`, not `黑暗之女`). Nobody rated
/// means nothing to say, first line included.
pub fn lines(
    view: &ChampSelectView,
    rule: &CalloutRule,
    language: Language,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let template = match rule.template.trim() {
        "" => template(language),
        own => own,
    };
    // Seats are counted before anyone is left out, so they stay the ones champ select shows.
    let mut rated: Vec<(usize, &Seat, &SeatRating)> = view
        .my_team
        .iter()
        .enumerate()
        .filter(|(_, seat)| rule.include_self || !seat.is_self)
        .filter_map(|(index, seat)| Some((index + 1, seat, seat.rating.as_ref()?)))
        .collect();
    rated.sort_by(|a, b| {
        a.2.tier
            .cmp(&b.2.tier)
            .then(b.2.score.total_cmp(&a.2.score))
    });
    let players: Vec<String> = rated
        .into_iter()
        .filter_map(|(number, seat, rating)| {
            let seat_label = seat_label(number, language);
            line(template, &seat_label, seat, rating, &champion)
        })
        .collect();
    if players.is_empty() {
        return players;
    }
    // The side leads, then winer's name, which every callout carries, then the opening line.
    let mut first = match (view.side, language) {
        (Some(side), Language::ZhCn) => {
            format!("{}{}", side_tag(side, language), signature(language))
        }
        (Some(side), Language::En) => {
            format!("{} {}", side_tag(side, language), signature(language))
        }
        (None, _) => signature(language).to_owned(),
    };
    let header = rule.header.trim();
    if !header.is_empty() {
        first.push_str(" · ");
        first.push_str(header);
    }
    std::iter::once(first).chain(players).collect()
}

fn line(
    template: &str,
    seat_label: &str,
    seat: &Seat,
    rating: &SeatRating,
    champion: &impl Fn(i64) -> Option<String>,
) -> Option<String> {
    let PlayerStats::Ready(summary) = &seat.stats else {
        return None;
    };
    let form = &summary.recent;
    let games = f64::from(form.games.max(1));
    let name = seat
        .name
        .as_ref()
        .or(summary.name.as_ref())
        .map(|name| name.game_name.clone())
        .unwrap_or_default();
    let values = [
        ("{standing}", rating.label.clone()),
        ("{seat}", seat_label.to_owned()),
        ("{champion}", champion(seat.champion_id).unwrap_or_default()),
        ("{name}", name),
        ("{games}", form.games.to_string()),
        (
            "{winRate}",
            format!("{:.0}%", f64::from(form.wins) * 100.0 / games),
        ),
        (
            "{kda}",
            format!("{:.1}", (form.kills + form.assists) / form.deaths.max(1.0)),
        ),
        ("{score}", format!("{:.1}", rating.score)),
        (
            "{title}",
            // An English title brings its own space, so a line without one has no gap to leave.
            rating.title.as_deref().map_or_else(String::new, |title| {
                if title.is_ascii() {
                    format!(" [{title}]")
                } else {
                    format!("「{title}」")
                }
            }),
        ),
        (
            "{quip}",
            rating.quip.as_deref().map_or_else(String::new, |quip| {
                if quip.is_ascii() {
                    format!(", {quip}")
                } else {
                    format!("，{quip}")
                }
            }),
        ),
    ];
    let mut text = template.to_owned();
    for (key, value) in values {
        text = text.replace(key, &value);
    }
    // A blank value (no champion yet, a hidden name) must not leave a gap in the sentence:
    // full-width punctuation takes no space on either side, and a comma none before it.
    let mut text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for mark in ["：", "，", "；"] {
        text = text
            .replace(&format!(" {mark}"), mark)
            .replace(&format!("{mark} "), mark);
    }
    Some(text.replace(" ,", ","))
}

/// A seat holding the user's own recent form in `tier` of `ranking`, for the previews: their most
/// played champion stands in for `{champion}`.
fn sample(me: &PlayerSummary, ranking: &Ranking, tier: usize, is_self: bool) -> Seat {
    let title = ranking
        .titles
        .then(|| rating::form_title(&me.recent))
        .flatten()
        .map(|title| title_name(title, ranking.language).to_owned());
    Seat {
        puuid: Some(me.puuid.clone()),
        name: me.name.clone(),
        champion_id: me
            .recent
            .champions
            .first()
            .map_or(0, |form| form.champion_id),
        intent: false,
        position: None,
        spells: [0, 0],
        is_self,
        premade: None,
        stats: PlayerStats::Ready(Box::new(me.clone())),
        rating: Some(SeatRating {
            score: rating::form_score(&me.recent).unwrap_or(5.0),
            tier: tier as u8,
            tiers: ranking.names.len() as u8,
            label: ranking.names[tier].clone(),
            grade: ranking.absolute.then_some(tier as u8),
            title,
            quip: ranking.quip(tier as u8, &me.puuid, 0),
        }),
    }
}

/// What `rule` would send, shown with the user's own recent form in every tier, so names and
/// template can be judged before a champ select. Their most played champion stands in for
/// `{champion}`, the blue side for whichever the game gives, and the tiers take the seats in
/// order, so the sample lines read 1L, 2L, … from the best tier down.
pub fn preview(
    me: &PlayerSummary,
    rule: &CalloutRule,
    general: &General,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let language = general.language;
    let ranking = ranking(rule, general);
    let my_team = (0..ranking.names.len())
        .map(|tier| sample(me, &ranking, tier, false))
        .collect();
    let view = ChampSelectView {
        game_id: 0,
        queue_id: 0,
        timer: TimerView::default(),
        my_team,
        their_team: Vec::new(),
        my_bans: Vec::new(),
        their_bans: Vec::new(),
        bench_enabled: false,
        bench: Vec::new(),
        rerolls_remaining: 0,
        callout: Vec::new(),
        side: Some(Side::Blue),
    };
    lines(
        &view,
        &CalloutRule {
            include_self: true,
            ..rule.clone()
        },
        language,
        champion,
    )
}

// ---- In the game: the other team, typed into the game's chat by the callout's shortcut ----

/// The other team's side, as the in-game callout's first line names it.
pub fn enemy_tag(side: Side, language: Language) -> &'static str {
    match (side, language) {
        (Side::Blue, Language::ZhCn) => "【敌方·蓝色方】",
        (Side::Red, Language::ZhCn) => "【敌方·红色方】",
        (Side::Blue, Language::En) => "[Enemy · Blue side]",
        (Side::Red, Language::En) => "[Enemy · Red side]",
    }
}

/// The line about the enemy to watch, when the user has not written their own. In the game the
/// champion is what identifies a player: it no longer changes, and it is what the map shows.
pub fn watch_template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "小心 {champion} {name}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}"
        }
        Language::En => {
            "Watch {champion} ({name}): {standing}, {winRate} in {games} games, KDA {kda}{title}"
        }
    }
}

/// The line about the enemy to go after, when the user has not written their own.
pub fn target_template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "对面 {champion} {name}：{standing}，近{games}场胜率{winRate}，可以多抓",
        Language::En => "Go after {champion} ({name}): {standing}, {winRate} in {games} games",
    }
}

/// Where a rating stands against the middle of its scheme: above it (`Less`: tier 0 is the best),
/// below it (`Greater`) or at it. A ranking splits around its middle tier (of five, the first two
/// are above and the last two below); of the eight grades, B and C, the bands either side of an
/// ordinary player's form, are the middle.
fn lean(rating: &SeatRating) -> Ordering {
    match rating.grade {
        Some(grade) if grade <= 2 => Ordering::Less,
        Some(grade) if grade >= 5 => Ordering::Greater,
        Some(_) => Ordering::Equal,
        None => (2 * u16::from(rating.tier) + 1).cmp(&u16::from(rating.tiers)),
    }
}

/// Who the in-game callout talks about, as places in `team`: the enemy to watch, the best rated
/// above the middle of the scheme, and the one to go after, the worst rated below it (the better
/// tier first, then the higher score, then the earlier seat). A player without a rating (hidden,
/// still loading, no games) is never chosen; where nobody stands out, nobody is.
pub fn pick(team: &[Seat]) -> (Option<usize>, Option<usize>) {
    let rated = || {
        team.iter()
            .enumerate()
            .filter_map(|(index, seat)| Some((index, seat.rating.as_ref()?)))
    };
    // `Less` is the better of two ratings.
    let better =
        |x: &SeatRating, y: &SeatRating| x.tier.cmp(&y.tier).then(y.score.total_cmp(&x.score));
    let watch = rated()
        .filter(|(_, rating)| lean(rating) == Ordering::Less)
        .min_by(|(a, x), (b, y)| better(x, y).then(a.cmp(b)));
    let target = rated()
        .filter(|(_, rating)| lean(rating) == Ordering::Greater)
        .max_by(|(a, x), (b, y)| better(x, y).then(b.cmp(a)));
    (
        watch.map(|(index, _)| index),
        target.map(|(index, _)| index),
    )
}

/// The in-game callout: a first line with the other team's side and winer's name, then the enemy
/// to watch and the one to go after (`pick`), each in the user's own words or the language's. It
/// talks about the other team only: the team heard about itself in champ select, and every line
/// typed is one more the player waits through. A map without sides (Arena's pairs, Swarm) has no
/// single other team and a spectator no team of their own, so nothing is said there, nor about a
/// team where nobody stands out: not even the first line.
pub fn game_lines(
    view: &GameView,
    rule: &CalloutRule,
    language: Language,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    if !view.sides || view.teams.len() != 2 {
        return Vec::new();
    }
    let Some(mine) = view
        .teams
        .iter()
        .position(|team| team.iter().any(|seat| seat.is_self))
    else {
        return Vec::new();
    };
    let (other, team) = (1 - mine, &view.teams[1 - mine]);
    let (watch, target) = pick(team);
    let own_or = |own: &str, default: &'static str| match own.trim() {
        "" => default.to_owned(),
        own => own.to_owned(),
    };
    let players: Vec<String> = [
        (
            watch,
            own_or(&rule.watch_template, watch_template(language)),
        ),
        (
            target,
            own_or(&rule.target_template, target_template(language)),
        ),
    ]
    .into_iter()
    .filter_map(|(index, template)| {
        let seat = &team[index?];
        line(
            &template,
            &seat_label(index? + 1, language),
            seat,
            seat.rating.as_ref()?,
            &champion,
        )
    })
    .collect();
    if players.is_empty() {
        return players;
    }
    let side = if other == 0 { Side::Blue } else { Side::Red };
    let first = match language {
        Language::ZhCn => format!("{}{}", enemy_tag(side, language), signature(language)),
        Language::En => format!("{} {}", enemy_tag(side, language), signature(language)),
    };
    std::iter::once(first).chain(players).collect()
}

/// What the in-game callout would type under `rule`, shown with the user's own recent form in the
/// enemy to watch (the best tier) and the one to go after (the worst), the enemy on the red side.
pub fn game_preview(
    me: &PlayerSummary,
    rule: &CalloutRule,
    general: &General,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let ranking = ranking(rule, general);
    let worst = ranking.names.len().saturating_sub(1);
    let view = GameView {
        game_id: 0,
        queue_id: 0,
        teams: vec![
            vec![sample(me, &ranking, 0, true)],
            vec![
                sample(me, &ranking, 0, false),
                sample(me, &ranking, worst, false),
            ],
        ],
        sides: true,
        callout: Vec::new(),
    };
    game_lines(&view, rule, general.language, champion)
}

// ---- The callout's shortcut ----

/// What a press of the callout's shortcut does.
#[derive(Clone, Debug, PartialEq)]
pub enum Press {
    /// Champ select: its lines go to its chat, as 发送到队伍 sends them.
    ChampSelect,
    /// The game runs and in-game sending is on: the shell types these into the game's chat.
    Game(Vec<String>),
    /// Nothing is sent.
    Skip(CalloutSkip),
}

/// What the callout's shortcut does in `phase`, from the views as drawn: in champ select the team's
/// lines go to its chat; while the game runs (`InProgress`: not its loading screen) the enemy lines
/// are typed into the game's chat, if in-game sending is on. No lines, nothing sent.
pub fn press(
    phase: Phase,
    champ_select: Option<&ChampSelectView>,
    game: Option<&GameView>,
    in_game: bool,
) -> Press {
    match phase {
        Phase::ChampSelect => match champ_select {
            Some(view) if !view.callout.is_empty() => Press::ChampSelect,
            _ => Press::Skip(CalloutSkip::NothingToSay),
        },
        Phase::InProgress if !in_game => Press::Skip(CalloutSkip::InGameOff),
        Phase::InProgress => match game {
            Some(view) if !view.callout.is_empty() => Press::Game(view.callout.clone()),
            _ => Press::Skip(CalloutSkip::NothingToSay),
        },
        _ => Press::Skip(CalloutSkip::NotNow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{PlayerSummary, Ranked, RecentForm, RiotId, TimerView};

    /// A rated seat in the three-horse set: tier 0 上等马, 1 中等马, 2 下等马.
    fn seat(name: &str, champion_id: i64, is_self: bool, rating: Option<(f64, u8)>) -> Seat {
        let summary = PlayerSummary {
            puuid: name.into(),
            name: RiotId::new(name, "1"),
            level: 30,
            icon_id: 1,
            private: false,
            ranked: Ranked::default(),
            recent: RecentForm {
                games: 20,
                wins: 11,
                kills: 6.0,
                deaths: 4.0,
                assists: 8.0,
                ..RecentForm::default()
            },
        };
        let horses = tier_names(
            &CalloutRule {
                tiers: TierSet::Horses,
                ..CalloutRule::default()
            },
            Language::ZhCn,
        );
        Seat {
            puuid: Some(name.into()),
            name: RiotId::new(name, "1"),
            champion_id,
            intent: false,
            position: None,
            spells: [0, 0],
            is_self,
            premade: None,
            stats: PlayerStats::Ready(Box::new(summary)),
            rating: rating.map(|(score, tier)| SeatRating {
                score,
                tier,
                tiers: 3,
                label: horses[usize::from(tier)].clone(),
                grade: None,
                title: None,
                quip: None,
            }),
        }
    }

    fn view(my_team: Vec<Seat>) -> ChampSelectView {
        ChampSelectView {
            game_id: 1,
            queue_id: 2400,
            timer: TimerView::default(),
            my_team,
            their_team: Vec::new(),
            my_bans: Vec::new(),
            their_bans: Vec::new(),
            bench_enabled: true,
            bench: Vec::new(),
            rerolls_remaining: 0,
            callout: Vec::new(),
            side: None,
        }
    }

    fn names(id: i64) -> Option<String> {
        (id == 1).then(|| "安妮".to_owned())
    }

    /// The players' lines, after the first line every callout opens with.
    fn players(view: &ChampSelectView, rule: &CalloutRule, language: Language) -> Vec<String> {
        let mut lines = lines(view, rule, language, names);
        assert!(
            lines
                .first()
                .is_some_and(|first| first.contains(signature(language))),
            "{lines:?}"
        );
        lines.remove(0);
        lines
    }

    #[test]
    fn lines_fill_the_template_best_tier_first_and_name_each_seat() {
        let view = view(vec![
            seat("bo", 0, false, Some((4.1, 2))),
            seat("ann", 1, true, Some((7.2, 0))),
            seat("cy", 2, false, Some((5.5, 1))),
        ]);
        assert_eq!(
            players(&view, &CalloutRule::default(), Language::ZhCn),
            [
                "上等马：2L ann，近20场胜率55%，KDA 3.5，战力7.2",
                "中等马：3L cy，近20场胜率55%，KDA 3.5，战力5.5",
                "下等马：1L bo，近20场胜率55%，KDA 3.5，战力4.1",
            ],
            "the seat is the place in champ select's list, whatever order the lines take"
        );
        // The labels were resolved in Chinese when the seats were rated; the line is English.
        assert_eq!(
            players(&view, &CalloutRule::default(), Language::En)[0],
            "上等马: P2 ann, 55% in 20 games, KDA 3.5, form 7.2"
        );
    }

    #[test]
    fn the_default_line_names_the_seat_and_the_player_not_the_champion() {
        assert_eq!(
            template(Language::ZhCn),
            "{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}"
        );
        assert_eq!(
            template(Language::En),
            "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        );
        let seats: Vec<String> = (1..=5)
            .map(|seat| seat_label(seat, Language::ZhCn))
            .collect();
        assert_eq!(seats, ["1L", "2L", "3L", "4L", "5L"]);
        let seats: Vec<String> = (1..=5).map(|seat| seat_label(seat, Language::En)).collect();
        assert_eq!(seats, ["P1", "P2", "P3", "P4", "P5"]);
    }

    #[test]
    fn a_template_of_the_users_own_can_still_name_the_champion() {
        let view = view(vec![
            seat("bo", 0, false, Some((4.1, 2))),
            seat("ann", 1, false, Some((7.2, 0))),
        ]);
        let rule = CalloutRule {
            template: "{standing}：{champion} {name} {seat}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            players(&view, &rule, Language::ZhCn),
            ["上等马：安妮 ann 2L", "下等马：bo 1L"],
            "an unknown champion leaves no double space"
        );
    }

    #[test]
    fn only_the_former_default_lines_count_as_former_defaults() {
        assert_eq!(
            former_default(
                "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
            ),
            Some(Language::ZhCn)
        );
        assert_eq!(
            former_default(
                "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
            ),
            Some(Language::En)
        );
        assert_eq!(
            former_default(
                "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
            ),
            Some(Language::ZhCn),
            "0.0.3's line, which called the form score 评分"
        );
        assert_eq!(
            former_default(
                "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
            ),
            Some(Language::En)
        );
        assert_eq!(former_default(template(Language::ZhCn)), None);
        assert_eq!(former_default(template(Language::En)), None);
        assert_eq!(former_default("{standing}：{champion} {name}"), None);
        assert_eq!(former_default(""), None);
    }

    #[test]
    fn every_tier_set_names_its_tiers_best_first() {
        let named = |tiers: TierSet, custom: &[&str], language: Language| {
            let rule = CalloutRule {
                tiers,
                custom_tiers: custom.iter().map(|name| (*name).to_owned()).collect(),
                ..CalloutRule::default()
            };
            tier_names(&rule, language)
        };
        assert_eq!(
            named(TierSet::Horses, &[], Language::ZhCn),
            ["上等马", "中等马", "下等马"]
        );
        assert_eq!(
            named(TierSet::HorsesFive, &[], Language::ZhCn),
            ["独角马", "上等马", "中等马", "下等马", "纯牛马"]
        );
        assert_eq!(named(TierSet::Rift, &[], Language::En).len(), 5);
        assert_eq!(
            named(TierSet::Custom, &[" 大腿 ", "", "挂件"], Language::ZhCn),
            ["大腿", "挂件"],
            "blanks are skipped"
        );
        assert_eq!(
            named(TierSet::Custom, &["孤品"], Language::ZhCn),
            ["上等马", "中等马", "下等马"],
            "one name cannot rank"
        );
    }

    #[test]
    fn the_users_template_wins_and_self_can_be_left_out() {
        let view = view(vec![
            seat("ann", 1, true, Some((7.2, 0))),
            seat("bo", 1, false, Some((3.0, 2))),
        ]);
        let rule = CalloutRule {
            template: "{name}={standing}@{seat} {unknown}".into(),
            include_self: false,
            ..CalloutRule::default()
        };
        // Labels are resolved when the seats are rated; the line uses the seat's own. Leaving
        // oneself out does not renumber the others.
        assert_eq!(
            players(&view, &rule, Language::ZhCn),
            vec!["bo=下等马@2L {unknown}".to_owned()]
        );
    }

    #[test]
    fn the_first_line_names_winer_and_needs_someone_to_introduce() {
        let rule = CalloutRule {
            header: "  开局分析  ".into(),
            ..CalloutRule::default()
        };
        let rated = view(vec![seat("ann", 1, false, Some((7.2, 0)))]);
        let lines = lines(&rated, &rule, Language::ZhCn, names);
        assert_eq!(
            lines.first().map(String::as_str),
            Some("winer 战绩鉴定 · 开局分析")
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(
            super::lines(&rated, &CalloutRule::default(), Language::En, names)[0],
            "winer rating",
            "the name is there without an opening line or a side"
        );
        let nobody = view(vec![seat("ann", 1, false, None)]);
        assert!(super::lines(&nobody, &rule, Language::ZhCn, names).is_empty());
    }

    #[test]
    fn the_side_leads_the_first_line_once_with_or_without_an_opening_line() {
        let mut red = view(vec![
            seat("ann", 1, false, Some((7.2, 0))),
            seat("bo", 0, false, Some((4.1, 2))),
        ]);
        red.side = Some(Side::Red);
        let opening = CalloutRule {
            header: "冲冲冲".into(),
            ..CalloutRule::default()
        };
        let lines = lines(&red, &opening, Language::ZhCn, names);
        assert_eq!(lines[0], "【红色方】winer 战绩鉴定 · 冲冲冲");
        assert_eq!(
            lines.len(),
            3,
            "one line for the side, none extra: {lines:?}"
        );
        assert!(lines[1..].iter().all(|line| !line.contains("红色方")));

        let bare = super::lines(&red, &CalloutRule::default(), Language::ZhCn, names);
        assert_eq!(
            bare[0], "【红色方】winer 战绩鉴定",
            "the side and the name without an opening line"
        );
        assert_eq!(bare.len(), 3);
        red.side = Some(Side::Blue);
        assert_eq!(
            super::lines(&red, &opening, Language::En, names)[0],
            "[Blue side] winer rating · 冲冲冲"
        );
        let nobody = view(vec![seat("ann", 1, false, None)]);
        assert!(
            super::lines(
                &ChampSelectView {
                    side: Some(Side::Red),
                    ..nobody
                },
                &opening,
                Language::ZhCn,
                names
            )
            .is_empty(),
            "nothing to say about anyone, nothing sent"
        );
    }

    #[test]
    fn a_title_follows_the_score_in_brackets_and_disappears_without_one() {
        let mut titled = seat("ann", 1, false, Some((6.0, 0)));
        if let Some(rating) = titled.rating.as_mut() {
            rating.title = Some("版本答案".into());
        }
        let view = view(vec![titled, seat("bo", 0, false, Some((4.1, 2)))]);
        let lines = players(&view, &CalloutRule::default(), Language::ZhCn);
        assert!(lines[0].ends_with("战力6.0「版本答案」"), "{lines:?}");
        assert!(
            lines[1].ends_with("战力4.1"),
            "no title, no trace: {lines:?}"
        );
    }

    #[test]
    fn a_line_leaves_no_gap_where_a_title_or_a_name_is_missing() {
        let mut quipped = seat("ann", 1, false, Some((7.2, 0)));
        if let Some(rating) = quipped.rating.as_mut() {
            rating.quip = Some("steady".into());
        }
        let mut titled = seat("bo", 0, false, Some((4.1, 2)));
        if let Some(rating) = titled.rating.as_mut() {
            rating.title = Some("Patch Champion".into());
        }
        let team = view(vec![quipped, titled]);
        assert_eq!(
            players(&team, &CalloutRule::default(), Language::En),
            [
                "上等马: P1 ann, 55% in 20 games, KDA 3.5, form 7.2, steady",
                "下等马: P2 bo, 55% in 20 games, KDA 3.5, form 4.1 [Patch Champion]",
            ],
            "no space before the quip's comma without a title, one before a title"
        );
        // 0.0.3's English line put a space before the title; a line of the user's own may too.
        let rule = CalloutRule {
            template: "{name} {score} {title}{quip}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            players(&team, &rule, Language::En),
            ["ann 7.2, steady", "bo 4.1 [Patch Champion]"]
        );

        let mut nameless = seat("cy", 0, false, Some((5.0, 1)));
        nameless.name = None;
        if let PlayerStats::Ready(summary) = &mut nameless.stats {
            summary.name = None;
        }
        assert_eq!(
            players(
                &view(vec![nameless]),
                &CalloutRule::default(),
                Language::ZhCn
            ),
            ["中等马：1L，近20场胜率55%，KDA 3.5，战力5.0"],
            "a name the client hides leaves no space before the comma"
        );
    }

    #[test]
    fn the_preview_shows_every_tier_with_the_users_own_form() {
        let Seat {
            stats: PlayerStats::Ready(me),
            ..
        } = seat("ann", 1, true, None)
        else {
            unreachable!()
        };
        let rule = CalloutRule {
            header: "开局分析".into(),
            tiers: TierSet::HorsesFive,
            include_self: false,
            ..CalloutRule::default()
        };
        let lines = preview(&me, &rule, &General::default(), names);
        assert_eq!(lines.len(), 6, "the opening line and five tiers: {lines:?}");
        assert_eq!(
            lines[0], "【蓝色方】winer 战绩鉴定 · 开局分析",
            "a side stands in for the game's"
        );
        assert!(
            lines[1].starts_with("独角马：1L ann，") && lines[5].starts_with("纯牛马：5L ann，"),
            "{lines:?}"
        );
        for (index, line) in lines[1..].iter().enumerate() {
            assert!(
                line.contains(&format!("：{}L ann，近20场", index + 1)),
                "the seats in order: {lines:?}"
            );
        }
        let english = General {
            language: Language::En,
            ..General::default()
        };
        let lines = preview(&me, &rule, &english, names);
        assert!(
            lines[1].starts_with("Unicorn: P1 ann, ")
                && lines[5].starts_with("Pack mule: P5 ann, "),
            "{lines:?}"
        );
    }

    #[test]
    fn the_default_ranks_five_players_into_five_rift_tiers_each_with_a_quip() {
        use crate::{live, model::ChampSelectSession};
        let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "gameId": 42,
            "localPlayerCellId": 0,
            "myTeam": (0..5).map(|cell| serde_json::json!({
                "cellId": cell, "championId": cell + 1, "puuid": format!("p{cell}"), "gameName": format!("P{cell}"), "tagLine": "1"
            })).collect::<Vec<_>>()
        }))
        .unwrap();
        let stats = |puuid: &str| {
            let wins = 6 + 2 * puuid[1..].parse::<u32>().unwrap();
            PlayerStats::Ready(Box::new(PlayerSummary {
                puuid: puuid.into(),
                name: RiotId::new(&puuid.to_uppercase(), "1"),
                level: 30,
                icon_id: 1,
                private: false,
                ranked: Ranked::default(),
                recent: RecentForm {
                    games: 20,
                    wins,
                    kills: 5.0,
                    deaths: 5.0,
                    assists: 5.0,
                    ..RecentForm::default()
                },
            }))
        };
        let rule = CalloutRule::default();
        assert_eq!(rule.tiers, TierSet::RiftFive, "峡谷五档 by default");
        let general = General::default();
        let view = live::champ_select_view(&session, stats, &ranking(&rule, &general), "KIWI");
        let lines = lines(&view, &rule, Language::ZhCn, |id| Some(format!("C{id}")));
        assert!(lines[0].ends_with(signature(Language::ZhCn)), "{lines:?}");
        let lines = &lines[1..];
        let heads: Vec<&str> = lines
            .iter()
            .map(|line| line.split('：').next().unwrap())
            .collect();
        assert_eq!(
            heads,
            [
                "峡谷通天代",
                "人形防御塔",
                "峡谷公务员",
                "移动眼位",
                "纯正牛马"
            ]
        );
        assert!(
            lines[0].starts_with("峡谷通天代：5L P4，近20场胜率70%"),
            "the best form sits in the fifth cell: {lines:?}"
        );
        for (tier, line) in lines.iter().enumerate() {
            let said = quips(TierSet::RiftFive, Language::ZhCn)[tier];
            assert!(
                said.iter().any(|quip| line.ends_with(&format!("，{quip}"))),
                "tier {tier} ends with one of its quips: {line}"
            );
        }
        // The same game says the same thing; other games vary it.
        assert_eq!(
            quip(TierSet::RiftFive, Language::ZhCn, 0, "p4", 42),
            quip(TierSet::RiftFive, Language::ZhCn, 0, "p4", 42)
        );
        let said: std::collections::HashSet<_> = (0..40)
            .filter_map(|game| quip(TierSet::RiftFive, Language::ZhCn, 0, "p4", game))
            .collect();
        assert!(said.len() > 1, "{said:?}");
        assert_eq!(
            quip(TierSet::Custom, Language::ZhCn, 0, "p4", 42),
            None,
            "the user's own names come bare"
        );
    }

    /// A full five-player team, from the client's session to the lines sent: every player lands in
    /// their own tier of the five horses, best form first, each line naming who it is about.
    #[test]
    fn a_five_player_team_is_ranked_into_five_tiers_end_to_end() {
        use crate::{live, model::ChampSelectSession};
        let session: ChampSelectSession = serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 2,
            "myTeam": (0..5).map(|cell| serde_json::json!({
                "cellId": cell, "championId": cell + 1, "puuid": format!("p{cell}"), "gameName": format!("P{cell}"), "tagLine": "1"
            })).collect::<Vec<_>>()
        }))
        .unwrap();
        // Wins out of twenty, KDA held equal: form follows the win rate. P3 best, P1 worst.
        let wins = |puuid: &str| match puuid {
            "p0" => 10,
            "p1" => 3,
            "p2" => 12,
            "p3" => 17,
            _ => 7,
        };
        let stats = |puuid: &str| {
            PlayerStats::Ready(Box::new(PlayerSummary {
                puuid: puuid.into(),
                name: None,
                level: 30,
                icon_id: 1,
                private: false,
                ranked: Ranked::default(),
                recent: RecentForm {
                    games: 20,
                    wins: wins(puuid),
                    kills: 5.0,
                    deaths: 5.0,
                    assists: 5.0,
                    ..RecentForm::default()
                },
            }))
        };
        let rule = CalloutRule {
            tiers: TierSet::HorsesFive,
            ..CalloutRule::default()
        };
        let general = General {
            titles: false,
            ..General::default()
        };
        let view = live::champ_select_view(&session, stats, &ranking(&rule, &general), "KIWI");
        let tiers: Vec<(String, u8)> = view
            .my_team
            .iter()
            .map(|seat| {
                (
                    seat.name.as_ref().unwrap().game_name.clone(),
                    seat.rating.as_ref().unwrap().tier,
                )
            })
            .collect();
        assert_eq!(
            tiers,
            vec![
                ("P0".into(), 2),
                ("P1".into(), 4),
                ("P2".into(), 1),
                ("P3".into(), 0),
                ("P4".into(), 3)
            ]
        );
        let champion = |id: i64| Some(format!("C{id}"));
        let lines = lines(&view, &rule, Language::ZhCn, champion);
        let lines = &lines[1..];
        let heads: Vec<&str> = lines
            .iter()
            .map(|line| line.split(' ').next().unwrap())
            .collect();
        // Seats follow the cells, 0 to 4, whatever tier each lands in.
        assert_eq!(
            heads,
            [
                "独角马：4L",
                "上等马：3L",
                "中等马：1L",
                "下等马：5L",
                "纯牛马：2L"
            ],
            "{lines:?}"
        );
        assert!(
            lines[0].starts_with("独角马：4L P3，近20场胜率85%"),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .all(|line| (1..=5).all(|id| !line.contains(&format!("C{id}")))),
            "no champion in the default line: {lines:?}"
        );
    }

    #[test]
    fn every_built_in_tier_has_something_to_say_and_the_users_own_names_nothing() {
        for language in [Language::ZhCn, Language::En] {
            for set in [
                TierSet::RiftFive,
                TierSet::Grades,
                TierSet::HorseUniverse,
                TierSet::HorsesFive,
                TierSet::Horses,
                TierSet::Rift,
            ] {
                let said = quips(set, language);
                assert_eq!(
                    said.len(),
                    preset(set, language).len(),
                    "{set:?} {language:?}"
                );
                assert!(said.iter().all(|options| !options.is_empty()), "{set:?}");
            }
            assert!(quips(TierSet::Custom, language).is_empty());
        }
    }

    #[test]
    fn ready_waits_for_every_teammate() {
        let mut loading = seat("cy", 2, false, None);
        loading.stats = PlayerStats::Loading;
        assert!(!ready(&view(vec![
            seat("ann", 1, true, Some((7.0, 1))),
            loading
        ])));
        let mut hidden = seat("cy", 2, false, None);
        hidden.stats = PlayerStats::Hidden;
        assert!(ready(&view(vec![
            seat("ann", 1, true, Some((7.0, 1))),
            hidden.clone()
        ])));
        assert!(!ready(&view(vec![hidden])), "nobody rated, nothing to say");
    }

    // ---- In the game ----

    /// A seat of the other team, rated `tier` of `tiers` with `score` (labelled `T<tier>`).
    fn enemy(name: &str, champion_id: i64, rating: Option<(u8, u8, f64)>) -> Seat {
        let mut seat = seat(name, champion_id, false, None);
        seat.rating = rating.map(|(tier, tiers, score)| SeatRating {
            score,
            tier,
            tiers,
            label: format!("T{tier}"),
            grade: None,
            title: None,
            quip: None,
        });
        seat
    }

    /// A grade of the eight, on fixed bands: the grade is the tier.
    fn graded(grade: u8, score: f64) -> Seat {
        let mut seat = enemy("g", 1, Some((grade, 8, score)));
        if let Some(rating) = seat.rating.as_mut() {
            rating.grade = Some(grade);
        }
        seat
    }

    /// A running game with the local player alone on team `mine` and `enemies` on the other.
    fn game(mine: usize, enemies: Vec<Seat>) -> GameView {
        let me = seat("me", 9, true, Some((6.0, 0)));
        GameView {
            game_id: 7,
            queue_id: 420,
            teams: if mine == 0 {
                vec![vec![me], enemies]
            } else {
                vec![enemies, vec![me]]
            },
            sides: true,
            callout: Vec::new(),
        }
    }

    fn champions(id: i64) -> Option<String> {
        Some(
            match id {
                1 => "亚索",
                2 => "盖伦",
                _ => return None,
            }
            .to_owned(),
        )
    }

    #[test]
    fn the_enemy_to_watch_rates_above_the_middle_and_the_one_to_go_after_below_it() {
        let five = [
            enemy("a", 1, Some((2, 5, 5.5))),
            enemy("b", 2, Some((4, 5, 3.1))),
            enemy("c", 3, Some((0, 5, 7.8))),
            enemy("d", 4, Some((1, 5, 6.4))),
            enemy("e", 5, Some((3, 5, 4.4))),
        ];
        assert_eq!(pick(&five), (Some(2), Some(1)), "the best and the worst");
        assert_eq!(
            pick(&[
                enemy("a", 1, Some((3, 5, 4.0))),
                enemy("b", 2, Some((1, 5, 6.0)))
            ]),
            (Some(1), Some(0)),
            "two players rank second and fourth of five"
        );
        // Three tiers split five players 2 / 1 / 2: the higher score above, the lower below.
        let horses = [
            enemy("a", 1, Some((0, 3, 6.1))),
            enemy("b", 2, Some((2, 3, 4.2))),
            enemy("c", 3, Some((0, 3, 6.9))),
            enemy("d", 4, Some((1, 3, 5.2))),
            enemy("e", 5, Some((2, 3, 3.9))),
        ];
        assert_eq!(pick(&horses), (Some(2), Some(4)));
        let tied = [
            enemy("a", 1, Some((0, 2, 6.0))),
            enemy("b", 2, Some((1, 2, 4.0))),
            enemy("c", 3, Some((0, 2, 6.0))),
            enemy("d", 4, Some((1, 2, 4.0))),
        ];
        assert_eq!(
            pick(&tied),
            (Some(0), Some(1)),
            "a tie goes to the earlier seat"
        );

        assert_eq!(pick(&[enemy("a", 1, None)]), (None, None), "nobody rated");
        assert_eq!(
            pick(&[enemy("a", 1, Some((2, 5, 5.0)))]),
            (None, None),
            "a player rated alone sits in the middle tier"
        );
    }

    #[test]
    fn of_the_eight_grades_b_and_c_are_ordinary_form_and_nobody_is_singled_out_for_them() {
        assert_eq!(
            pick(&[
                graded(3, 5.5),
                graded(2, 6.0),
                graded(4, 5.0),
                graded(5, 4.5)
            ]),
            (Some(1), Some(3)),
            "an A is watched, a D gone after"
        );
        assert_eq!(pick(&[graded(3, 5.5), graded(4, 4.9)]), (None, None));
        assert_eq!(
            pick(&[graded(1, 7.0), graded(0, 7.9), graded(2, 6.0)]),
            (Some(1), None),
            "a strong team has somebody to watch and nobody to go after"
        );
    }

    #[test]
    fn in_game_the_callout_names_the_enemy_side_and_whom_to_watch_and_to_go_after() {
        let mut strong = enemy("强者", 1, Some((0, 5, 7.8)));
        if let Some(rating) = strong.rating.as_mut() {
            rating.title = Some("版本答案".into());
        }
        let enemies = vec![
            enemy("路人", 3, Some((2, 5, 5.2))),
            strong,
            enemy("弱者", 2, Some((4, 5, 3.0))),
        ];
        assert_eq!(
            game_lines(
                &game(0, enemies.clone()),
                &CalloutRule::default(),
                Language::ZhCn,
                champions
            ),
            [
                "【敌方·红色方】winer 战绩鉴定",
                "小心 亚索 强者：T0，近20场胜率55%，KDA 3.5「版本答案」",
                "对面 盖伦 弱者：T4，近20场胜率55%，可以多抓",
            ],
            "the champion names the player in game; the middle one is not talked about"
        );
        assert_eq!(
            game_lines(
                &game(1, enemies),
                &CalloutRule::default(),
                Language::En,
                champions
            ),
            [
                "[Enemy · Blue side] winer rating",
                "Watch 亚索 (强者): T0, 55% in 20 games, KDA 3.5「版本答案」",
                "Go after 盖伦 (弱者): T4, 55% in 20 games",
            ]
        );
    }

    #[test]
    fn the_users_own_enemy_lines_win_and_nothing_is_said_without_two_sides_or_a_team() {
        let enemies = vec![
            enemy("强者", 1, Some((0, 5, 7.8))),
            enemy("弱者", 2, Some((4, 5, 3.0))),
        ];
        let rule = CalloutRule {
            watch_template: "注意{seat}{champion}，{standing}".into(),
            target_template: "  ".into(),
            ..CalloutRule::default()
        };
        let lines = game_lines(&game(0, enemies.clone()), &rule, Language::ZhCn, champions);
        assert_eq!(
            lines[1], "注意1L亚索，T0",
            "the seat is the place in their list"
        );
        assert!(
            lines[2].starts_with("对面 盖伦 弱者："),
            "a blank line is the default: {lines:?}"
        );
        assert!(
            game_lines(&game(0, enemies.clone()), &rule, Language::ZhCn, |_| None)[1]
                .starts_with("注意1L，"),
            "a champion the catalog does not name leaves no gap"
        );

        let mut sideless = game(0, enemies.clone());
        sideless.sides = false;
        assert!(
            game_lines(&sideless, &rule, Language::ZhCn, champions).is_empty(),
            "Arena's pairs have no one other team"
        );
        let mut watching = game(0, enemies);
        watching.teams[0][0].is_self = false;
        assert!(
            game_lines(&watching, &rule, Language::ZhCn, champions).is_empty(),
            "a spectator has no team of their own"
        );
        let ordinary = game(1, vec![enemy("路人", 3, Some((2, 5, 5.2)))]);
        assert!(
            game_lines(&ordinary, &rule, Language::ZhCn, champions).is_empty(),
            "nobody stands out: not even the first line"
        );
    }

    #[test]
    fn the_game_preview_types_both_lines_with_the_users_own_form() {
        let Seat {
            stats: PlayerStats::Ready(me),
            ..
        } = seat("ann", 1, true, None)
        else {
            unreachable!()
        };
        assert_eq!(
            game_preview(&me, &CalloutRule::default(), &General::default(), names),
            [
                "【敌方·红色方】winer 战绩鉴定",
                "小心 ann：峡谷通天代，近20场胜率55%，KDA 3.5",
                "对面 ann：纯正牛马，近20场胜率55%，可以多抓",
            ],
            "the best and the worst of the default five tiers"
        );
        let graded = CalloutRule {
            tiers: TierSet::Grades,
            ..CalloutRule::default()
        };
        let english = General {
            language: Language::En,
            ..General::default()
        };
        let lines = game_preview(&me, &graded, &english, names);
        assert!(
            lines[1].starts_with("Watch (ann): Rift Demigod, ")
                && lines[2].starts_with("Go after (ann): Pure Workhorse, "),
            "S+ and F of the grades: {lines:?}"
        );
    }

    #[test]
    fn the_shortcut_sends_in_champ_select_types_in_the_game_and_otherwise_says_why_not() {
        let mut select = view(vec![seat("ann", 1, false, Some((7.2, 0)))]);
        assert_eq!(
            press(Phase::ChampSelect, Some(&select), None, true),
            Press::Skip(CalloutSkip::NothingToSay),
            "no lines yet"
        );
        select.callout = vec!["line".into()];
        assert_eq!(
            press(Phase::ChampSelect, Some(&select), None, false),
            Press::ChampSelect,
            "champ select's chat has an API: in-game sending plays no part"
        );

        let mut running = game(0, Vec::new());
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), true),
            Press::Skip(CalloutSkip::NothingToSay)
        );
        running.callout = vec!["a".into(), "b".into()];
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), false),
            Press::Skip(CalloutSkip::InGameOff)
        );
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), true),
            Press::Game(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            press(Phase::InProgress, None, None, true),
            Press::Skip(CalloutSkip::NothingToSay)
        );
        for phase in [
            Phase::None,
            Phase::Lobby,
            Phase::GameStart,
            Phase::Reconnect,
            Phase::EndOfGame,
        ] {
            assert_eq!(
                press(phase, Some(&select), Some(&running), true),
                Press::Skip(CalloutSkip::NotNow),
                "{phase:?}"
            );
        }
    }
}
