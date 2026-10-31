//! Claiming the rewards the client holds for the player (`crate::rewards`): the window's button,
//! and with the switch on, once after connecting and once after each game. One claimer serves all
//! three, so no grant is sent twice, and it stops before the next request once the queue, champ
//! select or a game begins: the client answers each claim with toasts of its own.

use std::{
    collections::HashSet,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use lcu::{Lcu, Method};
use serde_json::Value;
use tokio::time::sleep;
use tracing::{info, warn};

use super::{Client, CoreError, Service, lock, query_value};
use crate::{
    rewards::{
        self, ClaimFailed, ClaimFailure, ClaimReport, Grant, GrantChoice, RewardsView, Selection,
    },
    view::{NoticeKind, Phase},
};

const GRANTS: &str = "/lol-rewards/v1/grants";
/// The client hands a claimed grant out about 1.5 s after the request (2026-10-31): winer reads
/// the grants again this often, at most this many times, before it calls the rest still processing.
const SETTLE_POLL: Duration = Duration::from_millis(500);
const SETTLE_POLLS: u32 = 20;
/// The post-game screen gets a moment first, as before going back to the lobby.
const AFTER_GAME: Duration = Duration::from_secs(3);

/// Where claiming may go on: out of the queue, champ select and the game. The queue is out too,
/// because the ready check can come at any moment.
pub(super) fn claims_in(phase: Phase) -> bool {
    matches!(phase, Phase::None | Phase::Lobby | Phase::EndOfGame)
}

/// Whether the claim after connecting runs: only once the client said where it is (`initial`),
/// and not on the post-game screen, whose own claim follows.
pub(super) fn claims_on_connect(initial: Option<Phase>) -> bool {
    initial.is_some_and(|phase| phase != Phase::EndOfGame && claims_in(phase))
}

/// Whether a run may send its next request: still the client and account it started for, out of
/// the queue and the game, and for an automatic run the switch still on.
pub(super) fn may_claim(
    same_account: bool,
    phase: Phase,
    automatic: bool,
    switched_on: bool,
) -> bool {
    same_account && claims_in(phase) && (!automatic || switched_on)
}

/// The client's side of claiming; the tests use one of their own.
pub(super) trait RewardsApi {
    /// The grants the client lists as waiting for the player.
    fn waiting(&self) -> impl Future<Output = Result<Vec<Grant>, CoreError>> + Send;
    fn select(&self, selection: &Selection) -> impl Future<Output = Result<(), CoreError>> + Send;
    /// Every grant, whatever its status.
    fn grants(&self) -> impl Future<Output = Result<Vec<Grant>, CoreError>> + Send;
}

pub(super) struct LcuRewards(pub Lcu);

impl RewardsApi for LcuRewards {
    async fn waiting(&self) -> Result<Vec<Grant>, CoreError> {
        Ok(self
            .0
            .get(&format!("{GRANTS}?status={}", rewards::WAITING))
            .await?)
    }

    async fn select(&self, selection: &Selection) -> Result<(), CoreError> {
        let path = format!("{GRANTS}/{}/select", query_value(&selection.grant_id));
        // The answer is the grant, `PENDING_FULFILLMENT` by then; the settling reads say the rest.
        self.0
            .call::<_, Value>(Method::POST, &path, &selection.body())
            .await?;
        Ok(())
    }

    async fn grants(&self) -> Result<Vec<Grant>, CoreError> {
        Ok(self.0.get(GRANTS).await?)
    }
}

/// What a run claims besides every grant with nothing to choose: the player's picks from the
/// button. The automatic runs never pick.
pub(super) enum Trigger {
    Button(Vec<GrantChoice>),
    Automatic,
}

#[derive(Default)]
pub(super) struct RewardsClaimer {
    /// One run at a time: a grant a run sent no longer waits when the next one reads.
    turn: tokio::sync::Mutex<()>,
    /// Grants a claim failed for. The automatic runs leave them alone while winer runs, so one
    /// broken grant does not fail after every game; the button still tries them.
    failed: Mutex<HashSet<String>>,
}

impl RewardsClaimer {
    /// Claims what `trigger` asks for, oldest grant first, one request at a time and only while
    /// `open` says so; then reads the grants until the client has handed them out, or
    /// [`SETTLE_POLLS`] reads later calls the rest still processing.
    pub(super) async fn claim(
        &self,
        api: &impl RewardsApi,
        trigger: Trigger,
        open: impl Fn() -> bool,
    ) -> Result<ClaimReport, CoreError> {
        let _turn = self.turn.lock().await;
        let waiting = rewards::waiting(api.waiting().await?);
        let (automatic, picks) = match &trigger {
            Trigger::Button(picks) => (false, picks.as_slice()),
            Trigger::Automatic => (true, &[][..]),
        };
        let mut report = ClaimReport::default();
        for choice in picks {
            if !waiting.iter().any(|grant| grant.info.id == choice.grant_id) {
                report.failed.push(ClaimFailure {
                    grant_id: choice.grant_id.clone(),
                    title: String::new(),
                    reason: ClaimFailed::Gone,
                });
            }
        }

        let mut sent: Vec<(String, String)> = Vec::new();
        for grant in &waiting {
            let id = &grant.info.id;
            if automatic && lock(&self.failed).contains(id) {
                continue;
            }
            let picked = picks
                .iter()
                .find(|choice| &choice.grant_id == id)
                .map(|choice| choice.reward_ids.as_slice());
            let title = rewards::pending(grant).title;
            let selection = match rewards::selection(grant, picked) {
                // A choice the player has not made.
                None => continue,
                Some(Err(reason)) => {
                    report.failed.push(ClaimFailure {
                        grant_id: id.clone(),
                        title,
                        reason,
                    });
                    continue;
                }
                Some(Ok(selection)) => selection,
            };
            if !open() {
                report.stopped = true;
                break;
            }
            match api.select(&selection).await {
                Ok(()) => sent.push((id.clone(), title)),
                Err(error) => {
                    lock(&self.failed).insert(id.clone());
                    report.failed.push(ClaimFailure {
                        grant_id: id.clone(),
                        title,
                        reason: ClaimFailed::Refused {
                            message: error.to_string(),
                        },
                    });
                }
            }
        }

        let estimate = waiting.len() - sent.len();
        let mut unsettled = sent;
        for _ in 0..SETTLE_POLLS {
            if unsettled.is_empty() {
                break;
            }
            sleep(SETTLE_POLL).await;
            // A read that failed is tried again until the time is up.
            let Ok(grants) = api.grants().await else {
                continue;
            };
            unsettled.retain(|(id, title)| {
                let status = grants
                    .iter()
                    .find(|grant| &grant.info.id == id)
                    .map(|grant| grant.info.status.as_str());
                match status {
                    Some(rewards::FULFILLED) => {
                        report.claimed += 1;
                        false
                    }
                    Some(rewards::FAILED) => {
                        lock(&self.failed).insert(id.clone());
                        report.failed.push(ClaimFailure {
                            grant_id: id.clone(),
                            title: title.clone(),
                            reason: ClaimFailed::Failed,
                        });
                        false
                    }
                    _ => true,
                }
            });
        }
        report.processing = unsettled.len() as u32;
        report.left = match api.waiting().await {
            Ok(list) => rewards::waiting(list).len(),
            Err(_) => estimate,
        } as u32;
        Ok(report)
    }
}

impl Service {
    /// The grants waiting for the player, oldest first.
    pub async fn pending_rewards(&self) -> Result<RewardsView, CoreError> {
        let lcu = self.client()?.lcu;
        let waiting = LcuRewards(lcu).waiting().await?;
        Ok(rewards::view(&rewards::waiting(waiting)))
    }

    /// Claims every grant with nothing to choose, and each of `choices` as the player picked;
    /// refused while the queue, champ select or a game is on.
    pub async fn claim_rewards(&self, choices: Vec<GrantChoice>) -> Result<ClaimReport, CoreError> {
        let client = self.client()?;
        if !claims_in(lock(&client.live).phase) {
            return Err(CoreError::Busy(
                "not during the queue, champ select or a game".into(),
            ));
        }
        let open = self.claim_gate(&client, false);
        let api = LcuRewards(client.lcu.clone());
        let report = self
            .inner
            .rewards
            .claim(&api, Trigger::Button(choices), open)
            .await?;
        info!(
            claimed = report.claimed,
            failed = report.failed.len(),
            processing = report.processing,
            left = report.left,
            stopped = report.stopped,
            "claimed rewards"
        );
        Ok(report)
    }

    /// With the switch on, claims what needs no choice; `after_game`, once the post-game screen
    /// has had a moment. Never awaited by the event loop, which keeps the phase current meanwhile.
    pub(super) fn claim_rewards_by_rule(&self, client: &Client, after_game: bool) {
        if !self.settings().automation.claim_rewards {
            return;
        }
        let (service, client) = (self.clone(), client.clone());
        self.spawn(async move {
            if after_game {
                sleep(AFTER_GAME).await;
            }
            let open = service.claim_gate(&client, true);
            if !open() {
                return;
            }
            let api = LcuRewards(client.lcu.clone());
            match service
                .inner
                .rewards
                .claim(&api, Trigger::Automatic, open)
                .await
            {
                Ok(report) => {
                    info!(
                        claimed = report.claimed,
                        failed = report.failed.len(),
                        processing = report.processing,
                        left = report.left,
                        stopped = report.stopped,
                        "claimed rewards automatically"
                    );
                    if report.claimed > 0 {
                        service.notice(NoticeKind::RewardsClaimed {
                            claimed: report.claimed,
                            left: report.left,
                        });
                    }
                    if !report.failed.is_empty() {
                        service.notice(NoticeKind::RewardsFailed {
                            failed: report.failed.len() as u32,
                        });
                    }
                }
                Err(error) => {
                    warn!(%error, "claiming rewards automatically failed");
                    service.notice(NoticeKind::Failed {
                        action: "claimRewards".into(),
                        message: error.to_string(),
                    });
                }
            }
        });
    }

    /// Whether a run for `client` may go on (`may_claim`), as things are each time it asks.
    fn claim_gate(&self, client: &Client, automatic: bool) -> impl Fn() -> bool + Send + 'static {
        let service = self.clone();
        let live = client.live.clone();
        let me = lock(&client.live).me.clone();
        move || {
            let connected = service
                .inner
                .client
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .is_some_and(|now| Arc::ptr_eq(&now.live, &live));
            let switched_on = service.settings().automation.claim_rewards;
            let state = lock(&live);
            may_claim(
                connected && state.me == me,
                state.phase,
                automatic,
                switched_on,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::test_support::fixture;

    /// The client as the measured list has it: a claimed grant waits for fulfilment, then is
    /// handed out at the next read, unless the test says otherwise.
    #[derive(Default)]
    struct Fake {
        grants: Mutex<Vec<Grant>>,
        sent: Mutex<Vec<String>>,
        /// The request is refused.
        refuse: HashSet<String>,
        /// Taken, then reported failed.
        fail: HashSet<String>,
        /// Taken and never handed out.
        stuck: HashSet<String>,
        /// Runs after each request the client took.
        after: Option<Box<dyn Fn() + Send + Sync>>,
    }

    impl Fake {
        fn measured() -> Self {
            Self {
                grants: Mutex::new(fixture("lcu-rest/lol-rewards-v1-grants.json")),
                ..Self::default()
            }
        }

        fn sent(&self) -> Vec<String> {
            lock(&self.sent).clone()
        }
    }

    impl RewardsApi for Fake {
        async fn waiting(&self) -> Result<Vec<Grant>, CoreError> {
            Ok(lock(&self.grants)
                .iter()
                .filter(|grant| grant.info.status == rewards::WAITING)
                .cloned()
                .collect())
        }

        async fn select(&self, selection: &Selection) -> Result<(), CoreError> {
            lock(&self.sent).push(selection.grant_id.clone());
            if self.refuse.contains(&selection.grant_id) {
                return Err(CoreError::Invalid("refused".into()));
            }
            for grant in lock(&self.grants).iter_mut() {
                if grant.info.id == selection.grant_id {
                    grant.info.status = "PENDING_FULFILLMENT".into();
                }
            }
            if let Some(after) = &self.after {
                after();
            }
            Ok(())
        }

        async fn grants(&self) -> Result<Vec<Grant>, CoreError> {
            let mut grants = lock(&self.grants);
            for grant in grants.iter_mut() {
                let id = &grant.info.id;
                if grant.info.status == "PENDING_FULFILLMENT" && !self.stuck.contains(id) {
                    grant.info.status = if self.fail.contains(id) {
                        rewards::FAILED
                    } else {
                        rewards::FULFILLED
                    }
                    .into();
                }
            }
            Ok(grants.clone())
        }
    }

    const NO_CHOICE: [&str; 4] = ["grant-0001", "grant-0007", "grant-0013", "grant-0021"];

    fn always() -> impl Fn() -> bool {
        || true
    }

    fn pick(grant: &str, rewards: &[&str]) -> GrantChoice {
        GrantChoice {
            grant_id: grant.into(),
            reward_ids: rewards.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn claims_happen_out_of_the_queue_champ_select_and_the_game_only() {
        for phase in [Phase::None, Phase::Lobby, Phase::EndOfGame] {
            assert!(claims_in(phase), "{phase:?}");
        }
        for phase in [
            Phase::Matchmaking,
            Phase::ReadyCheck,
            Phase::ChampSelect,
            Phase::GameStart,
            Phase::InProgress,
            Phase::Reconnect,
            Phase::WaitingForStats,
            Phase::PreEndOfGame,
        ] {
            assert!(!claims_in(phase), "{phase:?}");
        }
        assert!(claims_on_connect(Some(Phase::Lobby)));
        assert!(!claims_on_connect(None), "the phase was not read");
        assert!(
            !claims_on_connect(Some(Phase::EndOfGame)),
            "the post-game claim covers it"
        );
        assert!(!claims_on_connect(Some(Phase::InProgress)));
        assert!(may_claim(true, Phase::Lobby, true, true));
        assert!(!may_claim(true, Phase::Lobby, true, false), "switched off");
        assert!(may_claim(true, Phase::Lobby, false, false), "the button");
        assert!(
            !may_claim(false, Phase::Lobby, false, true),
            "another client"
        );
        assert!(!may_claim(true, Phase::ChampSelect, false, true));
    }

    #[tokio::test(start_paused = true)]
    async fn every_grant_without_a_choice_is_claimed_oldest_first_and_read_back_handed_out() {
        let client = Fake::measured();
        let report = RewardsClaimer::default()
            .claim(&client, Trigger::Automatic, always())
            .await
            .unwrap();
        assert_eq!(client.sent(), NO_CHOICE);
        assert_eq!(
            report,
            ClaimReport {
                claimed: 4,
                left: 1,
                ..ClaimReport::default()
            },
            "the choice waits for the player"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_choice_is_claimed_as_picked_and_a_pick_that_does_not_fit_is_not_sent() {
        let client = Fake::measured();
        let claimer = RewardsClaimer::default();
        let report = claimer
            .claim(
                &client,
                Trigger::Button(vec![
                    pick("grant-choice", &["reward-choice-1", "reward-choice-2"]),
                    pick("grant-gone", &["reward-x"]),
                ]),
                always(),
            )
            .await
            .unwrap();
        assert_eq!(client.sent(), NO_CHOICE, "two picks of a one-of-three");
        let reasons: Vec<(String, ClaimFailed)> = report
            .failed
            .iter()
            .map(|failure| (failure.grant_id.clone(), failure.reason.clone()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("grant-gone".to_owned(), ClaimFailed::Gone),
                ("grant-choice".to_owned(), ClaimFailed::Invalid),
            ]
        );
        assert_eq!((report.claimed, report.left), (4, 1));

        let report = claimer
            .claim(
                &client,
                Trigger::Button(vec![pick("grant-choice", &["reward-choice-2"])]),
                always(),
            )
            .await
            .unwrap();
        assert_eq!(
            client.sent().last().map(String::as_str),
            Some("grant-choice")
        );
        assert_eq!((report.claimed, report.left), (1, 0));
    }

    #[tokio::test(start_paused = true)]
    async fn champ_select_beginning_stops_the_run_before_the_next_request() {
        let open = Arc::new(AtomicBool::new(true));
        let client = Fake {
            after: Some(Box::new({
                let open = open.clone();
                move || open.store(false, Ordering::SeqCst)
            })),
            ..Fake::measured()
        };
        let report = RewardsClaimer::default()
            .claim(&client, Trigger::Automatic, {
                let open = open.clone();
                move || open.load(Ordering::SeqCst)
            })
            .await
            .unwrap();
        assert_eq!(client.sent(), ["grant-0001"]);
        assert!(report.stopped);
        assert_eq!((report.claimed, report.left), (1, 4));
    }

    #[tokio::test(start_paused = true)]
    async fn two_runs_at_once_send_each_grant_once() {
        let client = Fake::measured();
        let claimer = RewardsClaimer::default();
        let (first, second) = tokio::join!(
            claimer.claim(&client, Trigger::Automatic, always()),
            claimer.claim(&client, Trigger::Automatic, always())
        );
        assert_eq!(client.sent(), NO_CHOICE);
        assert_eq!(first.unwrap().claimed + second.unwrap().claimed, 4);
    }

    #[tokio::test(start_paused = true)]
    async fn an_automatic_run_leaves_a_grant_that_failed_alone_and_the_button_tries_it_again() {
        let client = Fake {
            refuse: HashSet::from(["grant-0007".to_owned()]),
            ..Fake::measured()
        };
        let claimer = RewardsClaimer::default();
        let report = claimer
            .claim(&client, Trigger::Automatic, always())
            .await
            .unwrap();
        assert_eq!(report.claimed, 3);
        assert!(matches!(
            &report.failed[..],
            [ClaimFailure { grant_id, reason: ClaimFailed::Refused { .. }, .. }] if grant_id == "grant-0007"
        ));
        assert_eq!(report.left, 2, "the refused grant and the choice");

        claimer
            .claim(&client, Trigger::Automatic, always())
            .await
            .unwrap();
        assert_eq!(client.sent().len(), 4, "not sent again");
        claimer
            .claim(&client, Trigger::Button(Vec::new()), always())
            .await
            .unwrap();
        assert_eq!(client.sent().last().map(String::as_str), Some("grant-0007"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_grant_not_handed_out_in_time_is_processing_and_one_reported_failed_is_failed() {
        let client = Fake {
            stuck: HashSet::from(["grant-0013".to_owned()]),
            fail: HashSet::from(["grant-0021".to_owned()]),
            ..Fake::measured()
        };
        let started = tokio::time::Instant::now();
        let report = RewardsClaimer::default()
            .claim(&client, Trigger::Automatic, always())
            .await
            .unwrap();
        assert_eq!((report.claimed, report.processing), (2, 1));
        assert!(matches!(
            &report.failed[..],
            [ClaimFailure { grant_id, reason: ClaimFailed::Failed, .. }] if grant_id == "grant-0021"
        ));
        assert_eq!(
            started.elapsed(),
            SETTLE_POLL * SETTLE_POLLS,
            "waited the whole time for the stuck one"
        );
    }
}
