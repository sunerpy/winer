//! The champ-select callout: every rated teammate's standing and recent form, one chat line each.
//! Rating the seats is `live`'s job; this module only names the standings and writes the lines.

use crate::{
    rating::{self, FormTitle},
    settings::{CalloutRule, General, Language, TierSet},
    view::{ChampSelectView, PlayerStats, PlayerSummary, Seat, SeatRating, Side, TimerView},
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
/// not the champion: champions change during champ select, seats do not.
pub fn template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
        }
        Language::En => {
            "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
        }
    }
}

/// The default line up to 0.0.2, which named the champion where the seat now stands.
const FORMER_TEMPLATES: [(Language, &str); 2] = [
    (
        Language::ZhCn,
        "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
    ),
    (
        Language::En,
        "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
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
            rating.title.as_deref().map_or_else(String::new, |title| {
                if title.is_ascii() {
                    format!("[{title}]")
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
    // A blank value (no champion yet, a hidden name) must not leave a gap in the sentence, and
    // full-width punctuation takes no space after it.
    let mut text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for mark in ["：", "，", "；"] {
        text = text.replace(&format!("{mark} "), mark);
    }
    Some(text)
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
    let names = &ranking.names;
    let score = rating::form_score(&me.recent).unwrap_or(5.0);
    let title = general
        .titles
        .then(|| rating::form_title(&me.recent))
        .flatten()
        .map(|title| title_name(title, language).to_owned());
    let champion_id = me
        .recent
        .champions
        .first()
        .map_or(0, |form| form.champion_id);
    let my_team = names
        .iter()
        .enumerate()
        .map(|(tier, label)| Seat {
            puuid: Some(me.puuid.clone()),
            name: me.name.clone(),
            champion_id,
            intent: false,
            position: None,
            spells: [0, 0],
            is_self: false,
            premade: None,
            stats: PlayerStats::Ready(Box::new(me.clone())),
            rating: Some(SeatRating {
                score,
                tier: tier as u8,
                tiers: names.len() as u8,
                label: label.clone(),
                grade: ranking.absolute.then_some(tier as u8),
                title: title.clone(),
                quip: ranking.quip(tier as u8, &me.puuid, 0),
            }),
        })
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
                "上等马：2L ann 近20场胜率55% KDA 3.5 评分7.2",
                "中等马：3L cy 近20场胜率55% KDA 3.5 评分5.5",
                "下等马：1L bo 近20场胜率55% KDA 3.5 评分4.1",
            ],
            "the seat is the place in champ select's list, whatever order the lines take"
        );
        // The labels were resolved in Chinese when the seats were rated; the line is English.
        assert_eq!(
            players(&view, &CalloutRule::default(), Language::En)[0],
            "上等马: P2 ann, 55% in 20 games, KDA 3.5, score 7.2"
        );
    }

    #[test]
    fn the_default_line_names_the_seat_and_the_player_not_the_champion() {
        assert_eq!(
            template(Language::ZhCn),
            "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
        );
        assert_eq!(
            template(Language::En),
            "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
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
        assert_eq!(former_default(template(Language::ZhCn)), None);
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
        assert!(lines[0].ends_with("评分6.0「版本答案」"), "{lines:?}");
        assert!(
            lines[1].ends_with("评分4.1"),
            "no title, no trace: {lines:?}"
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
            lines[1].starts_with("独角马：1L ann ") && lines[5].starts_with("纯牛马：5L ann "),
            "{lines:?}"
        );
        for (index, line) in lines[1..].iter().enumerate() {
            assert!(
                line.contains(&format!("：{}L ann 近20场", index + 1)),
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
            lines[0].starts_with("峡谷通天代：5L P4 近20场胜率70%"),
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
            lines[0].starts_with("独角马：4L P3 近20场胜率85%"),
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
}
