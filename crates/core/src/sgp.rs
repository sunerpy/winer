//! Match history from the shard's own server (SGP). The Tencent client's LCU answers every page of `/lol-match-history` with the one window it fetched last,
//! never more than twenty games and whatever range was asked; SGP pages through the player's whole
//! history and sends all ten players of each game (measured on NJ100, 2026-10-05).
//!
//! Its game is a different shape from the LCU's (flat participants with their Riot IDs, runes nested
//! by tree) and is turned into the LCU's here, so the analysis reads one shape.

use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;

use crate::{
    diagnose::failure_of,
    model::{Game, Participant, ParticipantIdentity, Player, Stats, Team, TeamBan, Timeline},
    net,
    view::CheckFailure,
};

/// Fifty games, the most one page asks for, are about 6.5 MB.
const TIMEOUT: Duration = Duration::from_secs(20);

/// The match-history server of a Tencent shard. Other platforms have none here.
pub fn base(platform_id: &str) -> Option<&'static str> {
    Some(match platform_id.to_ascii_uppercase().as_str() {
        "HN1" => "https://hn1-k8s-sgp.lol.qq.com:21019",
        "HN10" => "https://hn10-k8s-sgp.lol.qq.com:21019",
        "TJ100" => "https://tj100-sgp.lol.qq.com:21019",
        "TJ101" => "https://tj101-sgp.lol.qq.com:21019",
        "NJ100" => "https://nj100-sgp.lol.qq.com:21019",
        "GZ100" => "https://gz100-sgp.lol.qq.com:21019",
        "CQ100" => "https://cq100-sgp.lol.qq.com:21019",
        "BGP2" => "https://bgp2-k8s-sgp.lol.qq.com:21019",
        _ => return None,
    })
}

/// The shard named by the entitlements issuer (`http://nj100-bcs-internal.lol.qq.com:28088` →
/// `NJ100`), for when chat has not said.
pub fn platform_of_issuer(issuer: &str) -> Option<String> {
    let host = issuer.split("://").nth(1)?;
    let shard = host.split(['-', '.', ':', '/']).next()?;
    (!shard.is_empty()).then(|| shard.to_ascii_uppercase())
}

/// The client's own match-history user agent, on the running client's version
/// (`16.19.8217343+branch…`), or the documented one when the version is unknown.
pub fn user_agent(game_version: &str) -> String {
    let version = game_version.split('+').next().unwrap_or_default().trim();
    let version = if version.is_empty() {
        "14.13.596.7996"
    } else {
        version
    };
    format!("LeagueOfLegendsClient/{version} (rcp-be-lol-match-history)")
}

/// One page as the server sent it, turned into the LCU's shape: an entry per place in the history,
/// `None` where the server listed a game it never recorded. A short page is the last.
#[derive(Debug, Default, PartialEq)]
pub struct Page {
    pub entries: Vec<Option<Game>>,
}

/// `count` games of `puuid`'s from the `begin`-th newest, with the client's own access token.
pub async fn history(
    base: &str,
    token: &str,
    user_agent: &str,
    puuid: &str,
    begin: u32,
    count: u32,
) -> Result<Page, String> {
    let response = request(base, token, user_agent, puuid, begin, count)?
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.without_url().to_string())?;
    let body = response.bytes().await.map_err(|error| error.to_string())?;
    let history: History = serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    Ok(history.into_page())
}

/// Whether the server answers one game of `puuid`'s, for the self-check: how it failed, without
/// the request (whose address holds the puuid).
pub async fn probe(
    base: &str,
    token: &str,
    user_agent: &str,
    puuid: &str,
) -> Result<(), CheckFailure> {
    let response = request(base, token, user_agent, puuid, 0, 1)
        .map_err(|_| CheckFailure::Other)?
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| failure_of(&error))?;
    let body = response.bytes().await.map_err(|error| failure_of(&error))?;
    serde_json::from_slice::<History>(&body).map_err(|_| CheckFailure::Decode)?;
    Ok(())
}

fn request(
    base: &str,
    token: &str,
    user_agent: &str,
    puuid: &str,
    begin: u32,
    count: u32,
) -> Result<reqwest::RequestBuilder, String> {
    let url = format!(
        "{base}/match-history-query/v1/products/lol/player/{puuid}/SUMMARY?startIndex={begin}&count={count}"
    );
    Ok(net::client()?
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(USER_AGENT, user_agent)
        .header(ACCEPT, "application/json")
        .timeout(TIMEOUT))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct History {
    games: Vec<Entry>,
}

impl History {
    pub(crate) fn into_page(self) -> Page {
        Page {
            entries: self
                .games
                .into_iter()
                .map(|entry| entry.json.into_game())
                .collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Entry {
    json: SgpGame,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SgpGame {
    game_id: i64,
    game_creation: i64,
    game_duration: i64,
    game_mode: String,
    game_type: String,
    queue_id: i64,
    map_id: i64,
    game_version: String,
    participants: Vec<SgpParticipant>,
    teams: Vec<SgpTeam>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SgpParticipant {
    participant_id: i64,
    puuid: String,
    riot_id_game_name: String,
    riot_id_tagline: String,
    summoner_id: i64,
    profile_icon: i64,
    team_id: i64,
    champion_id: i64,
    champ_level: i64,
    spell1_id: i64,
    spell2_id: i64,
    lane: String,
    role: String,
    team_position: String,
    win: bool,
    kills: i64,
    deaths: i64,
    assists: i64,
    total_minions_killed: i64,
    neutral_minions_killed: i64,
    gold_earned: i64,
    total_damage_dealt_to_champions: i64,
    total_damage_taken: i64,
    damage_self_mitigated: i64,
    damage_dealt_to_objectives: i64,
    #[serde(rename = "timeCCingOthers")]
    time_ccing_others: i64,
    vision_score: i64,
    item0: i64,
    item1: i64,
    item2: i64,
    item3: i64,
    item4: i64,
    item5: i64,
    item6: i64,
    perks: Perks,
    largest_multi_kill: i64,
    penta_kills: i64,
    quadra_kills: i64,
    triple_kills: i64,
    double_kills: i64,
    largest_killing_spree: i64,
    first_blood_kill: bool,
    turret_kills: i64,
    was_afk: bool,
    game_ended_in_early_surrender: bool,
    subteam_placement: i64,
    player_augment1: i64,
    player_augment2: i64,
    player_augment3: i64,
    player_augment4: i64,
    player_augment5: i64,
    player_augment6: i64,
}

/// Runes nested by tree, where the LCU flattens them.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Perks {
    styles: Vec<PerkStyle>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PerkStyle {
    /// `primaryStyle` or `subStyle`.
    description: String,
    style: i64,
    selections: Vec<PerkSelection>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PerkSelection {
    perk: i64,
}

impl Perks {
    fn style(&self, description: &str, position: usize) -> Option<&PerkStyle> {
        self.styles
            .iter()
            .find(|style| style.description == description)
            .or_else(|| self.styles.get(position))
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SgpTeam {
    team_id: i64,
    win: bool,
    bans: Vec<SgpBan>,
    objectives: Objectives,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SgpBan {
    champion_id: i64,
    pick_turn: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Objectives {
    baron: Objective,
    dragon: Objective,
    tower: Objective,
    inhibitor: Objective,
    rift_herald: Objective,
    horde: Objective,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Objective {
    kills: i64,
}

impl SgpGame {
    /// The LCU's shape of the same game; `None` for an entry the server sent without one (a game
    /// left before it was recorded comes back with id 0 and no players).
    fn into_game(self) -> Option<Game> {
        if self.game_id <= 0 {
            return None;
        }
        let (participants, participant_identities) = self
            .participants
            .into_iter()
            .map(SgpParticipant::split)
            .unzip();
        Some(Game {
            game_id: self.game_id,
            game_creation: self.game_creation,
            game_duration: self.game_duration,
            game_mode: self.game_mode,
            game_type: self.game_type,
            queue_id: self.queue_id,
            map_id: self.map_id,
            game_version: self.game_version,
            participants,
            participant_identities,
            teams: self.teams.into_iter().map(SgpTeam::into_team).collect(),
        })
    }
}

impl SgpParticipant {
    /// The LCU keeps who played apart from how they played.
    fn split(self) -> (Participant, ParticipantIdentity) {
        let primary = self.perks.style("primaryStyle", 0);
        let keystone = primary
            .and_then(|style| style.selections.first())
            .map_or(0, |selection| selection.perk);
        let primary_style = primary.map_or(0, |style| style.style);
        let sub_style = self
            .perks
            .style("subStyle", 1)
            .map_or(0, |style| style.style);
        let identity = ParticipantIdentity {
            participant_id: self.participant_id,
            player: Player {
                puuid: self.puuid,
                game_name: self.riot_id_game_name,
                tag_line: self.riot_id_tagline,
                summoner_id: self.summoner_id,
                profile_icon: self.profile_icon,
                platform_id: String::new(),
            },
        };
        let participant = Participant {
            participant_id: self.participant_id,
            team_id: self.team_id,
            champion_id: self.champion_id,
            spell1_id: self.spell1_id,
            spell2_id: self.spell2_id,
            timeline: Timeline {
                lane: self.lane,
                role: self.role,
            },
            team_position: self.team_position,
            stats: Stats {
                win: self.win,
                kills: self.kills,
                deaths: self.deaths,
                assists: self.assists,
                champ_level: self.champ_level,
                total_minions_killed: self.total_minions_killed,
                neutral_minions_killed: self.neutral_minions_killed,
                gold_earned: self.gold_earned,
                total_damage_dealt_to_champions: self.total_damage_dealt_to_champions,
                total_damage_taken: self.total_damage_taken,
                damage_self_mitigated: self.damage_self_mitigated,
                damage_dealt_to_objectives: self.damage_dealt_to_objectives,
                time_ccing_others: self.time_ccing_others,
                vision_score: self.vision_score,
                item0: self.item0,
                item1: self.item1,
                item2: self.item2,
                item3: self.item3,
                item4: self.item4,
                item5: self.item5,
                item6: self.item6,
                perk0: keystone,
                perk_primary_style: primary_style,
                perk_sub_style: sub_style,
                largest_multi_kill: self.largest_multi_kill,
                penta_kills: self.penta_kills,
                quadra_kills: self.quadra_kills,
                triple_kills: self.triple_kills,
                double_kills: self.double_kills,
                largest_killing_spree: self.largest_killing_spree,
                first_blood_kill: self.first_blood_kill,
                turret_kills: self.turret_kills,
                was_afk: self.was_afk,
                game_ended_in_early_surrender: self.game_ended_in_early_surrender,
                subteam_placement: self.subteam_placement,
                player_augment1: self.player_augment1,
                player_augment2: self.player_augment2,
                player_augment3: self.player_augment3,
                player_augment4: self.player_augment4,
                player_augment5: self.player_augment5,
                player_augment6: self.player_augment6,
            },
        };
        (participant, identity)
    }
}

impl SgpTeam {
    fn into_team(self) -> Team {
        let objectives = self.objectives;
        Team {
            team_id: self.team_id,
            win: if self.win { "Win" } else { "Fail" }.to_owned(),
            bans: self
                .bans
                .into_iter()
                .map(|ban| TeamBan {
                    champion_id: ban.champion_id,
                    pick_turn: ban.pick_turn,
                })
                .collect(),
            baron_kills: objectives.baron.kills,
            dragon_kills: objectives.dragon.kills,
            tower_kills: objectives.tower.kills,
            inhibitor_kills: objectives.inhibitor.kills,
            rift_herald_kills: objectives.rift_herald.kills,
            horde_kills: objectives.horde.kills,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analysis, test_support::fixture};

    #[test]
    fn tencent_shards_have_a_server_and_their_issuer_names_them() {
        assert_eq!(base("NJ100"), Some("https://nj100-sgp.lol.qq.com:21019"));
        assert_eq!(base("hn1"), Some("https://hn1-k8s-sgp.lol.qq.com:21019"));
        assert_eq!(base("NA1"), None);
        assert_eq!(base(""), None);
        assert_eq!(
            platform_of_issuer("http://nj100-bcs-internal.lol.qq.com:28088").as_deref(),
            Some("NJ100")
        );
        assert_eq!(platform_of_issuer("not a url"), None);
        assert_eq!(
            user_agent("16.19.8217343+branch.releases-16-19.code.publictencent.content.release"),
            "LeagueOfLegendsClient/16.19.8217343 (rcp-be-lol-match-history)"
        );
        assert!(user_agent("").contains("/14.13.596.7996 "));
    }

    #[test]
    fn a_page_from_the_server_describes_its_game_as_the_client_does() {
        let page = fixture::<History>("live/responses/sgp-match-history-summary.json").into_page();
        let [Some(from_server)] = page.entries.as_slice() else {
            panic!("one game expected, got {:?}", page.entries.len());
        };
        let kinds = analysis::QueueKinds::new();
        // The same game as the client's own `/lol-match-history/v1/games/{id}` sends it.
        let from_client: Game = fixture("live/responses/match-history-game-sgp-twin.json");
        assert_eq!(from_server.game_id, from_client.game_id);
        assert_eq!(
            analysis::match_detail(from_server, &crate::rating::Roles::new()),
            analysis::match_detail(&from_client, &crate::rating::Roles::new())
        );
        let roles = crate::rating::Roles::new();
        assert_eq!(
            analysis::match_summary("PUUID-0010", from_server, &roles, &kinds),
            analysis::match_summary("PUUID-0010", &from_client, &roles, &kinds)
        );
        let line = analysis::match_summary("PUUID-0010", from_server, &roles, &kinds)
            .expect("the player is in the game")
            .line;
        assert_eq!(
            (line.kills, line.deaths, line.assists, line.cs),
            (14, 12, 45, 62)
        );
        assert_eq!(line.augments.len(), 4);
        // The whole game is there, so the summary carries what only the whole game knows.
        let detail = analysis::match_detail(from_server, &crate::rating::Roles::new());
        let scored = detail
            .teams
            .iter()
            .flat_map(|team| &team.players)
            .find(|player| player.puuid == "PUUID-0010")
            .unwrap();
        assert_eq!(
            (line.score, line.award, line.grade, &line.feats),
            (scored.score, scored.award, scored.grade, &scored.feats)
        );
        assert!(
            detail
                .teams
                .iter()
                .flat_map(|team| &team.players)
                .any(|player| player.feats.contains(&crate::view::Feat::MostKills)),
            "somebody led the game in kills"
        );
        assert!(line.score.is_some() && line.damage_share.is_some());
        let mvp = detail
            .teams
            .iter()
            .flat_map(|team| &team.players)
            .find(|player| player.award == Some(crate::view::Award::Mvp))
            .unwrap();
        assert_eq!(
            analysis::match_summary(&mvp.puuid, from_server, &roles, &kinds)
                .unwrap()
                .line
                .award,
            Some(crate::view::Award::Mvp)
        );
    }

    #[test]
    fn an_entry_without_a_game_counts_toward_the_page_but_is_not_a_game() {
        let history: History = serde_json::from_value(serde_json::json!({
            "games": [
                {"metadata": {"private": true}, "json": {"gameId": 0, "participants": []}},
                {"json": {"gameId": 7, "queueId": 2400, "participants": [
                    {"participantId": 1, "puuid": "a", "teamId": 100, "win": true,
                     "perks": {"styles": [
                         {"description": "primaryStyle", "style": 8100, "selections": [{"perk": 8112}]},
                         {"description": "subStyle", "style": 8300, "selections": []}
                     ]}}
                ], "teams": [{"teamId": 100, "win": true, "objectives": {"tower": {"kills": 3}}}]}}
            ]
        }))
        .unwrap();
        let page = history.into_page();
        let [None, Some(game)] = page.entries.as_slice() else {
            panic!("an empty place, then a game: {:?}", page.entries);
        };
        let stats = &game.participants[0].stats;
        assert_eq!(
            (stats.perk0, stats.perk_primary_style, stats.perk_sub_style),
            (8112, 8100, 8300)
        );
        assert_eq!(
            game.identity(1).map(|player| player.puuid.as_str()),
            Some("a")
        );
        assert_eq!(
            (game.teams[0].win.as_str(), game.teams[0].tower_kills),
            ("Win", 3)
        );
    }
}
