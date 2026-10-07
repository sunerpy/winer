//! The callout: in champ select every rated teammate's standing and recent form, one chat line
//! each; in the game, the enemy to watch and the one to go after, and the team's lines again, every
//! player named by their champion. Rating the seats is `live`'s job; this module only names the
//! standings and writes the lines.

use std::cmp::Ordering;

use crate::{
    rating::{self, FormTitle},
    settings::{CalloutRule, CalloutStyle, GameTeams, General, Language, TierSet},
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

/// The line written for each player when the user has not written their own, in `style`. It names
/// the seat, not the champion: champions change during champ select, seats do not. The score is
/// called what the window calls it, 战力 (the form score), not 评分, which is a game's.
///
/// - Compact uses a seat-first scan order: seat, tier, win rate, KDA and strength.
/// - Rich adds the tier's emoji and the sample size, but keeps those columns in the same order.
///
/// Chinese defaults omit free-form player names, titles and quips. The seat already identifies a
/// player in champ select; removing the adjacent name avoids the client's filter joining a tier
/// ending in 马 with a name beginning in 会. A custom template may still use every placeholder.
pub fn template(style: CalloutStyle, language: Language) -> &'static str {
    match (style, language) {
        (CalloutStyle::Compact, Language::ZhCn) => {
            "{seat}: {standing}|胜率{winRate}|KDA{kda}|战力{score}"
        }
        (CalloutStyle::Compact, Language::En) => {
            "{seat} {standing} | {winRate} | KDA {kda} | form {score} | {name}"
        }
        (CalloutStyle::Rich, Language::ZhCn) => {
            "{seat}: {emoji}{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}"
        }
        (CalloutStyle::Rich, Language::En) => {
            "{emoji}{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        }
    }
}

/// The default lines of earlier versions, each with the style whose default it was (those from
/// before there were styles count as the rich style's): up to 0.0.2 they named the champion where
/// the seat now stands, up to 0.0.3 they called the form score 评分 and ran the name into the
/// numbers, up to 0.0.4 the Chinese lines left the name bare, and up to 0.0.7 they included
/// free-form names, titles and quips in the default team message.
const FORMER_TEMPLATES: [((CalloutStyle, Language), &str); 10] = [
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
    ),
    (
        (CalloutStyle::Rich, Language::En),
        "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}",
    ),
    (
        (CalloutStyle::Rich, Language::En),
        "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    ),
    (
        (CalloutStyle::Rich, Language::En),
        "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}",
    ),
    (
        (CalloutStyle::Compact, Language::ZhCn),
        "{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜{name}",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{emoji}{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    ),
    (
        (CalloutStyle::Compact, Language::ZhCn),
        "{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜【{name}】",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{emoji}{standing}：{seat}【{name}】，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    ),
];

/// The style and language whose former default line `template` is, character for character.
pub(crate) fn former_default(template: &str) -> Option<(CalloutStyle, Language)> {
    former(&FORMER_TEMPLATES, template)
}

/// What `template` is one of `formers` for (a language, or a style and a language), character for
/// character.
fn former<K: Copy>(formers: &[(K, &str)], template: &str) -> Option<K> {
    formers
        .iter()
        .find(|(_, former)| *former == template)
        .map(|(key, _)| *key)
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
    use FormTitle::*;
    let [zh, en] = match title {
        OnAStreak => ["版本答案", "Patch Champion"],
        Immortal => ["峡谷永生者", "Rift Immortal"],
        Reaper => ["人头收割机", "Kill Collector"],
        Playmaker => ["团战发动机", "Teamfight Engine"],
        Winner => ["常胜将军", "Serial Winner"],
        RockSolid => ["定海神针", "Rock Solid"],
        Reliable => ["靠谱队友", "Reliable Teammate"],
        Trader => ["一换一专业户", "One-for-one Trader"],
        Helper => ["峡谷慈善家", "Rift Philanthropist"],
        SlotMachine => ["峡谷老虎机", "Slot Machine"],
        Steady => ["正常发挥", "Business as Usual"],
        GivingAway => ["排位慈善家", "Ranked Philanthropist"],
        Bodhisattva => ["电竞菩萨", "Esports Bodhisattva"],
        GreyScreen => ["黑白电视机资深会员", "Grey-screen Regular"],
        Spectator => ["团战观众", "Teamfight Spectator"],
        Tourist => ["峡谷观光客", "Rift Tourist"],
        AlongForTheRide => ["陪跑选手", "Along for the Ride"],
    };
    match language {
        Language::ZhCn => zh,
        Language::En => en,
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
    let template = own_or(&rule.template, template(rule.style, language));
    let players = rated_lines(
        &view.my_team,
        rule.include_self,
        template,
        language,
        &champion,
        true,
    );
    if players.is_empty() {
        return players;
    }
    // The side leads, then winer's name, which every callout carries, then the opening line; the
    // rich style puts a megaphone before them.
    let mut first = first_line(view.side.map(|side| side_tag(side, language)), language);
    if rule.style == CalloutStyle::Rich {
        // The bracket of a Chinese side tag needs no space after it.
        let gap = if first.starts_with('【') { "" } else { " " };
        first.insert_str(0, &format!("{MEGAPHONE}{gap}"));
    }
    let header = rule.header.trim();
    if !header.is_empty() {
        first.push_str(&opening(header, language));
    }
    std::iter::once(first).chain(players).collect()
}

/// The opening line as the first line carries it after winer's name: in English after a dot; in
/// Chinese in 【】, so that the chat's filter does not read the end of 战绩鉴定 and the start of the
/// opening line as one word (`template`), unless it opens with a bracket of its own.
fn opening(header: &str, language: Language) -> String {
    match language {
        Language::ZhCn if header.starts_with('【') => header.to_owned(),
        Language::ZhCn => format!("【{header}】"),
        Language::En => format!(" · {header}"),
    }
}

/// The user's own template, or `default` where they left it blank.
fn own_or<'a>(own: &'a str, default: &'a str) -> &'a str {
    match own.trim() {
        "" => default,
        own => own,
    }
}

/// A callout's first line: the side's tag, if any, then winer's name, which every callout carries.
fn first_line(tag: Option<&str>, language: Language) -> String {
    match (tag, language) {
        (Some(tag), Language::ZhCn) => format!("{tag}{}", signature(language)),
        (Some(tag), Language::En) => format!("{tag} {}", signature(language)),
        (None, _) => signature(language).to_owned(),
    }
}

/// One line per rated player of `team` under `template`, the best standing first and by score
/// within a standing; the local player only with `include_self`. A player's seat is their place in
/// `team`, counted from 1 before anyone is left out, so it stays the one the client shows.
fn rated_lines(
    team: &[Seat],
    include_self: bool,
    template: &str,
    language: Language,
    champion: &impl Fn(i64) -> Option<String>,
    emoji: bool,
) -> Vec<String> {
    // Keep the client's team order. The tier describes each seat; it must not rearrange 1L–5L.
    let rated: Vec<(usize, &Seat, &SeatRating)> = team
        .iter()
        .enumerate()
        .filter(|(_, seat)| include_self || !seat.is_self)
        .filter_map(|(index, seat)| Some((index + 1, seat, seat.rating.as_ref()?)))
        .collect();
    rated
        .into_iter()
        .filter_map(|(number, seat, rating)| {
            line(
                template,
                &seat_label(number, language),
                seat,
                rating,
                champion,
                emoji,
            )
        })
        .collect()
}

/// Before the rich style's first line in champ select.
const MEGAPHONE: &str = "📢";

/// The emoji before a tier in the rich style, by where it stands in its scheme: the best tier is
/// crowned, the others above the middle burn, the middle is fine, below it sweats and the worst is
/// done for. Only the client's chat shows emoji; the game's does not, so no in-game line has one.
pub fn tier_emoji(rating: &SeatRating) -> &'static str {
    let worst = rating.tiers.saturating_sub(1);
    match rating.grade {
        Some(0) => "👑",
        Some(7) => "💀",
        Some(_) => match rating::lean(rating.tier, rating.tiers, rating.grade) {
            rating::Lean::Above => "🔥",
            rating::Lean::Middle => "👌",
            rating::Lean::Below => "😅",
        },
        None if rating.tier == 0 => "👑",
        None if rating.tier == worst => "💀",
        None => match rating::lean(rating.tier, rating.tiers, None) {
            rating::Lean::Above => "🔥",
            rating::Lean::Middle => "👌",
            rating::Lean::Below => "😅",
        },
    }
}

fn line(
    template: &str,
    seat_label: &str,
    seat: &Seat,
    rating: &SeatRating,
    champion: &impl Fn(i64) -> Option<String>,
    emoji: bool,
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
    // A seat without a champion the catalog names (none picked yet, the catalog not loaded) goes
    // by the player's name instead, unless the line names the player anyway: whoever a line is
    // about, it says so.
    let champion = champion(seat.champion_id)
        .filter(|champion| !champion.is_empty())
        .or_else(|| (!template.contains("{name}")).then(|| name.clone()))
        .unwrap_or_default();
    let values = [
        (
            "{emoji}",
            if emoji {
                format!("{} ", tier_emoji(rating))
            } else {
                String::new()
            },
        ),
        ("{standing}", rating.label.clone()),
        ("{seat}", seat_label.to_owned()),
        ("{champion}", champion),
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
            // An English title brings its own space, so a line without one has no gap to leave. A
            // Chinese one sits in 【】, which keep it apart from the quip after it for the chat's
            // filter (`template`).
            rating.title.as_deref().map_or_else(String::new, |title| {
                if title.is_ascii() {
                    format!(" [{title}]")
                } else {
                    format!("【{title}】")
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
        // A blank value is marked, so that only brackets it emptied go: a pair the user wrote
        // empty in the template is the user's to keep.
        let value = if value.is_empty() {
            BLANK.to_string()
        } else {
            value
        };
        text = text.replace(key, &value);
    }
    // A blank value (no champion yet, a hidden name) must not leave a gap in the sentence: the
    // brackets it sat in go with it, full-width punctuation takes no space on either side, and a
    // comma or a colon between words none before it.
    let mut text = without_emptied_brackets(&text)
        .replace(BLANK, "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for mark in ["：", "，", "；"] {
        text = text
            .replace(&format!(" {mark}"), mark)
            .replace(&format!("{mark} "), mark);
    }
    let text = text.replace(" ,", ",").replace(" : ", ": ");
    // Nor a column separator with nothing after it, where the compact line's name is hidden.
    Some(text.trim_matches(['｜', '|', ' ']).to_owned())
}

/// The brackets a line may hold a value in, opening and closing.
const BRACKETS: [(char, char); 5] = [
    ('【', '】'),
    ('「', '」'),
    ('[', ']'),
    ('(', ')'),
    ('（', '）'),
];

/// Stands in for a value left blank until the brackets it emptied are taken away; a character of
/// Unicode's private use area, which no template or value holds.
const BLANK: char = '\u{E000}';

/// `text` without the pairs of brackets a blank value emptied: holding nothing but spaces and at
/// least one [`BLANK`]. A pair the template itself left empty holds no mark and stays.
fn without_emptied_brackets(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    for character in text.chars() {
        let Some(&(open, _)) = BRACKETS.iter().find(|(_, close)| *close == character) else {
            kept.push(character);
            continue;
        };
        // A closing bracket whose opening one comes before it with only spaces and blanks between,
        // one blank at least, takes it away and leaves a blank in its place, so that an emptied
        // pair inside another empties that one in turn; any other is kept.
        let inside = kept.trim_end_matches([' ', BLANK]);
        let emptied = kept[inside.len()..].contains(BLANK);
        match inside.strip_suffix(open).map(str::len) {
            Some(before) if emptied => {
                kept.truncate(before);
                kept.push(BLANK);
            }
            _ => kept.push(character),
        }
    }
    kept
}

/// A seat holding the user's own recent form in `tier` of `ranking`, for the previews: their most
/// played champion stands in for `{champion}`.
fn sample(me: &PlayerSummary, ranking: &Ranking, tier: usize, is_self: bool) -> Seat {
    let (count, grade) = (
        ranking.names.len() as u8,
        ranking.absolute.then_some(tier as u8),
    );
    let title = ranking
        .titles
        .then(|| rating::form_title(&me.recent, rating::lean(tier as u8, count, grade)))
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
        premade_inferred: false,
        autofilled: false,
        note: None,
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
        recommendations: Vec::new(),
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

// ---- In the game: both teams by champion, typed into the game's chat by the callout's shortcut ----

/// The other team's side, as the in-game callout's first line names it.
pub fn enemy_tag(side: Side, language: Language) -> &'static str {
    match (side, language) {
        (Side::Blue, Language::ZhCn) => "【敌方·蓝色方】",
        (Side::Red, Language::ZhCn) => "【敌方·红色方】",
        (Side::Blue, Language::En) => "[Enemy · Blue side]",
        (Side::Red, Language::En) => "[Enemy · Red side]",
    }
}

/// The local team's side, as the first of its in-game lines names it.
pub fn ally_tag(side: Side, language: Language) -> &'static str {
    match (side, language) {
        (Side::Blue, Language::ZhCn) => "【我方·蓝色方】",
        (Side::Red, Language::ZhCn) => "【我方·红色方】",
        (Side::Blue, Language::En) => "[My team · Blue side]",
        (Side::Red, Language::En) => "[My team · Red side]",
    }
}

/// The line about the enemy to watch, when the user has not written their own. In the game a
/// player goes by their champion: it no longer changes, and it is what the map and the scoreboard
/// show, so the line names the champion alone (`line` puts the player's name where none is known).
/// In Chinese the champion sits in 【】, as names do in champ select (`template`).
pub fn watch_template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "小心【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}",
        Language::En => {
            "Watch {champion}: {standing}, {winRate} in {games} games, KDA {kda}{title}"
        }
    }
}

/// The line about the enemy to go after, when the user has not written their own.
pub fn target_template(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "对面【{champion}】|档位{standing}|近{games}场胜率{winRate}|可以多抓",
        Language::En => "Go after {champion}: {standing}, {winRate} in {games} games",
    }
}

/// The line about each teammate in the game, when the user has not written their own, in `style`:
/// champion, tier and the same seat-first data columns. The game's chat shows no emoji; titles and
/// free-form quips stay out of the safe default. A custom template may still include them.
pub fn ally_template(style: CalloutStyle, language: Language) -> &'static str {
    match (style, language) {
        (CalloutStyle::Compact, Language::ZhCn) => {
            "【{champion}】|档位{standing}|胜率{winRate}|KDA{kda}|战力{score}"
        }
        (CalloutStyle::Compact, Language::En) => {
            "{standing} [{champion}] | {winRate} | KDA {kda} | form {score}"
        }
        (CalloutStyle::Rich, Language::ZhCn) => {
            "【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}"
        }
        (CalloutStyle::Rich, Language::En) => {
            "{standing}: {champion}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        }
    }
}

/// The enemy lines' former defaults: at first they named the champion and the player, and up to
/// 0.0.4 the Chinese ones left the champion bare; up to 0.0.7 they included free-form titles.
const FORMER_WATCH_TEMPLATES: [(Language, &str); 4] = [
    (
        Language::ZhCn,
        "小心 {champion} {name}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}",
    ),
    (
        Language::En,
        "Watch {champion} ({name}): {standing}, {winRate} in {games} games, KDA {kda}{title}",
    ),
    (
        Language::ZhCn,
        "小心 {champion}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}",
    ),
    (
        Language::ZhCn,
        "小心【{champion}】：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}",
    ),
];
const FORMER_TARGET_TEMPLATES: [(Language, &str); 4] = [
    (
        Language::ZhCn,
        "对面 {champion} {name}：{standing}，近{games}场胜率{winRate}，可以多抓",
    ),
    (
        Language::En,
        "Go after {champion} ({name}): {standing}, {winRate} in {games} games",
    ),
    (
        Language::ZhCn,
        "对面 {champion}：{standing}，近{games}场胜率{winRate}，可以多抓",
    ),
    (
        Language::ZhCn,
        "对面【{champion}】：{standing}，近{games}场胜率{winRate}，可以多抓",
    ),
];
/// Former team lines: 0.0.4 ran the champion into the tier with a space or colon; up to 0.0.7
/// they included free-form titles and quips.
const FORMER_ALLY_TEMPLATES: [((CalloutStyle, Language), &str); 5] = [
    (
        (CalloutStyle::Compact, Language::ZhCn),
        "{standing} {champion}｜胜率{winRate}｜KDA {kda}｜战力{score}",
    ),
    (
        (CalloutStyle::Compact, Language::En),
        "{standing} {champion} | {winRate} | KDA {kda} | form {score}",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{standing}：{champion}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    ),
    (
        (CalloutStyle::Compact, Language::ZhCn),
        "{standing}【{champion}】｜胜率{winRate}｜KDA {kda}｜战力{score}",
    ),
    (
        (CalloutStyle::Rich, Language::ZhCn),
        "{standing}【{champion}】，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}",
    ),
];

/// The language whose former default line about the enemy to watch `template` is.
pub(crate) fn former_watch_default(template: &str) -> Option<Language> {
    former(&FORMER_WATCH_TEMPLATES, template)
}

/// The language whose former default line about the enemy to go after `template` is.
pub(crate) fn former_target_default(template: &str) -> Option<Language> {
    former(&FORMER_TARGET_TEMPLATES, template)
}

/// The style and language whose former default line about each teammate in the game `template`
/// is.
pub(crate) fn former_ally_default(template: &str) -> Option<(CalloutStyle, Language)> {
    former(&FORMER_ALLY_TEMPLATES, template)
}

/// The local player's team in `view`, as its place in `view.teams`. A map without sides (Arena's
/// pairs, Swarm) has no single other team, and a spectator no team of their own: `None` for both.
fn my_team(view: &GameView) -> Option<usize> {
    if !view.sides || view.teams.len() != 2 {
        return None;
    }
    view.teams
        .iter()
        .position(|team| team.iter().any(|seat| seat.is_self))
}

/// The side of the team at `index` of a game's two: the client lists blue first.
fn side_of(index: usize) -> Side {
    if index == 0 { Side::Blue } else { Side::Red }
}

/// Where a rating stands against the middle of its scheme: above it (`Less`: tier 0 is the best),
/// below it (`Greater`) or at it. A ranking splits around its middle tier (of five, the first two
/// are above and the last two below); of the eight grades, B and C, the bands either side of an
/// ordinary player's form, are the middle.
fn lean(rating: &SeatRating) -> Ordering {
    match rating::lean(rating.tier, rating.tiers, rating.grade) {
        rating::Lean::Above => Ordering::Less,
        rating::Lean::Middle => Ordering::Equal,
        rating::Lean::Below => Ordering::Greater,
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

/// The in-game callout's enemy lines: a first line with the other team's side and winer's name,
/// then the enemy to watch and the one to go after (`pick`), each in the user's own words or the
/// language's. A map without sides and a spectator get nothing (`my_team`), nor does a team where
/// nobody stands out: not even the first line.
pub fn game_lines(
    view: &GameView,
    rule: &CalloutRule,
    language: Language,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let Some(mine) = my_team(view) else {
        return Vec::new();
    };
    let (other, team) = (1 - mine, &view.teams[1 - mine]);
    let (watch, target) = pick(team);
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
            template,
            &seat_label(index? + 1, language),
            seat,
            seat.rating.as_ref()?,
            &champion,
            false,
        )
    })
    .collect();
    if players.is_empty() {
        return players;
    }
    let first = first_line(Some(enemy_tag(side_of(other), language)), language);
    std::iter::once(first).chain(players).collect()
}

/// The team's own lines in the game: a first line with the team's side and winer's name, then one
/// line per rated teammate in the client's team order (`lines`: 1L through 5L, oneself only with
/// `include_self`), each naming the champion (`ally_template`) where champ select names the seat
/// and the player: in the game the team knows its players by champion, and champions no longer
/// change. Nothing where `game_lines` has no teams to tell apart, nor while nobody is rated.
pub fn ally_lines(
    view: &GameView,
    rule: &CalloutRule,
    language: Language,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let Some(mine) = my_team(view) else {
        return Vec::new();
    };
    let template = own_or(&rule.ally_template, ally_template(rule.style, language));
    // The game's chat shows no emoji.
    let players = rated_lines(
        &view.teams[mine],
        rule.include_self,
        template,
        language,
        &champion,
        false,
    );
    if players.is_empty() {
        return players;
    }
    let first = first_line(Some(ally_tag(side_of(mine), language)), language);
    std::iter::once(first).chain(players).collect()
}

/// The most lines one press of the shortcut types into the game. Each holds the player's keyboard
/// for about a second (`game_chat`'s pauses), so a press types a team's first line and five
/// players at most: the team's own lines fit whole, both teams together are cut.
pub const GAME_LINE_LIMIT: usize = 6;

/// What one press of the shortcut types in the game under `teams`: the enemy lines
/// (`GameView::callout`), the team's (`GameView::ally_callout`) or both, the enemy's first, cut at
/// [`GAME_LINE_LIMIT`]. The enemy's three lines at most leave the team its first line and its best
/// two players.
pub fn typed(view: &GameView, teams: GameTeams) -> Vec<String> {
    let none: &[String] = &[];
    let (enemy, allies) = match teams {
        GameTeams::Enemies => (view.callout.as_slice(), none),
        GameTeams::Allies => (none, view.ally_callout.as_slice()),
        GameTeams::Both => (view.callout.as_slice(), view.ally_callout.as_slice()),
    };
    enemy
        .iter()
        .chain(allies)
        .take(GAME_LINE_LIMIT)
        .cloned()
        .collect()
}

/// What one press of the shortcut would type in the game under `rule`, shown with the user's own
/// recent form: on the red side the enemy to watch (the best tier) and the one to go after (the
/// worst), on the blue side a team holding the user in every tier, as champ select's preview does.
/// Their most played champion stands in for every champion.
pub fn game_preview(
    me: &PlayerSummary,
    rule: &CalloutRule,
    general: &General,
    champion: impl Fn(i64) -> Option<String>,
) -> Vec<String> {
    let ranking = ranking(rule, general);
    let worst = ranking.names.len().saturating_sub(1);
    let rule = CalloutRule {
        include_self: true,
        ..rule.clone()
    };
    let mut view = GameView {
        game_id: 0,
        queue_id: 0,
        teams: vec![
            (0..ranking.names.len())
                .map(|tier| sample(me, &ranking, tier, tier == 0))
                .collect(),
            vec![
                sample(me, &ranking, 0, false),
                sample(me, &ranking, worst, false),
            ],
        ],
        sides: true,
        callout: Vec::new(),
        ally_callout: Vec::new(),
    };
    view.callout = game_lines(&view, &rule, general.language, &champion);
    view.ally_callout = ally_lines(&view, &rule, general.language, &champion);
    typed(&view, rule.game_teams)
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

/// What the callout's shortcut does in `phase`, from the views as drawn and the callout's settings:
/// in champ select the team's lines go to its chat; while the game runs (`InProgress`: not its
/// loading screen) the lines `rule.game_teams` chooses are typed into the game's chat (`typed`), if
/// in-game sending is on. No lines, nothing sent.
pub fn press(
    phase: Phase,
    champ_select: Option<&ChampSelectView>,
    game: Option<&GameView>,
    rule: &CalloutRule,
) -> Press {
    match phase {
        Phase::ChampSelect => match champ_select {
            Some(view) if !view.callout.is_empty() => Press::ChampSelect,
            _ => Press::Skip(CalloutSkip::NothingToSay),
        },
        Phase::InProgress if !rule.in_game => Press::Skip(CalloutSkip::InGameOff),
        Phase::InProgress => match game.map(|view| typed(view, rule.game_teams)) {
            Some(lines) if !lines.is_empty() => Press::Game(lines),
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
            premade_inferred: false,
            autofilled: false,
            note: None,
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
            recommendations: Vec::new(),
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
    fn lines_follow_champ_select_seat_order_whatever_each_players_tier() {
        let view = view(vec![
            seat("bo", 0, false, Some((4.1, 2))),
            seat("ann", 1, true, Some((7.2, 0))),
            seat("cy", 2, false, Some((5.5, 1))),
        ]);
        assert_eq!(
            players(&view, &CalloutRule::default(), Language::ZhCn),
            [
                "1L: 💀 下等马|近20场胜率55%|KDA3.5|战力4.1",
                "2L: 👑 上等马|近20场胜率55%|KDA3.5|战力7.2",
                "3L: 👌 中等马|近20场胜率55%|KDA3.5|战力5.5",
            ],
            "the lines stay in the same 1L-to-5L order as champ select"
        );
        // The labels were resolved in Chinese when the seats were rated; the line is English.
        assert_eq!(
            players(&view, &CalloutRule::default(), Language::En)[0],
            "💀 下等马: P1 bo, 55% in 20 games, KDA 3.5, form 4.1"
        );
    }

    #[test]
    fn the_compact_style_writes_one_short_line_a_player_in_the_same_columns() {
        let mut team = view(vec![
            seat("bo", 0, false, Some((4.1, 2))),
            seat("ann", 1, true, Some((7.2, 0))),
            seat("cy", 2, false, Some((5.5, 1))),
        ]);
        team.side = Some(Side::Red);
        let compact = CalloutRule {
            style: CalloutStyle::Compact,
            ..CalloutRule::default()
        };
        assert_eq!(
            lines(&team, &compact, Language::ZhCn, names),
            [
                "【红色方】winer 战绩鉴定",
                "1L: 下等马|胜率55%|KDA3.5|战力4.1",
                "2L: 上等马|胜率55%|KDA3.5|战力7.2",
                "3L: 中等马|胜率55%|KDA3.5|战力5.5",
            ],
            "no emoji, title or quip: the same fields in the same order"
        );
        assert_eq!(
            players(&team, &compact, Language::En)[0],
            "P1 下等马 | 55% | KDA 3.5 | form 4.1 | bo"
        );
        // A name the client hides takes its brackets with it and leaves no separator hanging.
        let mut hidden = seat("dee", 3, false, Some((5.0, 1)));
        hidden.name = None;
        if let PlayerStats::Ready(summary) = &mut hidden.stats {
            summary.name = None;
        }
        assert_eq!(
            players(&view(vec![hidden]), &compact, Language::ZhCn),
            ["1L: 中等马|胜率55%|KDA3.5|战力5.0"]
        );
        // A template of one's own takes {emoji} in either style.
        let own = CalloutRule {
            template: "{emoji}{seat} {standing}".into(),
            ..compact
        };
        assert_eq!(players(&team, &own, Language::ZhCn)[0], "💀 1L 下等马");
    }

    #[test]
    fn a_tiers_emoji_follows_where_it_stands_in_its_scheme() {
        let emoji = |tier: u8, tiers: u8, grade: Option<u8>| {
            tier_emoji(&SeatRating {
                score: 5.0,
                tier,
                tiers,
                label: String::new(),
                grade,
                title: None,
                quip: None,
            })
        };
        let five: Vec<&str> = (0..5).map(|tier| emoji(tier, 5, None)).collect();
        assert_eq!(five, ["👑", "🔥", "👌", "😅", "💀"]);
        let three: Vec<&str> = (0..3).map(|tier| emoji(tier, 3, None)).collect();
        assert_eq!(three, ["👑", "👌", "💀"]);
        let grades: Vec<&str> = (0..8).map(|grade| emoji(grade, 8, Some(grade))).collect();
        assert_eq!(grades, ["👑", "🔥", "🔥", "👌", "👌", "😅", "😅", "💀"]);
    }

    #[test]
    fn the_default_line_uses_a_seat_and_safe_columns_in_chinese() {
        assert_eq!(
            template(CalloutStyle::Rich, Language::ZhCn),
            "{seat}: {emoji}{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}"
        );
        assert_eq!(
            template(CalloutStyle::Rich, Language::En),
            "{emoji}{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        );
        assert_eq!(
            template(CalloutStyle::Compact, Language::ZhCn),
            "{seat}: {standing}|胜率{winRate}|KDA{kda}|战力{score}"
        );
        assert_eq!(
            template(CalloutStyle::Compact, Language::En),
            "{seat} {standing} | {winRate} | KDA {kda} | form {score} | {name}"
        );
        for style in [CalloutStyle::Compact, CalloutStyle::Rich] {
            let chinese = template(style, Language::ZhCn);
            assert!(
                chinese.contains("{seat}")
                    && chinese.contains("{standing}")
                    && !chinese.contains("{name}")
                    && !chinese.contains("{champion}")
                    && !chinese.contains("{title}")
                    && !chinese.contains("{quip}"),
                "the team-safe Chinese default omits free text: {chinese}"
            );
            let english = template(style, Language::En);
            assert!(
                english.contains("{seat}")
                    && english.contains("{name}")
                    && !english.contains("{champion}"),
                "{english}"
            );
        }
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
            ["下等马：bo 1L", "上等马：安妮 ann 2L"],
            "seat order is kept and an unknown champion leaves no double space"
        );
    }

    #[test]
    fn only_the_former_default_lines_count_as_former_defaults() {
        let rich = |language| Some((CalloutStyle::Rich, language));
        assert_eq!(
            former_default(
                "{standing}：{champion} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
            ),
            rich(Language::ZhCn)
        );
        assert_eq!(
            former_default(
                "{standing}: {champion} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
            ),
            rich(Language::En)
        );
        assert_eq!(
            former_default(
                "{standing}：{seat} {name} 近{games}场胜率{winRate} KDA {kda} 评分{score}{title}{quip}"
            ),
            rich(Language::ZhCn),
            "0.0.3's line, which called the form score 评分"
        );
        assert_eq!(
            former_default(
                "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, score {score} {title}{quip}"
            ),
            rich(Language::En)
        );
        assert_eq!(
            former_default(
                "{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}"
            ),
            rich(Language::ZhCn),
            "the line before styles and emoji, the rich style's since"
        );
        assert_eq!(
            former_default(
                "{standing}: {seat} {name}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
            ),
            rich(Language::En)
        );
        // 0.0.4's Chinese lines, each its own style's, with the name bare.
        assert_eq!(
            former_default(
                "{emoji}{standing}：{seat} {name}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}"
            ),
            rich(Language::ZhCn)
        );
        assert_eq!(
            former_default("{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜{name}"),
            Some((CalloutStyle::Compact, Language::ZhCn))
        );
        for style in [CalloutStyle::Compact, CalloutStyle::Rich] {
            assert_eq!(former_default(template(style, Language::ZhCn)), None);
            assert_eq!(former_default(template(style, Language::En)), None);
        }
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
            Some("📢 winer 战绩鉴定【开局分析】")
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(
            super::lines(&rated, &CalloutRule::default(), Language::En, names)[0],
            "📢 winer rating",
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
        assert_eq!(lines[0], "📢【红色方】winer 战绩鉴定【冲冲冲】");
        assert_eq!(
            lines.len(),
            3,
            "one line for the side, none extra: {lines:?}"
        );
        assert!(lines[1..].iter().all(|line| !line.contains("红色方")));

        let bare = super::lines(&red, &CalloutRule::default(), Language::ZhCn, names);
        assert_eq!(
            bare[0], "📢【红色方】winer 战绩鉴定",
            "the side and the name without an opening line"
        );
        assert_eq!(bare.len(), 3);
        red.side = Some(Side::Blue);
        assert_eq!(
            super::lines(&red, &opening, Language::En, names)[0],
            "📢 [Blue side] winer rating · 冲冲冲"
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
    fn the_safe_chinese_default_omits_free_form_titles() {
        let mut titled = seat("ann", 1, false, Some((6.0, 0)));
        if let Some(rating) = titled.rating.as_mut() {
            rating.title = Some("版本答案".into());
        }
        let view = view(vec![titled, seat("bo", 0, false, Some((4.1, 2)))]);
        let lines = players(&view, &CalloutRule::default(), Language::ZhCn);
        assert_eq!(
            lines,
            [
                "1L: 👑 上等马|近20场胜率55%|KDA3.5|战力6.0",
                "2L: 💀 下等马|近20场胜率55%|KDA3.5|战力4.1",
            ],
            "the safe default leaves free-form titles out"
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
                "👑 上等马: P1 ann, 55% in 20 games, KDA 3.5, form 7.2, steady",
                "💀 下等马: P2 bo, 55% in 20 games, KDA 3.5, form 4.1 [Patch Champion]",
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
            ["1L: 👌 中等马|近20场胜率55%|KDA3.5|战力5.0"],
            "a name the client hides takes its brackets with it and leaves no space before the comma"
        );
    }

    /// `seat` with its name hidden, as the client hides some.
    fn nameless(mut seat: Seat) -> Seat {
        seat.name = None;
        if let PlayerStats::Ready(summary) = &mut seat.stats {
            summary.name = None;
        }
        seat
    }

    #[test]
    fn brackets_a_blank_value_leaves_empty_go_with_it() {
        let emptied = |text: &str| without_emptied_brackets(text).replace(BLANK, "");
        assert_eq!(emptied("1L【\u{E000}】，近20场"), "1L，近20场");
        assert_eq!(emptied("上等马【 \u{E000} 】｜胜率"), "上等马｜胜率");
        assert_eq!(emptied("Top horse [\u{E000}] | 55%"), "Top horse  | 55%");
        assert_eq!(emptied("Watch 亚索 (\u{E000}): T0"), "Watch 亚索 : T0");
        assert_eq!(
            emptied("「\u{E000}」（\u{E000}）【【\u{E000}】】"),
            "",
            "an emptied pair inside another empties that one too"
        );
        assert_eq!(
            emptied("【蓝色方】1L【ann】：】x【"),
            "【蓝色方】1L【ann】：】x【",
            "a pair with something in it stays, and so does a bracket without its other half"
        );
        assert_eq!(
            emptied("留着【】和( )【【】】"),
            "留着【】和( )【【】】",
            "pairs the template wrote empty are the user's"
        );
        // Through a line of the user's own: a hidden name, and no champion to stand in for it.
        let rule = CalloutRule {
            template: "{standing}【{name}】({champion}) {seat}".into(),
            ..CalloutRule::default()
        };
        let lone = nameless(seat("cy", 0, false, Some((5.0, 1))));
        assert_eq!(
            players(&view(vec![lone.clone()]), &rule, Language::ZhCn),
            ["中等马 1L"]
        );
        // A pair the user wrote empty stays, beside the ones a blank value emptied.
        let own = CalloutRule {
            template: "{standing}【】【{name}】{seat}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            players(&view(vec![lone]), &own, Language::ZhCn),
            ["中等马【】1L"]
        );
    }

    #[test]
    fn a_chinese_opening_line_follows_winers_name_in_brackets() {
        let rated = view(vec![seat("ann", 1, false, Some((7.2, 0)))]);
        let first = |header: &str, style: CalloutStyle, language: Language| {
            let rule = CalloutRule {
                header: header.into(),
                style,
                ..CalloutRule::default()
            };
            lines(&rated, &rule, language, names).remove(0)
        };
        assert_eq!(
            first("会跑路", CalloutStyle::Rich, Language::ZhCn),
            "📢 winer 战绩鉴定【会跑路】"
        );
        assert_eq!(
            first(" 会跑路 ", CalloutStyle::Compact, Language::ZhCn),
            "winer 战绩鉴定【会跑路】"
        );
        assert_eq!(
            first("【五黑】冲", CalloutStyle::Rich, Language::ZhCn),
            "📢 winer 战绩鉴定【五黑】冲",
            "a bracket of its own keeps it apart already"
        );
        assert_eq!(
            first("会跑路", CalloutStyle::Rich, Language::En),
            "📢 winer rating · 会跑路"
        );
    }

    #[test]
    fn team_safe_chinese_defaults_keep_free_text_out_of_the_team_columns() {
        let mut player = seat("会跑路的防御塔", 0, false, Some((7.2, 0)));
        if let Some(rating) = player.rating.as_mut() {
            rating.title = Some("版本答案".into());
            rating.quip = Some("稳得离谱".into());
        }
        for style in [CalloutStyle::Compact, CalloutStyle::Rich] {
            let rule = CalloutRule {
                style,
                ..CalloutRule::default()
            };
            let line = &players(&view(vec![player.clone()]), &rule, Language::ZhCn)[0];
            assert!(line.starts_with("1L: "), "{line}");
            assert!(line.contains("上等马|"), "{line}");
            assert!(
                !line.contains("会跑路")
                    && !line.contains("版本答案")
                    && !line.contains("稳得离谱"),
                "{line}"
            );
            assert!(!line.contains("马会"), "{line}");
        }

        let mut in_game = player;
        in_game.is_self = true;
        let line = &ally_lines(
            &game_of(vec![in_game]),
            &CalloutRule::default(),
            Language::ZhCn,
            champions,
        )[1];
        assert_eq!(
            line,
            "【会跑路的防御塔】|档位上等马|近20场胜率55%|KDA3.5|战力7.2"
        );
        assert!(
            !line.contains("马会"),
            "the word 档位 separates the fallback name: {line}"
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
            lines[0], "📢【蓝色方】winer 战绩鉴定【开局分析】",
            "a side stands in for the game's"
        );
        assert!(
            lines[1].starts_with("1L: 👑 独角马|") && lines[5].starts_with("5L: 💀 纯牛马|"),
            "{lines:?}"
        );
        for (index, line) in lines[1..].iter().enumerate() {
            assert!(
                line.starts_with(&format!("{}L: ", index + 1)),
                "the seats in order: {lines:?}"
            );
        }
        let english = General {
            language: Language::En,
            ..General::default()
        };
        let lines = preview(&me, &rule, &english, names);
        assert!(
            lines[1].starts_with("👑 Unicorn: P1 ann, ")
                && lines[5].starts_with("💀 Pack mule: P5 ann, "),
            "{lines:?}"
        );
    }

    #[test]
    fn the_default_ranks_five_players_into_seat_first_safe_columns() {
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
                    score: Some(f64::from(wins) / 2.0),
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
            .map(|line| line.split('|').next().unwrap())
            .collect();
        assert_eq!(
            heads,
            [
                "1L: 💀 纯正牛马",
                "2L: 😅 移动眼位",
                "3L: 👌 峡谷公务员",
                "4L: 🔥 人形防御塔",
                "5L: 👑 峡谷通天代"
            ]
        );
        assert!(
            lines[4].starts_with("5L: 👑 峡谷通天代|近20场胜率70%"),
            "the best form stays in the fifth cell: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .all(|line| !line.contains("评语") && !line.contains("称号")),
            "free-form titles and quips stay out of the team-safe default: {lines:?}"
        );
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
    /// their own tier of the five horses, in 1L-to-5L seat order, each line naming who it is about.
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
                    score: Some(f64::from(wins(puuid)) / 2.0),
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
            .map(|line| line.split('|').next().unwrap())
            .collect();
        // Seats follow the cells, 0 to 4, whatever tier each lands in.
        assert_eq!(
            heads,
            [
                "1L: 👌 中等马",
                "2L: 💀 纯牛马",
                "3L: 🔥 上等马",
                "4L: 👑 独角马",
                "5L: 😅 下等马"
            ],
            "{lines:?}"
        );
        assert!(
            lines[3].starts_with("4L: 👑 独角马|近20场胜率85%"),
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
            ally_callout: Vec::new(),
        }
    }

    /// A running game with `team`, the local player among them, on the blue side.
    fn game_of(team: Vec<Seat>) -> GameView {
        GameView {
            teams: vec![team, vec![enemy("路人", 3, Some((2, 5, 5.2)))]],
            ..game(0, Vec::new())
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
                "小心【亚索】|档位T0|近20场胜率55%|KDA3.5",
                "对面【盖伦】|档位T4|近20场胜率55%|可以多抓",
            ],
            "the champion alone names the player in game; the middle one is not talked about"
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
                "Watch 亚索: T0, 55% in 20 games, KDA 3.5【版本答案】",
                "Go after 盖伦: T4, 55% in 20 games",
            ]
        );
    }

    #[test]
    fn in_game_the_default_lines_name_the_champion_alone() {
        assert_eq!(
            watch_template(Language::ZhCn),
            "小心【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}"
        );
        assert_eq!(
            target_template(Language::ZhCn),
            "对面【{champion}】|档位{standing}|近{games}场胜率{winRate}|可以多抓"
        );
        assert_eq!(
            ally_template(CalloutStyle::Rich, Language::ZhCn),
            "【{champion}】|档位{standing}|近{games}场胜率{winRate}|KDA{kda}|战力{score}",
            "champ select's line, the champion where the seat and the name were"
        );
        assert_eq!(
            ally_template(CalloutStyle::Compact, Language::ZhCn),
            "【{champion}】|档位{standing}|胜率{winRate}|KDA{kda}|战力{score}"
        );
        assert_eq!(
            watch_template(Language::En),
            "Watch {champion}: {standing}, {winRate} in {games} games, KDA {kda}{title}"
        );
        assert_eq!(
            target_template(Language::En),
            "Go after {champion}: {standing}, {winRate} in {games} games"
        );
        assert_eq!(
            ally_template(CalloutStyle::Rich, Language::En),
            "{standing}: {champion}, {winRate} in {games} games, KDA {kda}, form {score}{title}{quip}"
        );
        assert_eq!(
            ally_template(CalloutStyle::Compact, Language::En),
            "{standing} [{champion}] | {winRate} | KDA {kda} | form {score}",
            "a bare space would run the tier and the champion together"
        );
        for template in [
            watch_template(Language::ZhCn),
            target_template(Language::ZhCn),
            ally_template(CalloutStyle::Rich, Language::ZhCn),
            ally_template(CalloutStyle::Compact, Language::ZhCn),
            watch_template(Language::En),
            target_template(Language::En),
            ally_template(CalloutStyle::Rich, Language::En),
            ally_template(CalloutStyle::Compact, Language::En),
        ] {
            assert!(
                !template.contains("{name}")
                    && !template.contains("{seat}")
                    && !template.contains("{emoji}"),
                "{template}"
            );
        }
    }

    #[test]
    fn a_seat_without_a_known_champion_goes_by_the_players_name() {
        let enemies = vec![
            enemy("强者", 0, Some((0, 5, 7.8))),
            enemy("弱者", 77, Some((4, 5, 3.0))),
        ];
        let lines = game_lines(
            &game(0, enemies.clone()),
            &CalloutRule::default(),
            Language::ZhCn,
            champions,
        );
        assert_eq!(
            lines[1..],
            [
                "小心【强者】|档位T0|近20场胜率55%|KDA3.5",
                "对面【弱者】|档位T4|近20场胜率55%|可以多抓"
            ],
            "no champion, and one the catalog does not name: the line never reads empty"
        );
        let named = CalloutRule {
            watch_template: "小心 {champion} {name}：{standing}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            game_lines(&game(0, enemies), &named, Language::ZhCn, champions)[1],
            "小心 强者：T0",
            "a line that names the player already does not name them twice"
        );
        // Champ select too: a line of the user's own that names only the champion, before a pick.
        let own = CalloutRule {
            template: "{standing}：{champion}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            players(
                &view(vec![
                    seat("ann", 1, false, Some((7.2, 0))),
                    seat("bo", 0, false, Some((4.1, 2)))
                ]),
                &own,
                Language::ZhCn
            ),
            ["上等马：安妮", "下等马：bo"]
        );
    }

    #[test]
    fn in_game_the_tier_and_the_name_standing_in_for_a_champion_never_run_together() {
        // No champion the catalog names: the team's compact line goes by the player's name, which
        // 0.0.4 put after the tier with a space, and the chat's filter read 上等马 会跑路的防御塔 as
        // holding 马会.
        let team = || game_of(vec![seat("会跑路的防御塔", 0, true, Some((7.2, 0)))]);
        let compact = CalloutRule {
            style: CalloutStyle::Compact,
            ..CalloutRule::default()
        };
        assert_eq!(
            ally_lines(&team(), &compact, Language::ZhCn, champions)[1],
            "【会跑路的防御塔】|档位上等马|胜率55%|KDA3.5|战力7.2"
        );
        assert_eq!(
            ally_lines(&team(), &CalloutRule::default(), Language::ZhCn, champions)[1],
            "【会跑路的防御塔】|档位上等马|近20场胜率55%|KDA3.5|战力7.2"
        );
        assert_eq!(
            ally_lines(&team(), &compact, Language::En, champions)[1],
            "上等马 [会跑路的防御塔] | 55% | KDA 3.5 | form 7.2"
        );
        // Nothing to call the player by, neither champion nor name: no empty brackets either.
        let unnamed = || game_of(vec![nameless(seat("x", 0, true, Some((7.2, 0))))]);
        assert_eq!(
            ally_lines(&unnamed(), &compact, Language::ZhCn, champions)[1],
            "档位上等马|胜率55%|KDA3.5|战力7.2"
        );
        assert_eq!(
            ally_lines(
                &unnamed(),
                &CalloutRule::default(),
                Language::ZhCn,
                champions
            )[1],
            "档位上等马|近20场胜率55%|KDA3.5|战力7.2"
        );
        assert_eq!(
            ally_lines(&unnamed(), &compact, Language::En, champions)[1],
            "上等马 | 55% | KDA 3.5 | form 7.2"
        );
        let enemies = game(0, vec![nameless(enemy("x", 0, Some((0, 5, 7.8))))]);
        assert_eq!(
            game_lines(&enemies, &CalloutRule::default(), Language::ZhCn, champions)[1],
            "小心|档位T0|近20场胜率55%|KDA3.5"
        );
        assert_eq!(
            game_lines(&enemies, &CalloutRule::default(), Language::En, champions)[1],
            "Watch: T0, 55% in 20 games, KDA 3.5",
            "no space before the colon either"
        );
        let smiley = CalloutRule {
            watch_template: "{standing} :) {name}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            game_lines(&enemies, &smiley, Language::En, champions)[1],
            "T0 :)",
            "a colon that is no punctuation between words stays as written"
        );
    }

    #[test]
    fn only_the_former_in_game_lines_count_as_their_former_defaults() {
        assert_eq!(
            former_watch_default(
                "小心 {champion} {name}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}"
            ),
            Some(Language::ZhCn)
        );
        assert_eq!(
            former_watch_default(
                "Watch {champion} ({name}): {standing}, {winRate} in {games} games, KDA {kda}{title}"
            ),
            Some(Language::En)
        );
        assert_eq!(
            former_target_default(
                "对面 {champion} {name}：{standing}，近{games}场胜率{winRate}，可以多抓"
            ),
            Some(Language::ZhCn)
        );
        assert_eq!(
            former_target_default(
                "Go after {champion} ({name}): {standing}, {winRate} in {games} games"
            ),
            Some(Language::En)
        );
        // 0.0.4's, the champion bare.
        assert_eq!(
            former_watch_default(
                "小心 {champion}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}"
            ),
            Some(Language::ZhCn)
        );
        assert_eq!(
            former_target_default(
                "对面 {champion}：{standing}，近{games}场胜率{winRate}，可以多抓"
            ),
            Some(Language::ZhCn)
        );
        assert_eq!(
            former_ally_default("{standing} {champion}｜胜率{winRate}｜KDA {kda}｜战力{score}"),
            Some((CalloutStyle::Compact, Language::ZhCn))
        );
        assert_eq!(
            former_ally_default("{standing} {champion} | {winRate} | KDA {kda} | form {score}"),
            Some((CalloutStyle::Compact, Language::En))
        );
        assert_eq!(
            former_ally_default(
                "{standing}：{champion}，近{games}场胜率{winRate}，KDA {kda}，战力{score}{title}{quip}"
            ),
            Some((CalloutStyle::Rich, Language::ZhCn))
        );
        // Each line migrates to its own default only, and the defaults of now are not former ones.
        assert_eq!(
            former_target_default(
                "小心 {champion} {name}：{standing}，近{games}场胜率{winRate}，KDA {kda}{title}"
            ),
            None
        );
        assert_eq!(
            former_ally_default("{seat} {standing}｜胜率{winRate}｜KDA {kda}｜战力{score}｜{name}"),
            None,
            "champ select's line is not the team's in-game one"
        );
        for language in [Language::ZhCn, Language::En] {
            assert_eq!(former_watch_default(watch_template(language)), None);
            assert_eq!(former_target_default(target_template(language)), None);
            for style in [CalloutStyle::Compact, CalloutStyle::Rich] {
                assert_eq!(former_ally_default(ally_template(style, language)), None);
            }
        }
        assert_eq!(former_watch_default("小心 {champion} {name}"), None);
        assert_eq!(former_watch_default(""), None);
        assert_eq!(former_ally_default(""), None);
    }

    #[test]
    fn in_game_the_team_hears_about_itself_by_champion_in_team_order() {
        let mut best = seat("ann", 2, false, Some((7.2, 0)));
        if let Some(rating) = best.rating.as_mut() {
            rating.title = Some("版本答案".into());
            rating.quip = Some("稳得离谱，能C还能活".into());
        }
        let mut hidden = seat("cy", 1, false, None);
        hidden.stats = PlayerStats::Hidden;
        let team = vec![
            seat("me", 1, true, Some((5.5, 1))),
            hidden,
            best,
            seat("bo", 0, false, Some((4.1, 2))),
        ];
        let rule = CalloutRule::default();
        assert_eq!(
            ally_lines(&game_of(team.clone()), &rule, Language::ZhCn, champions),
            [
                "【我方·蓝色方】winer 战绩鉴定",
                "【亚索】|档位中等马|近20场胜率55%|KDA3.5|战力5.5",
                "【盖伦】|档位上等马|近20场胜率55%|KDA3.5|战力7.2",
                "【bo】|档位下等马|近20场胜率55%|KDA3.5|战力4.1",
            ],
            "the team order, each by champion, the one without a champion by name"
        );

        let red = GameView {
            teams: vec![vec![enemy("路人", 3, Some((2, 5, 5.2)))], team.clone()],
            ..game_of(Vec::new())
        };
        let english = ally_lines(&red, &rule, Language::En, champions);
        assert_eq!(english[0], "[My team · Red side] winer rating");
        assert_eq!(
            english[2],
            "上等马: 盖伦, 55% in 20 games, KDA 3.5, form 7.2【版本答案】，稳得离谱，能C还能活"
        );

        let without_me = CalloutRule {
            include_self: false,
            ally_template: "{seat} {champion}={standing}".into(),
            ..CalloutRule::default()
        };
        assert_eq!(
            ally_lines(&game_of(team), &without_me, Language::ZhCn, champions),
            [
                "【我方·蓝色方】winer 战绩鉴定",
                "3L 盖伦=上等马",
                "4L bo=下等马"
            ],
            "the user's own line, without themselves; seats stay their places in the team's list"
        );
    }

    #[test]
    fn the_team_says_nothing_in_game_without_two_sides_a_team_or_anyone_rated() {
        let team = vec![
            seat("me", 1, true, Some((5.5, 1))),
            seat("bo", 2, false, Some((4.1, 2))),
        ];
        let rule = CalloutRule::default();
        let mut sideless = game_of(team.clone());
        sideless.sides = false;
        assert!(ally_lines(&sideless, &rule, Language::ZhCn, champions).is_empty());
        let mut watching = game_of(team);
        for seat in &mut watching.teams[0] {
            seat.is_self = false;
        }
        assert!(
            ally_lines(&watching, &rule, Language::ZhCn, champions).is_empty(),
            "a spectator has no team of their own"
        );
        let unrated = game_of(vec![seat("me", 1, true, None), seat("bo", 2, false, None)]);
        assert!(
            ally_lines(&unrated, &rule, Language::ZhCn, champions).is_empty(),
            "nobody rated: not even the first line"
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
            lines[2].starts_with("对面【盖伦】|档位"),
            "a blank line is the default: {lines:?}"
        );
        assert!(
            game_lines(&game(0, enemies.clone()), &rule, Language::ZhCn, |_| None)[1]
                .starts_with("注意1L强者，"),
            "a champion the catalog does not name gives way to the player's name, with no gap"
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
    fn the_game_preview_types_what_one_press_would_with_the_users_own_form() {
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
                "小心【ann】|档位峡谷通天代|近20场胜率55%|KDA3.5",
                "对面【ann】|档位纯正牛马|近20场胜率55%|可以多抓",
            ],
            "the best and worst tiers, without free-form titles; no champion played, so the name stands in"
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
            lines[1].starts_with("Watch ann: Rift Demigod, ")
                && lines[2].starts_with("Go after ann: Pure Workhorse, "),
            "S+ and F of the grades: {lines:?}"
        );

        let allies = CalloutRule {
            game_teams: GameTeams::Allies,
            include_self: false,
            ..CalloutRule::default()
        };
        let lines = game_preview(&me, &allies, &General::default(), names);
        assert_eq!(
            lines.len(),
            6,
            "the team's first line and five tiers: {lines:?}"
        );
        assert_eq!(lines[0], "【我方·蓝色方】winer 战绩鉴定");
        assert!(
            lines[1].starts_with("【ann】|档位峡谷通天代|近20场胜率55%|KDA3.5|战力")
                && lines[5].starts_with("【ann】|档位纯正牛马|"),
            "every tier, the user's own line too: {lines:?}"
        );
        let both = CalloutRule {
            game_teams: GameTeams::Both,
            ..CalloutRule::default()
        };
        let lines = game_preview(&me, &both, &General::default(), names);
        assert_eq!(lines.len(), GAME_LINE_LIMIT);
        assert_eq!(
            (lines[0].as_str(), lines[3].as_str()),
            (
                "【敌方·红色方】winer 战绩鉴定",
                "【我方·蓝色方】winer 战绩鉴定"
            )
        );
        assert!(
            lines[4].contains("|档位峡谷通天代|") && lines[5].contains("|档位人形防御塔|"),
            "the enemy's, then the team's best two: {lines:?}"
        );
        let graded_allies = CalloutRule {
            tiers: TierSet::Grades,
            ..allies
        };
        let lines = game_preview(&me, &graded_allies, &General::default(), names);
        assert_eq!(lines.len(), GAME_LINE_LIMIT, "eight grades, cut: {lines:?}");
    }

    /// A game whose lines are already written: the enemy's three, the team's first line and five.
    fn written() -> GameView {
        GameView {
            callout: vec!["敌方".into(), "小心".into(), "对面".into()],
            ally_callout: ["我方", "一", "二", "三", "四", "五"]
                .map(str::to_owned)
                .to_vec(),
            ..game(0, Vec::new())
        }
    }

    #[test]
    fn a_press_types_the_chosen_teams_lines_the_enemys_first_and_six_at_most() {
        let view = written();
        assert_eq!(typed(&view, GameTeams::Enemies), ["敌方", "小心", "对面"]);
        assert_eq!(
            typed(&view, GameTeams::Allies),
            ["我方", "一", "二", "三", "四", "五"],
            "a team's first line and five players fit whole"
        );
        assert_eq!(
            typed(&view, GameTeams::Both),
            ["敌方", "小心", "对面", "我方", "一", "二"],
            "both: the enemy's first, then the team's best, cut at the limit"
        );
        assert_eq!(GAME_LINE_LIMIT, 6);
        let short = GameView {
            ally_callout: vec!["我方".into(), "一".into()],
            ..written()
        };
        assert_eq!(
            typed(&short, GameTeams::Both),
            ["敌方", "小心", "对面", "我方", "一"]
        );

        let rule = |game_teams: GameTeams| CalloutRule {
            in_game: true,
            game_teams,
            ..CalloutRule::default()
        };
        assert_eq!(
            press(
                Phase::InProgress,
                None,
                Some(&view),
                &rule(GameTeams::Allies)
            ),
            Press::Game(typed(&view, GameTeams::Allies))
        );
        assert_eq!(
            CalloutRule::default().game_teams,
            GameTeams::Enemies,
            "the enemy lines by default"
        );
        let quiet = GameView {
            ally_callout: Vec::new(),
            ..written()
        };
        assert_eq!(
            press(
                Phase::InProgress,
                None,
                Some(&quiet),
                &rule(GameTeams::Allies)
            ),
            Press::Skip(CalloutSkip::NothingToSay),
            "nothing on the chosen side, whatever the other has"
        );
        assert_eq!(
            press(
                Phase::InProgress,
                None,
                Some(&quiet),
                &rule(GameTeams::Both)
            ),
            Press::Game(vec!["敌方".into(), "小心".into(), "对面".into()])
        );
    }

    #[test]
    fn the_shortcut_sends_in_champ_select_types_in_the_game_and_otherwise_says_why_not() {
        let off = CalloutRule::default();
        let on = CalloutRule {
            in_game: true,
            ..CalloutRule::default()
        };
        let mut select = view(vec![seat("ann", 1, false, Some((7.2, 0)))]);
        assert_eq!(
            press(Phase::ChampSelect, Some(&select), None, &on),
            Press::Skip(CalloutSkip::NothingToSay),
            "no lines yet"
        );
        select.callout = vec!["line".into()];
        assert_eq!(
            press(Phase::ChampSelect, Some(&select), None, &off),
            Press::ChampSelect,
            "champ select's chat has an API: in-game sending plays no part"
        );

        let mut running = game(0, Vec::new());
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), &on),
            Press::Skip(CalloutSkip::NothingToSay)
        );
        running.callout = vec!["a".into(), "b".into()];
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), &off),
            Press::Skip(CalloutSkip::InGameOff)
        );
        assert_eq!(
            press(Phase::InProgress, None, Some(&running), &on),
            Press::Game(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            press(Phase::InProgress, None, None, &on),
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
                press(phase, Some(&select), Some(&running), &on),
                Press::Skip(CalloutSkip::NotNow),
                "{phase:?}"
            );
        }
    }
}
