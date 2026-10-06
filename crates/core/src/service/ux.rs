//! The client's interface restarted for a loader linked just now. The client loads `version.dll`
//! only as its interface process starts, so a new loader waits for that process to restart
//! (`kill-and-restart-ux`). Sent while the client was still signing in, the restart left the new
//! interface hidden, its clicks lost for minutes (`docs/platform-notes.md`): it waits until the
//! client has settled, and the new interface is brought up (`ux-show`) once it is back.

use std::time::Duration;

use serde_json::json;
use tokio::{
    sync::watch,
    time::{Instant, sleep, timeout},
};
use tracing::{debug, info, warn};

use super::{CHAT_ME, Client, CoreError, PHASE, Service};
use crate::{model::ChatMe, view::Phase};

/// How long a loader linked just now waits for the client to settle; then it is left for the
/// client's next launch.
const SETTLE_WAIT: Duration = Duration::from_secs(120);
/// The first pause between two readings of a client settling; each pause doubles, up to
/// [`SETTLE_POLL_MAX`].
const SETTLE_POLL: Duration = Duration::from_secs(1);
const SETTLE_POLL_MAX: Duration = Duration::from_secs(5);
/// How long the restarted interface has to come back, its plugin saying hello on the bridge,
/// before it is brought up anyway.
const BACK_WAIT: Duration = Duration::from_secs(30);

const RESTART_UX: &str = "/riotclient/kill-and-restart-ux";
/// Shows the client's window and its page.
const SHOW_UX: &str = "/riotclient/ux-show";

/// How far a client has come in starting, as one reading finds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readiness {
    /// Signed in to chat: the presence carries an availability and the client's own `lol` keys,
    /// which a client still signing in has not written.
    pub signed_in: bool,
    /// The gameflow phase; `Unknown` when the client did not say.
    pub phase: Phase,
}

impl Readiness {
    /// From the client's answers: its chat presence (`None` when it gave none) and its phase.
    fn read(me: Option<&ChatMe>, phase: Option<&str>) -> Self {
        Self {
            signed_in: me.is_some_and(|me| !me.availability.is_empty() && !me.lol.is_empty()),
            phase: phase.map_or(Phase::Unknown, Phase::parse),
        }
    }

    /// Done starting, with the player idle in it: outside any lobby, queue, champ select or game.
    pub fn settled(self) -> bool {
        self.signed_in && self.phase == Phase::None
    }
}

/// What came of restarting the client's interface for a loader linked just now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiRestart {
    /// Restarted once the client had settled, then brought up (`shown`, unless the client refused):
    /// as soon as its plugin was back on the bridge (`plugin_back`), else after half a minute.
    Restarted { plugin_back: bool, shown: bool },
    /// The client had not settled within two minutes, as it was last read; the loader starts with
    /// the client's next launch.
    NotSettled(Readiness),
}

impl Service {
    /// Restarts the client's interface for a loader linked just now, once the client has settled:
    /// signed in to chat, with the player outside any lobby, queue, champ select or game. A restart
    /// any sooner can leave the new interface hidden. The client is read every few seconds for up
    /// to two minutes, then left to load the loader at its next launch. The new interface is
    /// brought up once its plugin is back on the bridge, or after half a minute. The game and the
    /// login session are untouched. This takes minutes at worst: call it off anything that must
    /// answer at once.
    pub async fn restart_client_ui_when_idle(&self) -> Result<UiRestart, CoreError> {
        let mut hellos = self.plugin_hellos();
        let ux = Connected {
            service: self,
            client: self.client()?,
        };
        restart_when_settled(&ux, &mut hellos).await
    }

    /// A plugin in the client's page said hello on the bridge: the page is up, a loader in it.
    pub(crate) fn plugin_connected(&self) {
        self.inner.plugin_hellos.send_modify(|hellos| *hellos += 1);
    }

    /// Changes with every [`Self::plugin_connected`] from now on.
    pub(crate) fn plugin_hellos(&self) -> watch::Receiver<u64> {
        self.inner.plugin_hellos.subscribe()
    }
}

/// What the restart asks of a client: the LCU in the app, a script in the tests.
trait ClientUx {
    /// How far the client has come; `NotConnected` once it has gone.
    async fn readiness(&self) -> Result<Readiness, CoreError>;
    async fn restart(&self) -> Result<(), CoreError>;
    async fn show(&self) -> Result<(), CoreError>;
}

/// Restarts `ux`'s interface once it has settled, read every few seconds for up to
/// [`SETTLE_WAIT`], then brings the new one up as soon as `hellos` changes (its plugin is back on
/// the bridge), or after [`BACK_WAIT`].
async fn restart_when_settled(
    ux: &impl ClientUx,
    hellos: &mut watch::Receiver<u64>,
) -> Result<UiRestart, CoreError> {
    let started = Instant::now();
    let deadline = started + SETTLE_WAIT;
    let mut pause = SETTLE_POLL;
    loop {
        let readiness = ux.readiness().await?;
        if readiness.settled() {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(UiRestart::NotSettled(readiness));
        }
        debug!(
            signed_in = readiness.signed_in,
            phase = ?readiness.phase,
            "the client has not settled"
        );
        sleep(pause.min(deadline - now)).await;
        pause = (pause * 2).min(SETTLE_POLL_MAX);
    }
    info!(waited = ?started.elapsed(), "the client has settled; restarting its interface");
    // Only a hello from the new interface counts.
    hellos.mark_unchanged();
    ux.restart().await?;
    let plugin_back = timeout(BACK_WAIT, hellos.changed())
        .await
        .is_ok_and(|changed| changed.is_ok());
    let shown = ux
        .show()
        .await
        .inspect_err(|error| warn!(%error, "the restarted client interface was not brought up"))
        .is_ok();
    Ok(UiRestart::Restarted { plugin_back, shown })
}

/// The connected client, for as long as it stays the one connected.
struct Connected<'a> {
    service: &'a Service,
    client: Client,
}

impl ClientUx for Connected<'_> {
    async fn readiness(&self) -> Result<Readiness, CoreError> {
        if !self.service.is_current(&self.client) {
            return Err(CoreError::NotConnected);
        }
        let lcu = &self.client.lcu;
        let (me, phase) = tokio::join!(
            lcu.get_optional::<ChatMe>(CHAT_ME),
            lcu.get::<String>(PHASE)
        );
        let me = me.unwrap_or_else(|error| {
            debug!(%error, "chat presence unavailable");
            None
        });
        Ok(Readiness::read(me.as_ref(), phase.ok().as_deref()))
    }

    async fn restart(&self) -> Result<(), CoreError> {
        Ok(self.client.lcu.post(RESTART_UX, &json!({})).await?)
    }

    async fn show(&self) -> Result<(), CoreError> {
        Ok(self.client.lcu.post(SHOW_UX, &json!({})).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use crate::test_support::fixture;

    const SIGNING_IN: Readiness = Readiness {
        signed_in: false,
        phase: Phase::None,
    };
    const IDLE: Readiness = Readiness {
        signed_in: true,
        phase: Phase::None,
    };

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    /// A client answering from a script: one reading a poll, the last one again once the script
    /// runs out, `None` for a client that has gone. Its plugin says hello `hello_after` the
    /// restart, if at all. Notes what was asked of it, and when.
    struct Scripted {
        readings: Mutex<VecDeque<Option<Readiness>>>,
        hellos: watch::Sender<u64>,
        hello_after: Option<Duration>,
        refuse_show: bool,
        started: Instant,
        asked: Mutex<Vec<(&'static str, Duration)>>,
    }

    impl Scripted {
        fn new(readings: &[Option<Readiness>], hello_after: Option<Duration>) -> Self {
            Self {
                readings: Mutex::new(readings.iter().copied().collect()),
                hellos: watch::Sender::new(0),
                hello_after,
                refuse_show: false,
                started: Instant::now(),
                asked: Mutex::new(Vec::new()),
            }
        }

        fn note(&self, what: &'static str) {
            let at = self.started.elapsed();
            self.asked.lock().unwrap().push((what, at));
        }

        /// When the client was read.
        fn reads(&self) -> Vec<Duration> {
            let asked = self.asked.lock().unwrap();
            asked
                .iter()
                .filter(|(what, _)| *what == "read")
                .map(|(_, at)| *at)
                .collect()
        }

        /// What else was asked of it, and when.
        fn acts(&self) -> Vec<(&'static str, Duration)> {
            let asked = self.asked.lock().unwrap();
            asked
                .iter()
                .filter(|(what, _)| *what != "read")
                .copied()
                .collect()
        }
    }

    impl ClientUx for Scripted {
        async fn readiness(&self) -> Result<Readiness, CoreError> {
            self.note("read");
            let mut readings = self.readings.lock().unwrap();
            let reading = if readings.len() > 1 {
                readings.pop_front().flatten()
            } else {
                readings.front().copied().flatten()
            };
            reading.ok_or(CoreError::NotConnected)
        }

        async fn restart(&self) -> Result<(), CoreError> {
            self.note("restart");
            if let Some(after) = self.hello_after {
                let hellos = self.hellos.clone();
                tokio::spawn(async move {
                    sleep(after).await;
                    hellos.send_modify(|hellos| *hellos += 1);
                });
            }
            Ok(())
        }

        async fn show(&self) -> Result<(), CoreError> {
            self.note("show");
            if self.refuse_show {
                return Err(CoreError::Invalid("refused".into()));
            }
            Ok(())
        }
    }

    #[test]
    fn a_client_has_settled_once_signed_in_to_chat_with_the_player_idle() {
        let signed_in: ChatMe = fixture("live/profile/chat-me.json");
        let invisible = ChatMe {
            availability: "offline".into(),
            ..signed_in.clone()
        };
        // A presence without `lol`, as a client still signing in to chat has.
        let signing_in: ChatMe =
            serde_json::from_value(json!({"availability": "chat", "platformId": "NJ100"})).unwrap();
        let no_availability = ChatMe {
            availability: String::new(),
            ..signed_in.clone()
        };
        assert!(Readiness::read(Some(&signed_in), Some("None")).settled());
        assert!(
            Readiness::read(Some(&invisible), Some("None")).settled(),
            "an invisible player is signed in too"
        );
        assert_eq!(
            Readiness::read(Some(&signing_in), Some("None")),
            SIGNING_IN,
            "no `lol` yet"
        );
        assert!(!Readiness::read(Some(&no_availability), Some("None")).settled());
        assert!(
            !Readiness::read(None, Some("None")).settled(),
            "no presence at all"
        );
        for phase in [
            "Lobby",
            "Matchmaking",
            "ReadyCheck",
            "ChampSelect",
            "InProgress",
            "EndOfGame",
        ] {
            assert!(
                !Readiness::read(Some(&signed_in), Some(phase)).settled(),
                "{phase}"
            );
        }
        assert_eq!(
            Readiness::read(Some(&signed_in), None),
            Readiness {
                signed_in: true,
                phase: Phase::Unknown
            },
            "a phase the client did not give is not idle"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_loader_waits_for_the_client_to_sign_in_and_go_idle_then_shows_it() {
        let lobby = Readiness {
            signed_in: true,
            phase: Phase::Lobby,
        };
        let ux = Scripted::new(
            &[Some(SIGNING_IN), Some(SIGNING_IN), Some(lobby), Some(IDLE)],
            Some(secs(3)),
        );
        let mut hellos = ux.hellos.subscribe();
        assert_eq!(
            restart_when_settled(&ux, &mut hellos).await.unwrap(),
            UiRestart::Restarted {
                plugin_back: true,
                shown: true
            }
        );
        assert_eq!(
            ux.reads(),
            [secs(0), secs(1), secs(3), secs(7)],
            "each pause twice the last"
        );
        assert_eq!(
            ux.acts(),
            [("restart", secs(7)), ("show", secs(10))],
            "restarted at the first idle reading, shown as soon as its plugin said hello"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_client_that_never_settles_keeps_its_new_loader_for_the_next_launch() {
        let playing = Readiness {
            signed_in: true,
            phase: Phase::InProgress,
        };
        for stuck in [SIGNING_IN, playing] {
            let ux = Scripted::new(&[Some(stuck)], Some(secs(1)));
            let mut hellos = ux.hellos.subscribe();
            assert_eq!(
                restart_when_settled(&ux, &mut hellos).await.unwrap(),
                UiRestart::NotSettled(stuck)
            );
            assert!(ux.acts().is_empty(), "neither restarted nor shown");
            let reads = ux.reads();
            assert_eq!(
                reads.last(),
                Some(&SETTLE_WAIT),
                "read a last time as the wait ends"
            );
            assert!(
                reads
                    .windows(2)
                    .all(|pair| pair[1] - pair[0] <= SETTLE_POLL_MAX),
                "{reads:?}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_new_interface_is_brought_up_when_its_plugin_never_comes_back() {
        let ux = Scripted::new(&[Some(IDLE)], None);
        let mut hellos = ux.hellos.subscribe();
        // A hello from before the restart, which says nothing about the new interface.
        ux.hellos.send_modify(|hellos| *hellos += 1);
        assert_eq!(
            restart_when_settled(&ux, &mut hellos).await.unwrap(),
            UiRestart::Restarted {
                plugin_back: false,
                shown: true
            }
        );
        assert_eq!(ux.acts(), [("restart", secs(0)), ("show", BACK_WAIT)]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_show_the_client_refuses_leaves_the_restart_standing() {
        let mut ux = Scripted::new(&[Some(IDLE)], Some(secs(2)));
        ux.refuse_show = true;
        let mut hellos = ux.hellos.subscribe();
        assert_eq!(
            restart_when_settled(&ux, &mut hellos).await.unwrap(),
            UiRestart::Restarted {
                plugin_back: true,
                shown: false
            }
        );
        assert_eq!(ux.acts(), [("restart", secs(0)), ("show", secs(2))]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_client_gone_while_it_settles_is_left_alone() {
        let ux = Scripted::new(&[Some(SIGNING_IN), None], Some(secs(1)));
        let mut hellos = ux.hellos.subscribe();
        assert!(matches!(
            restart_when_settled(&ux, &mut hellos).await,
            Err(CoreError::NotConnected)
        ));
        assert!(ux.acts().is_empty(), "{:?}", ux.acts());
    }

    #[tokio::test]
    async fn without_a_client_nothing_waits() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tokio::runtime::Handle::current(),
        );
        assert!(matches!(
            service.restart_client_ui_when_idle().await,
            Err(CoreError::NotConnected)
        ));
    }
}
