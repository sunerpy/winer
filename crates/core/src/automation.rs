//! What to do for the user in queue and in champ select, decided from the session alone.
//! Carrying a decision out is the service's job; nothing here touches the network.

use std::collections::HashSet;

use crate::{
    model::{ChampSelectSession, ReadyCheck},
    settings::Automation,
    view::{LanePreference, Position},
};

/// Accept while the dialog is up and the user has not answered. A decline stays a decline.
pub fn should_accept(check: &ReadyCheck) -> bool {
    check.state == "InProgress" && check.player_response == "None"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChampAction {
    pub action_id: i64,
    pub champion_id: i64,
    pub step: Step,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// Planning phase: show the pick as an intent; it cannot be completed yet.
    Declare,
    /// The pick turn, hovering only; the user locks.
    Hover,
    Lock,
    Ban,
}

impl Step {
    /// The action is completed after the champion is set.
    pub fn completes(self) -> bool {
        matches!(self, Self::Lock | Self::Ban)
    }
}

/// What the client allows this account, as far as it has said. `None` means not known yet, in
/// which case nothing is filtered and the client has the last word.
#[derive(Clone, Debug, Default)]
pub struct Availability {
    pub pickable: Option<HashSet<i64>>,
    pub bannable: Option<HashSet<i64>>,
}

impl Availability {
    fn can_pick(&self, champion: i64) -> bool {
        self.pickable
            .as_ref()
            .is_none_or(|ids| ids.contains(&champion))
    }

    fn can_ban(&self, champion: i64) -> bool {
        self.bannable
            .as_ref()
            .is_none_or(|ids| ids.contains(&champion))
    }
}

/// The ARAM bench champion to take: the one highest on the wishlist, provided it ranks above
/// what the player holds now. Taking it puts the held champion on the bench, which by the same
/// rule is never taken back.
pub fn bench_pick(session: &ChampSelectSession, wishlist: &[i64]) -> Option<i64> {
    if !session.bench_enabled {
        return None;
    }
    let rank = |champion: i64| wishlist.iter().position(|&id| id == champion && id > 0);
    let held = session
        .local_player()
        .and_then(|me| rank(me.champion_id))
        .unwrap_or(usize::MAX);
    session
        .bench_champions
        .iter()
        .filter_map(|bench| Some((rank(bench.champion_id)?, bench.champion_id)))
        .filter(|&(place, _)| place < held)
        .min()
        .map(|(_, champion)| champion)
}

/// The lanes the local player asked for in the lobby, kept for the champ select it leads to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanePreferences {
    /// The queue the lobby was set up for.
    pub queue_id: i64,
    /// First choice first; FILL included, nothing for `UNSELECTED`.
    pub lanes: Vec<LanePreference>,
}

/// Whether champ select sent the local player to a lane they did not ask for (补位): the lobby's
/// preferences are for this queue, the assigned lane is one of the five, at least one preference
/// is a lane, none of them is FILL, and the assigned lane is not among them. No preferences, none
/// set, FILL, no lane assigned or another queue's lobby are never counted.
pub fn autofilled(preferences: Option<&LanePreferences>, queue_id: i64, assigned: &str) -> bool {
    let Some(preferences) = preferences.filter(|preferences| preferences.queue_id == queue_id)
    else {
        return false;
    };
    let Some(lane) = Position::parse(assigned) else {
        return false;
    };
    if preferences.lanes.contains(&LanePreference::Fill) {
        return false;
    }
    let asked: Vec<Position> = preferences
        .lanes
        .iter()
        .filter_map(|preference| preference.position())
        .collect();
    !asked.is_empty() && !asked.contains(&lane)
}

/// `filled`: the player was sent to a lane they did not ask for ([`autofilled`]); with
/// `skip_when_filled` on, only that lane's own list is picked from then.
pub fn decide(
    session: &ChampSelectSession,
    rules: &Automation,
    available: &Availability,
    filled: bool,
) -> Option<ChampAction> {
    let me = session.local_player()?;
    let position = Position::parse(&me.assigned_position);
    let own_only = filled && rules.pick.skip_when_filled;
    let mine = || {
        session
            .all_actions()
            .filter(|action| action.actor_cell_id == me.cell_id && !action.completed)
    };

    // Gone for everyone: banned, or locked by anyone.
    let mut gone: HashSet<i64> = session
        .bans
        .my_team_bans
        .iter()
        .chain(&session.bans.their_team_bans)
        .copied()
        .collect();
    gone.extend(
        session
            .all_actions()
            .filter(|action| action.completed && action.champion_id > 0)
            .map(|action| action.champion_id),
    );
    // Spoken for by a teammate: never take or ban what someone on the team has shown.
    let teammates: HashSet<i64> = session
        .my_team
        .iter()
        .filter(|player| player.cell_id != me.cell_id)
        .flat_map(|player| [player.champion_id, player.champion_pick_intent])
        .filter(|&id| id > 0)
        .collect();
    let free =
        |champion: i64| champion > 0 && !gone.contains(&champion) && !teammates.contains(&champion);

    let pick = |hovered: i64| {
        // What the user (or an earlier intent) already chose wins over the list.
        if free(hovered) && available.can_pick(hovered) {
            return Some(hovered);
        }
        let pool = &rules.pick.champions;
        let candidates = if own_only {
            pool.own_candidates(position)
        } else {
            pool.candidates(position)
        };
        candidates
            .into_iter()
            .find(|&id| free(id) && available.can_pick(id))
    };

    match session.timer.phase.as_str() {
        // The ban action already reads as in progress here, but completing anything is refused.
        "PLANNING" => {
            if !(rules.pick.enabled && rules.pick.declare_intent) || me.champion_pick_intent > 0 {
                return None;
            }
            let action = mine().find(|action| action.kind == "pick")?;
            let champion = pick(0)?;
            Some(ChampAction {
                action_id: action.id,
                champion_id: champion,
                step: Step::Declare,
            })
        }
        "BAN_PICK" => {
            let action = mine().find(|action| action.is_in_progress)?;
            match action.kind.as_str() {
                "ban" if rules.ban.enabled => {
                    let ban = |id: i64| free(id) && available.can_ban(id);
                    let champion =
                        Some(action.champion_id).filter(|&id| ban(id)).or_else(|| {
                            rules
                                .ban
                                .champions
                                .candidates(position)
                                .into_iter()
                                .find(|&id| ban(id))
                        })?;
                    Some(ChampAction {
                        action_id: action.id,
                        champion_id: champion,
                        step: Step::Ban,
                    })
                }
                "pick" if rules.pick.enabled => {
                    let hovered = if action.champion_id > 0 {
                        action.champion_id
                    } else {
                        me.champion_pick_intent
                    };
                    if !rules.pick.lock_in {
                        // Hover once; whatever the user does afterwards is theirs.
                        return (hovered == 0).then(|| pick(0)).flatten().map(|champion| {
                            ChampAction {
                                action_id: action.id,
                                champion_id: champion,
                                step: Step::Hover,
                            }
                        });
                    }
                    let champion = pick(hovered)?;
                    Some(ChampAction {
                        action_id: action.id,
                        champion_id: champion,
                        step: Step::Lock,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_lane_the_player_did_not_ask_for_is_autofilled() {
        use crate::view::LanePreference::{Fill, Middle, Top};
        let asked = |lanes: Vec<LanePreference>| LanePreferences {
            queue_id: 420,
            lanes,
        };
        let mid_top = asked(vec![Middle, Top]);
        assert!(autofilled(Some(&mid_top), 420, "utility"));
        assert!(!autofilled(Some(&mid_top), 420, "top"), "the second choice");
        assert!(
            !autofilled(Some(&mid_top), 440, "utility"),
            "another queue's lobby"
        );
        assert!(!autofilled(Some(&mid_top), 420, ""), "no lane assigned");
        assert!(!autofilled(Some(&mid_top), 420, "NONE"));
        assert!(
            !autofilled(Some(&asked(vec![Middle, Fill])), 420, "jungle"),
            "FILL asked"
        );
        assert!(!autofilled(Some(&asked(vec![Fill])), 420, "jungle"));
        assert!(
            !autofilled(Some(&asked(vec![])), 420, "jungle"),
            "nothing asked"
        );
        assert!(!autofilled(None, 420, "jungle"), "no lobby seen");
        assert!(autofilled(Some(&asked(vec![Middle])), 420, "jungle"));
    }
    use crate::{settings::ChampionPool, test_support::fixture};

    fn rules(pick: &[i64], ban: &[i64]) -> Automation {
        let mut rules = Automation::default();
        rules.pick.enabled = true;
        rules.pick.champions = ChampionPool {
            any: pick.to_vec(),
            ..ChampionPool::default()
        };
        rules.ban.enabled = true;
        rules.ban.champions = ChampionPool {
            any: ban.to_vec(),
            ..ChampionPool::default()
        };
        rules
    }

    fn session(name: &str) -> ChampSelectSession {
        fixture(&format!(
            "lcu-rest/lol-champ-select-v1-session--champ-select-{name}.json"
        ))
    }

    #[test]
    fn accepts_only_an_unanswered_check() {
        let check = |state: &str, response: &str| ReadyCheck {
            state: state.into(),
            player_response: response.into(),
            timer: 3.0,
        };
        assert!(should_accept(&check("InProgress", "None")));
        assert!(!should_accept(&check("InProgress", "Declined")));
        assert!(!should_accept(&check("InProgress", "Accepted")));
        assert!(!should_accept(&check("Invalid", "None")));
    }

    #[test]
    fn planning_declares_an_intent_and_never_completes_the_ban_it_shows_in_progress() {
        let planning = session("planning");
        let action = decide(
            &planning,
            &rules(&[86], &[157]),
            &Availability::default(),
            false,
        );
        assert_eq!(
            action,
            Some(ChampAction {
                action_id: 1,
                champion_id: 86,
                step: Step::Declare
            })
        );

        let mut no_intent = rules(&[86], &[157]);
        no_intent.pick.declare_intent = false;
        assert_eq!(
            decide(&planning, &no_intent, &Availability::default(), false),
            None
        );
    }

    #[test]
    fn the_ban_turn_bans_the_first_free_candidate() {
        let ban_pick = session("ban-pick");
        let action = decide(
            &ban_pick,
            &rules(&[86], &[157, 238]),
            &Availability::default(),
            false,
        );
        assert_eq!(
            action,
            Some(ChampAction {
                action_id: 0,
                champion_id: 157,
                step: Step::Ban
            })
        );

        let not_bannable = Availability {
            bannable: Some(HashSet::from([238])),
            ..Availability::default()
        };
        assert_eq!(
            decide(&ban_pick, &rules(&[], &[157, 238]), &not_bannable, false)
                .map(|action| action.champion_id),
            Some(238)
        );

        let mut off = rules(&[86], &[157]);
        off.ban.enabled = false;
        assert_eq!(
            decide(&ban_pick, &off, &Availability::default(), false),
            None
        );
    }

    fn pick_turn(my_team: serde_json::Value, actions: serde_json::Value) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 0,
            "timer": {"phase": "BAN_PICK"},
            "myTeam": my_team,
            "actions": actions,
            "bans": {"myTeamBans": [11], "theirTeamBans": [22]}
        }))
        .unwrap()
    }

    /// Sent to a lane the player did not ask for, the pick comes from that lane's own list only;
    /// without one there is no pick, and a champion the player hovered is still theirs.
    #[test]
    fn an_autofilled_player_picks_from_the_lanes_own_list_only() {
        let session = pick_turn(
            serde_json::json!([{"cellId": 0, "assignedPosition": "utility"}]),
            serde_json::json!([[{"id": 7, "actorCellId": 0, "type": "pick", "isInProgress": true}]]),
        );
        let mut rules = rules(&[86], &[]);
        rules.pick.lock_in = true;
        let pick = |rules: &Automation, filled: bool| {
            decide(&session, rules, &Availability::default(), filled)
                .map(|action| action.champion_id)
        };
        assert_eq!(pick(&rules, false), Some(86), "not filled: the any list");
        assert_eq!(pick(&rules, true), None, "filled, no support list: nothing");
        rules.pick.champions.utility = vec![412];
        assert_eq!(pick(&rules, true), Some(412));
        rules.pick.skip_when_filled = false;
        assert_eq!(
            pick(&rules, true),
            Some(412),
            "the lane's list comes first anyway"
        );
        rules.pick.champions.utility.clear();
        assert_eq!(
            pick(&rules, true),
            Some(86),
            "switched off: the any list again"
        );

        let hovered = pick_turn(
            serde_json::json!([{"cellId": 0, "assignedPosition": "utility"}]),
            serde_json::json!([[{"id": 7, "actorCellId": 0, "type": "pick", "isInProgress": true, "championId": 99}]]),
        );
        rules.pick.skip_when_filled = true;
        assert_eq!(
            decide(&hovered, &rules, &Availability::default(), true)
                .map(|action| action.champion_id),
            Some(99),
            "the user's own hover is still locked"
        );
    }

    #[test]
    fn the_pick_turn_skips_banned_locked_and_teammate_champions() {
        let session = pick_turn(
            serde_json::json!([
                {"cellId": 0, "assignedPosition": "middle"},
                {"cellId": 1, "championPickIntent": 33}
            ]),
            serde_json::json!([
                [{"id": 5, "actorCellId": 3, "type": "pick", "completed": true, "championId": 44}],
                [{"id": 6, "actorCellId": 0, "type": "pick", "isInProgress": true}]
            ]),
        );
        let mut rules = rules(&[11, 22, 33, 44, 55, 66], &[]);
        rules.pick.champions.middle = vec![66];
        let action = decide(
            &session,
            &rules,
            &Availability {
                pickable: Some(HashSet::from([55, 66])),
                ..Availability::default()
            },
            false,
        );
        assert_eq!(
            action,
            Some(ChampAction {
                action_id: 6,
                champion_id: 66,
                step: Step::Lock
            }),
            "the position list comes first"
        );
        let unowned = Availability {
            pickable: Some(HashSet::from([55])),
            ..Availability::default()
        };
        assert_eq!(
            decide(&session, &rules, &unowned, false).map(|action| action.champion_id),
            Some(55)
        );
    }

    #[test]
    fn a_hovered_champion_is_locked_and_hover_only_mode_leaves_it_alone() {
        let session = pick_turn(
            serde_json::json!([{"cellId": 0, "championPickIntent": 77}]),
            serde_json::json!([[{"id": 9, "actorCellId": 0, "type": "pick", "isInProgress": true, "championId": 0}]]),
        );
        let rules_lock = rules(&[55], &[]);
        assert_eq!(
            decide(&session, &rules_lock, &Availability::default(), false)
                .map(|action| action.champion_id),
            Some(77)
        );

        let mut hover_only = rules(&[55], &[]);
        hover_only.pick.lock_in = false;
        assert_eq!(
            decide(&session, &hover_only, &Availability::default(), false),
            None,
            "an intent is already showing"
        );

        let empty = pick_turn(
            serde_json::json!([{"cellId": 0}]),
            serde_json::json!([[{"id": 9, "actorCellId": 0, "type": "pick", "isInProgress": true}]]),
        );
        assert_eq!(
            decide(&empty, &hover_only, &Availability::default(), false),
            Some(ChampAction {
                action_id: 9,
                champion_id: 55,
                step: Step::Hover
            })
        );
    }

    fn bench(held: i64, bench: &[i64], enabled: bool) -> ChampSelectSession {
        serde_json::from_value(serde_json::json!({
            "localPlayerCellId": 3,
            "benchEnabled": enabled,
            "benchChampions": bench.iter().map(|id| serde_json::json!({"championId": id})).collect::<Vec<_>>(),
            "myTeam": [{"cellId": 3, "championId": held}]
        }))
        .unwrap()
    }

    #[test]
    fn the_bench_gives_up_only_a_better_wishlist_champion() {
        let wishlist = [10, 20, 30];
        assert_eq!(
            bench_pick(&bench(99, &[30, 20, 5], true), &wishlist),
            Some(20),
            "the best one on offer"
        );
        assert_eq!(bench_pick(&bench(20, &[30, 10], true), &wishlist), Some(10));
        assert_eq!(
            bench_pick(&bench(10, &[20, 30], true), &wishlist),
            None,
            "already holding the best"
        );
        assert_eq!(
            bench_pick(&bench(20, &[30, 40], true), &wishlist),
            None,
            "nothing better on the bench"
        );
        assert_eq!(
            bench_pick(&bench(99, &[10], false), &wishlist),
            None,
            "no bench in this mode"
        );
        assert_eq!(bench_pick(&bench(99, &[10], true), &[]), None);
    }

    #[test]
    fn nothing_happens_outside_planning_and_ban_pick() {
        let finalization = session("finalization");
        assert_eq!(
            decide(
                &finalization,
                &rules(&[86], &[157]),
                &Availability::default(),
                false
            ),
            None
        );
    }
}
