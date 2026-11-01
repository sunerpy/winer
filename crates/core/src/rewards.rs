//! Rewards the client holds for the player to take: grants waiting for a selection, which the
//! client lists under `/lol-rewards/v1/grants` and claims one at a time
//! (`POST /lol-rewards/v1/grants/{id}/select`). What to claim is decided here; `service::rewards`
//! carries it out. Measured on a Tencent account (2026-10-31): 25 grants were waiting, the oldest
//! from 2023, each of them a reward or two the player takes all of, none a real choice; an
//! event pass's unclaimed level rewards are among them.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

/// A grant the client is waiting on the player for.
pub const WAITING: &str = "PENDING_SELECTION";
/// A grant whose rewards have been handed out.
pub const FULFILLED: &str = "FULFILLED";
/// A grant the server gave up on.
pub const FAILED: &str = "FAILED";

/// The title the client gives the reward groups nobody named
/// (`Placeholder Name for Reward Group DO NOT TRANSLATE`).
const PLACEHOLDER: &str = "DO NOT TRANSLATE";

/// One grant of `GET /lol-rewards/v1/grants`, as much of it as winer reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Grant {
    pub info: GrantInfo,
    pub reward_group: RewardGroup,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GrantInfo {
    pub id: String,
    pub reward_group_id: String,
    pub status: String,
    /// As the client sends it, `2026-09-17T14:51:41.258+00:00`.
    pub date_created: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RewardGroup {
    pub localizations: Localizations,
    pub rewards: Vec<Reward>,
    pub selection_strategy_config: Option<SelectionRule>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Localizations {
    pub title: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Reward {
    pub id: String,
    pub quantity: i64,
    pub localizations: Localizations,
    pub media: Media,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Media {
    /// Under `/lol-game-data/assets/`; empty when the client has none.
    pub icon_url: String,
}

/// How many of a group's rewards the player takes.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SelectionRule {
    pub min_selections_allowed: u32,
    pub max_selections_allowed: u32,
}

/// A grant waiting for the player, as the window shows it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PendingGrant {
    pub id: String,
    pub title: String,
    /// When the client granted it, as the client wrote it (ISO 8601); empty when it did not say.
    pub created: String,
    pub rewards: Vec<GrantReward>,
    /// How many of the rewards the player takes: all of them where `max` covers them all.
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantReward {
    pub id: String,
    pub title: String,
    pub quantity: i64,
    /// A game-data path; empty when the client has no picture.
    pub icon: String,
}

impl PendingGrant {
    /// More rewards on offer than the player may take: the player picks, never winer.
    pub fn is_choice(&self) -> bool {
        self.rewards.len() > self.max as usize
    }
}

/// The waiting grants, the window's view: every one, oldest first, and how many winer can claim
/// without asking.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RewardsView {
    pub grants: Vec<PendingGrant>,
    /// Grants with nothing to choose.
    pub claimable: u32,
    /// Grants that wait for the player to pick.
    pub choices: u32,
}

/// The rewards the player picked from one grant that offers a choice.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GrantChoice {
    pub grant_id: String,
    pub reward_ids: Vec<String>,
}

/// What came of claiming.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClaimReport {
    /// Grants the client read back as handed out.
    pub claimed: u32,
    pub failed: Vec<ClaimFailure>,
    /// Grants the client took but had not handed out when winer stopped waiting.
    pub processing: u32,
    /// Grants still waiting for the player afterwards.
    pub left: u32,
    /// Champ select or a game began, and the rest waits for later.
    pub stopped: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClaimFailure {
    pub grant_id: String,
    /// Empty for a grant that no longer waits: the window names it from its own list.
    pub title: String,
    pub reason: ClaimFailed,
}

/// Why a grant was not claimed.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClaimFailed {
    /// The client refused the request; its own words.
    Refused { message: String },
    /// The client took it and then reported it failed.
    Failed,
    /// The picks do not fit the grant's rule (too few, too many, not its rewards).
    Invalid,
    /// It no longer waits: claimed meanwhile, or gone.
    Gone,
}

/// The body of `POST /lol-rewards/v1/grants/{grantId}/select` (`LolRewardsSelectionRequestDTO`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub grant_id: String,
    pub reward_group_id: String,
    pub reward_ids: Vec<String>,
}

impl Selection {
    pub fn body(&self) -> Value {
        json!({
            "grantId": self.grant_id,
            "rewardGroupId": self.reward_group_id,
            "selections": self.reward_ids,
        })
    }
}

/// The grants of `list` still waiting for the player, oldest first.
pub fn waiting(list: Vec<Grant>) -> Vec<Grant> {
    let mut waiting: Vec<Grant> = list
        .into_iter()
        .filter(|grant| grant.info.status == WAITING && !grant.info.id.is_empty())
        .collect();
    // One format throughout, so the text sorts as the time does.
    waiting.sort_by(|a, b| a.info.date_created.cmp(&b.info.date_created));
    waiting
}

/// How `grant` reads in the window.
pub fn pending(grant: &Grant) -> PendingGrant {
    let group = &grant.reward_group;
    let rewards: Vec<GrantReward> = group
        .rewards
        .iter()
        .map(|reward| GrantReward {
            id: reward.id.clone(),
            title: reward.localizations.title.trim().to_owned(),
            quantity: reward.quantity,
            icon: reward.media.icon_url.clone(),
        })
        .collect();
    let named = group.localizations.title.trim();
    let title = if named.is_empty() || named.contains(PLACEHOLDER) {
        rewards
            .iter()
            .map(|reward| reward.title.as_str())
            .filter(|title| !title.is_empty())
            .collect::<Vec<_>>()
            .join("、")
    } else {
        named.to_owned()
    };
    // A group without a rule is taken whole.
    let count = rewards.len() as u32;
    let (min, max) = match group.selection_strategy_config {
        Some(rule) if rule.max_selections_allowed > 0 => (
            rule.min_selections_allowed.min(rule.max_selections_allowed),
            rule.max_selections_allowed,
        ),
        _ => (count, count),
    };
    PendingGrant {
        id: grant.info.id.clone(),
        title,
        created: grant.info.date_created.clone(),
        rewards,
        min,
        max,
    }
}

pub fn view(waiting: &[Grant]) -> RewardsView {
    let grants: Vec<PendingGrant> = waiting.iter().map(pending).collect();
    let choices = grants.iter().filter(|grant| grant.is_choice()).count() as u32;
    RewardsView {
        claimable: grants.len() as u32 - choices,
        choices,
        grants,
    }
}

/// The request that claims `grant`: every reward of a grant without a choice; for a choice,
/// exactly the player's `picked`, which must be the grant's own rewards, each once, as many as its
/// rule allows. `None` for a choice the player has not made.
pub fn selection(
    grant: &Grant,
    picked: Option<&[String]>,
) -> Option<Result<Selection, ClaimFailed>> {
    let shown = pending(grant);
    let reward_ids = if shown.is_choice() {
        let picked = picked?;
        let mut seen: Vec<&String> = Vec::with_capacity(picked.len());
        for id in picked {
            if seen.contains(&id) || !shown.rewards.iter().any(|reward| &reward.id == id) {
                return Some(Err(ClaimFailed::Invalid));
            }
            seen.push(id);
        }
        let count = picked.len() as u32;
        if count < shown.min || count > shown.max || count == 0 {
            return Some(Err(ClaimFailed::Invalid));
        }
        picked.to_vec()
    } else {
        shown
            .rewards
            .iter()
            .map(|reward| reward.id.clone())
            .collect()
    };
    Some(Ok(Selection {
        grant_id: grant.info.id.clone(),
        reward_group_id: grant.info.reward_group_id.clone(),
        reward_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;

    fn measured() -> Vec<Grant> {
        fixture("lcu-rest/lol-rewards-v1-grants.json")
    }

    fn grant(id: &str) -> Grant {
        measured()
            .into_iter()
            .find(|grant| grant.info.id == id)
            .unwrap()
    }

    #[test]
    fn the_waiting_grants_come_oldest_first_without_the_fulfilled_one() {
        let ids: Vec<String> = waiting(measured())
            .into_iter()
            .map(|grant| grant.info.id)
            .collect();
        assert_eq!(
            ids,
            [
                "grant-0001",
                "grant-0007",
                "grant-0013",
                "grant-0021",
                "grant-choice"
            ]
        );
    }

    #[test]
    fn a_grant_reads_by_its_name_or_its_rewards_and_says_what_the_player_takes() {
        let icon = pending(&grant("grant-0001"));
        assert_eq!(icon.title, "斗魂觉醒 神之眼 图标");
        assert_eq!((icon.min, icon.max, icon.is_choice()), (1, 1, false));
        assert!(icon.rewards[0].icon.starts_with("/lol-game-data/assets/"));

        // The placeholder group name is the client's own: the rewards name it instead.
        let essence = pending(&grant("grant-0007"));
        assert_eq!(essence.title, "750蓝色精萃");
        assert_eq!(essence.rewards[0].quantity, 750);
        assert_eq!(essence.created, "2026-03-21T14:01:56.987+00:00");

        let pair = pending(&grant("grant-0013"));
        assert_eq!(pair.title, "Faker之友", "trimmed");
        assert_eq!((pair.min, pair.max, pair.is_choice()), (2, 2, false));

        let choice = pending(&grant("grant-choice"));
        assert_eq!((choice.min, choice.max, choice.is_choice()), (1, 1, true));

        let shown = view(&waiting(measured()));
        assert_eq!((shown.claimable, shown.choices), (4, 1));
    }

    #[test]
    fn a_grant_without_a_choice_is_claimed_whole_and_a_choice_only_as_picked() {
        let pair = selection(&grant("grant-0013"), None).unwrap().unwrap();
        assert_eq!(pair.grant_id, "grant-0013");
        assert_eq!(pair.reward_ids.len(), 2, "both rewards");
        assert_eq!(
            pair.body(),
            json!({
                "grantId": "grant-0013",
                "rewardGroupId": pair.reward_group_id,
                "selections": pair.reward_ids,
            })
        );

        let choice = grant("grant-choice");
        assert_eq!(selection(&choice, None), None, "not picked: not claimed");
        let picked = |ids: &[&str]| {
            let ids: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
            selection(&choice, Some(&ids)).unwrap()
        };
        assert_eq!(
            picked(&["reward-choice-2"]).unwrap().reward_ids,
            ["reward-choice-2"]
        );
        assert_eq!(picked(&[]), Err(ClaimFailed::Invalid), "too few");
        assert_eq!(
            picked(&["reward-choice-1", "reward-choice-2"]),
            Err(ClaimFailed::Invalid),
            "too many"
        );
        assert_eq!(
            picked(&["reward-0002"]),
            Err(ClaimFailed::Invalid),
            "not its own"
        );
        let twice = Grant {
            reward_group: RewardGroup {
                selection_strategy_config: Some(SelectionRule {
                    min_selections_allowed: 1,
                    max_selections_allowed: 2,
                }),
                ..choice.reward_group.clone()
            },
            ..choice.clone()
        };
        let repeated = vec!["reward-choice-1".to_owned(), "reward-choice-1".to_owned()];
        assert_eq!(
            selection(&twice, Some(&repeated)),
            Some(Err(ClaimFailed::Invalid)),
            "the same reward twice"
        );
    }

    #[test]
    fn a_group_without_a_rule_is_taken_whole() {
        let mut bare = grant("grant-0013");
        bare.reward_group.selection_strategy_config = None;
        let shown = pending(&bare);
        assert_eq!((shown.min, shown.max, shown.is_choice()), (2, 2, false));
    }
}
