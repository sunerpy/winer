//! The Comebacks page's side of the service (`crate::comeback`): what a press of its shortcuts does
//! now, a line or a quick phrase posted in champ select's chat, and the library downloaded on the
//! user's request, kept in a file of its own beside the settings.

use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use tracing::{info, warn};

use super::{CoreError, Service, champ_select_chat, lock, say};
use crate::{
    callout,
    comeback::{self, ComebackPress, Library, Memory},
    settings::{Audience, CalloutDelivery, write_atomic},
    view::{ComebackLibrary, Phase},
};

/// What the service keeps for the Comebacks page: what went out last, and the library.
pub(super) struct ComebackState {
    memory: Mutex<Memory>,
    library: LibraryStore,
    /// Downloading and removing the library, one at a time.
    library_work: tokio::sync::Mutex<()>,
}

impl ComebackState {
    /// The library lives in `path` (`comeback-library.json` beside the settings).
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            memory: Mutex::new(Memory::default()),
            library: LibraryStore {
                path,
                loaded: Mutex::new(None),
            },
            library_work: tokio::sync::Mutex::new(()),
        }
    }
}

/// The downloaded library's file, read on first use and kept in memory.
struct LibraryStore {
    path: PathBuf,
    /// `None` until the file was read; then the library, or `None` inside where there is none.
    loaded: Mutex<Option<Option<Arc<Library>>>>,
}

impl LibraryStore {
    fn get(&self) -> Option<Arc<Library>> {
        let mut loaded = lock(&self.loaded);
        if let Some(library) = &*loaded {
            return library.clone();
        }
        let library = match std::fs::read(&self.path) {
            Ok(bytes) => match serde_json::from_slice::<Library>(&bytes) {
                Ok(library) => Some(Arc::new(library)),
                Err(error) => {
                    warn!(%error, "the downloaded library is unreadable; it counts as none");
                    None
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                warn!(%error, "the downloaded library could not be read");
                None
            }
        };
        *loaded = Some(library.clone());
        library
    }

    fn put(&self, library: Library) -> Result<Arc<Library>, CoreError> {
        let bytes =
            serde_json::to_vec(&library).map_err(|error| CoreError::Io(io::Error::other(error)))?;
        let mut loaded = lock(&self.loaded);
        write_atomic(&self.path, &bytes)?;
        let library = Arc::new(library);
        *loaded = Some(Some(library.clone()));
        Ok(library)
    }

    fn remove(&self) -> Result<(), CoreError> {
        let mut loaded = lock(&self.loaded);
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        *loaded = Some(None);
        Ok(())
    }
}

/// A random number for [`comeback::pick`]; the clock stands in should the system have none.
fn random() -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_ok() {
        u64::from_le_bytes(bytes)
    } else {
        u64::try_from(super::now_ms()).unwrap_or_default()
    }
}

impl Service {
    /// What a press of the Comebacks page's team shortcut (`all` false) or all-chat shortcut does
    /// now (`comeback::press`), from the phase and the settings as saved; a line that goes out is
    /// remembered, so the next press neither repeats it nor follows within `comeback::SPACING`.
    /// The shell acts on it: champ select's line goes out through [`Self::send_comeback`], the
    /// game's is typed by the shell itself.
    pub fn comeback_press(&self, all: bool) -> ComebackPress {
        let rule = self.settings().comeback;
        let phase = self.phase();
        let library = if rule.library.enabled {
            self.inner.comeback.library.get()
        } else {
            None
        };
        let pool = comeback::pool(&rule, library.as_deref());
        let now = Instant::now();
        let mut memory = lock(&self.inner.comeback.memory);
        let press = comeback::press(phase, &rule, all, &pool, &memory, now, random());
        if let ComebackPress::ChampSelect(line) | ComebackPress::Game { line, .. } = &press {
            memory.last = Some(line.clone());
            memory.sent_at = Some(now);
        }
        press
    }

    /// Posts `line` in champ select's chat as one message, as the callout's 发送到队伍 does. Returns
    /// the messages sent: one.
    pub async fn send_comeback(&self, line: String) -> Result<u32, CoreError> {
        let client = self.client()?;
        let chat = champ_select_chat(&client.lcu)
            .await?
            .ok_or_else(|| CoreError::Invalid("champ select chat is not open".into()))?;
        say(
            &client.lcu,
            &chat,
            &[line],
            Audience::Team,
            CalloutDelivery::Block,
        )
        .await?;
        Ok(1)
    }

    /// Sends quick phrase `index` (`comeback::phrases`) to champ select's chat for the teammate in
    /// cell `cell_id` of champ select `game_id`: their seat and champion filled in. Refused outside
    /// champ select (`Busy`), and for another champ select, the user's own cell or a cell not of
    /// the user's team (`Invalid`): a menu left open from an earlier champ select sends nothing.
    /// Returns the message sent.
    pub async fn send_phrase(
        &self,
        index: usize,
        game_id: i64,
        cell_id: i64,
    ) -> Result<String, CoreError> {
        let client = self.client()?;
        let text = self.render_phrase(index, game_id, cell_id)?;
        let chat = champ_select_chat(&client.lcu)
            .await?
            .ok_or_else(|| CoreError::Invalid("champ select chat is not open".into()))?;
        say(
            &client.lcu,
            &chat,
            std::slice::from_ref(&text),
            Audience::Team,
            CalloutDelivery::Block,
        )
        .await?;
        info!(index, "sent a quick phrase");
        Ok(text)
    }

    /// The text quick phrase `index` reads for cell `cell_id` of champ select `game_id`.
    fn render_phrase(&self, index: usize, game_id: i64, cell_id: i64) -> Result<String, CoreError> {
        let (phase, view) = {
            let state = lock(&self.inner.state);
            (state.phase, state.champ_select.clone())
        };
        let view = view
            .filter(|_| phase == Phase::ChampSelect)
            .ok_or_else(|| CoreError::Busy("not in champ select".into()))?;
        if view.game_id != game_id {
            return Err(CoreError::Invalid("that champ select is over".into()));
        }
        let (number, seat) = view
            .my_team
            .iter()
            .enumerate()
            .find(|(_, seat)| seat.cell_id == Some(cell_id))
            .ok_or_else(|| CoreError::Invalid("no teammate sits in that cell".into()))?;
        if seat.is_self {
            return Err(CoreError::Invalid("that seat is the user's own".into()));
        }
        let settings = self.settings();
        let language = settings.general.language;
        let phrases = comeback::phrases(&settings.comeback, language);
        let template = phrases
            .get(index)
            .ok_or_else(|| CoreError::Invalid("no such quick phrase".into()))?;
        let champion = self
            .game_data()
            .and_then(|data| {
                data.champions
                    .iter()
                    .find(|champion| champion.id == seat.champion_id)
                    .map(|champion| champion.short_name.clone())
            })
            .unwrap_or_default();
        let name = seat
            .name
            .as_ref()
            .map(|name| name.game_name.clone())
            .unwrap_or_default();
        Ok(comeback::phrase(
            template,
            &callout::seat_label(number + 1, language),
            &champion,
            &name,
        ))
    }

    /// The connected client's process id, for the shell's check of the rights a game it starts
    /// will have.
    pub fn client_pid(&self) -> Option<u32> {
        self.client().ok()?.credentials.pid
    }

    /// The downloaded library as the Comebacks page shows it; answers without a client.
    pub fn comeback_library(&self) -> ComebackLibrary {
        self.inner
            .comeback
            .library
            .get()
            .map(|library| library.status())
            .unwrap_or_default()
    }

    /// Downloads the library (`comeback::library_urls`), checks it is the pinned file and keeps its
    /// lines; only ever on the user's request, after the Comebacks page's confirmation. The switch
    /// that uses it goes off first: every download is opted into again. A failed download changes
    /// nothing.
    pub async fn download_comeback_library(&self) -> Result<ComebackLibrary, CoreError> {
        self.download_library(
            crate::net::web_client().map_err(CoreError::Remote)?,
            &comeback::library_urls(),
            comeback::LIBRARY_SIZE,
            comeback::LIBRARY_SHA256,
        )
        .await
    }

    /// [`Self::download_comeback_library`] from `urls`, for a file of `size` bytes and `sha256`.
    async fn download_library(
        &self,
        client: &reqwest::Client,
        urls: &[String],
        size: usize,
        sha256: &str,
    ) -> Result<ComebackLibrary, CoreError> {
        let _work = self.inner.comeback.library_work.lock().await;
        let library = comeback::download(client, urls, size, sha256, super::now_ms())
            .await
            .map_err(CoreError::Remote)?;
        self.switch_library_off()?;
        let library = self.inner.comeback.library.put(library)?;
        info!(
            mild = library.mild.len(),
            strong = library.strong.len(),
            "downloaded the library"
        );
        Ok(library.status())
    }

    /// Deletes the downloaded library, its switch turned off first; a press after it finds none.
    pub async fn remove_comeback_library(&self) -> Result<ComebackLibrary, CoreError> {
        let _work = self.inner.comeback.library_work.lock().await;
        self.switch_library_off()?;
        self.inner.comeback.library.remove()?;
        info!("removed the downloaded library");
        Ok(ComebackLibrary::default())
    }

    /// Saves the library's switch off, where it is on.
    fn switch_library_off(&self) -> Result<(), CoreError> {
        let mut settings = self.settings();
        if settings.comeback.library.enabled {
            settings.comeback.library.enabled = false;
            self.set_settings(settings)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        thread,
    };

    use lcu::{Credentials, Lcu};
    use tokio::runtime::Handle;

    use super::*;
    use crate::{
        service::{Client, Live},
        view::{ChampSelectView, ChampionInfo, GameData, PlayerStats, RiotId, Seat, TimerView},
    };

    /// The fixture `sqlite.rs` reads: harmless words shaped as the library is.
    const FIXTURE: &[u8] = include_bytes!("../testdata/comeback-library.db");

    fn seat(cell_id: i64, is_self: bool, champion_id: i64, name: Option<&str>) -> Seat {
        Seat {
            puuid: Some(format!("p{cell_id}")),
            name: name.and_then(|name| RiotId::new(name, "1")),
            champion_id,
            intent: false,
            position: None,
            spells: [0, 0],
            is_self,
            premade: None,
            premade_inferred: false,
            autofilled: false,
            note: None,
            stats: PlayerStats::Loading,
            rating: None,
            cell_id: Some(cell_id),
        }
    }

    /// A service in champ select `game_id`: the user in cell 0, a teammate on 龙王 in cell 1, the
    /// other team in cells 5 and 6; a client that never answers (port 1).
    fn in_champ_select(dir: &tempfile::TempDir, game_id: i64) -> Service {
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let credentials = Credentials::new(1, "t");
        let data = GameData {
            champions: vec![ChampionInfo {
                id: 136,
                name: "铸星龙王".into(),
                short_name: "龙王".into(),
                alias: "AurelionSol".into(),
                icon: String::new(),
            }],
            ..GameData::default()
        };
        *service
            .inner
            .client
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Client {
            lcu: Lcu::new(&credentials).unwrap(),
            credentials,
            platform_id: String::new(),
            data: Arc::new(Mutex::new(Some(Arc::new(data)))),
            live: Arc::new(Mutex::new(Live::default())),
        });
        seat_champ_select(&service, game_id);
        service
    }

    /// Puts champ select `game_id` in the service's snapshot. Saving settings draws the views again
    /// from the client's own state, which these tests leave empty: they seat it again after.
    fn seat_champ_select(service: &Service, game_id: i64) {
        let mut state = lock(&service.inner.state);
        state.phase = Phase::ChampSelect;
        state.champ_select = Some(ChampSelectView {
            game_id,
            queue_id: 420,
            timer: TimerView::default(),
            my_team: vec![seat(0, true, 1, Some("Me")), seat(1, false, 136, None)],
            their_team: vec![seat(5, false, 2, None), seat(6, false, 3, None)],
            my_bans: Vec::new(),
            their_bans: Vec::new(),
            bench_enabled: false,
            bench: Vec::new(),
            rerolls_remaining: 0,
            callout: Vec::new(),
            side: None,
            recommendations: Vec::new(),
        });
    }

    #[tokio::test]
    async fn a_phrase_goes_only_to_a_teammate_of_this_champ_select() {
        let dir = tempfile::tempdir().unwrap();
        let service = in_champ_select(&dir, 7);
        assert_eq!(
            service.render_phrase(0, 7, 1).unwrap(),
            "2L大佬，小弟想玩下龙王，给您磕一个了",
            "the teammate's seat in the list and their champion"
        );
        let refused = |result: Result<String, CoreError>| match result {
            Err(CoreError::Invalid(_)) => true,
            other => panic!("not refused as invalid: {other:?}"),
        };
        assert!(
            refused(service.render_phrase(0, 8, 1)),
            "another champ select"
        );
        assert!(
            refused(service.render_phrase(0, 7, 0)),
            "the user's own cell"
        );
        assert!(
            refused(service.render_phrase(0, 7, 5)),
            "the other team's cell"
        );
        assert!(
            refused(service.render_phrase(0, 7, 9)),
            "a cell nobody sits in"
        );
        assert!(refused(service.render_phrase(1, 7, 1)), "no second phrase");
        let mut settings = service.settings();
        settings.comeback.phrases = vec!["{seat}【{name}】，{champion}给我吗".into()];
        service.set_settings(settings).unwrap();
        seat_champ_select(&service, 7);
        assert_eq!(
            service.render_phrase(0, 7, 1).unwrap(),
            "2L，龙王给我吗",
            "a hidden name takes its brackets along"
        );
        // Past the checks, the phrase goes to the client, which here never answers.
        assert!(matches!(
            service.send_phrase(0, 7, 1).await,
            Err(CoreError::Lcu(_))
        ));
        lock(&service.inner.state).phase = Phase::InProgress;
        assert!(matches!(
            service.send_phrase(0, 7, 1).await,
            Err(CoreError::Busy(_))
        ));
    }

    /// Answers `routes` (a path, a status and a body each), one connection at a time, `count`
    /// connections in all; returns the base URL.
    fn serve(routes: Vec<(&'static str, u16, Vec<u8>)>, count: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for _ in 0..count {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let (status, body) = routes
                    .iter()
                    .find(|(route, ..)| *route == path)
                    .map_or((404, &[][..]), |(_, status, body)| {
                        (*status, body.as_slice())
                    });
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        base
    }

    async fn download(service: &Service, urls: &[String]) -> Result<ComebackLibrary, CoreError> {
        service
            .download_library(
                crate::net::client().unwrap(),
                urls,
                FIXTURE.len(),
                &comeback::digest(FIXTURE),
            )
            .await
    }

    fn switch_on(service: &Service) {
        let mut settings = service.settings();
        settings.comeback.library.enabled = true;
        service.set_settings(settings).unwrap();
    }

    #[tokio::test]
    async fn a_download_tries_the_next_mirror_turns_its_switch_off_and_outlives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let base = serve(
            vec![("/refuse", 503, Vec::new()), ("/ok", 200, FIXTURE.to_vec())],
            2,
        );
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        assert!(!service.comeback_library().downloaded, "none before");
        switch_on(&service);
        let status = download(&service, &[format!("{base}/refuse"), format!("{base}/ok")])
            .await
            .unwrap();
        assert!(status.downloaded);
        assert_eq!((status.mild, status.strong), (62, 73));
        assert!(status.fetched_at.is_some());
        assert!(
            !service.settings().comeback.library.enabled,
            "a download is opted into again"
        );
        assert!(dir.path().join("comeback-library.json").exists());
        let restarted = Service::new(dir.path().join("settings.json"), Handle::current());
        assert_eq!(
            restarted.comeback_library(),
            status,
            "read back after a restart"
        );
    }

    #[tokio::test]
    async fn the_library_is_used_only_switched_on_and_never_after_removal() {
        let dir = tempfile::tempdir().unwrap();
        let base = serve(vec![("/ok", 200, FIXTURE.to_vec())], 1);
        let service = in_champ_select(&dir, 7);
        download(&service, &[format!("{base}/ok")]).await.unwrap();
        let skipped = |press: ComebackPress| match press {
            ComebackPress::Skip { reason, .. } => reason,
            other => panic!("not skipped: {other:?}"),
        };
        assert_eq!(
            skipped(service.comeback_press(false)),
            crate::view::CalloutSkip::NoLines,
            "downloaded, not switched on"
        );
        switch_on(&service);
        seat_champ_select(&service, 7);
        let ComebackPress::ChampSelect(line) = service.comeback_press(false) else {
            panic!("a library line for champ select");
        };
        let library = library_now(&service);
        assert!(library.mild.contains(&line), "from the chosen level");
        let removed = service.remove_comeback_library().await.unwrap();
        assert!(!removed.downloaded);
        assert!(
            !service.settings().comeback.library.enabled,
            "switched off first"
        );
        assert!(!dir.path().join("comeback-library.json").exists());
        switch_on(&service);
        seat_champ_select(&service, 7);
        assert_eq!(
            skipped(service.comeback_press(false)),
            crate::view::CalloutSkip::NoLines,
            "switched on again, nothing left to pick"
        );
    }

    fn library_now(service: &Service) -> Arc<Library> {
        service.inner.comeback.library.get().expect("a library")
    }

    #[tokio::test]
    async fn other_bytes_or_more_of_them_are_refused_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut other = FIXTURE.to_vec();
        other[2000] ^= 0xFF;
        let mut more = FIXTURE.to_vec();
        more.push(0);
        let base = serve(vec![("/other", 200, other), ("/more", 200, more)], 2);
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        switch_on(&service);
        let Err(CoreError::Remote(error)) =
            download(&service, &[format!("{base}/other"), format!("{base}/more")]).await
        else {
            panic!("refused");
        };
        assert!(error.contains("not the file winer knows"), "{error}");
        assert!(error.contains("more bytes"), "{error}");
        assert!(!dir.path().join("comeback-library.json").exists());
        assert!(!service.comeback_library().downloaded);
        assert!(
            service.settings().comeback.library.enabled,
            "a failed download leaves the switch as it was"
        );
    }

    /// The real file through the real mirrors: needs the internet, so it runs only when asked
    /// (`cargo test -p winer-core pinned_library -- --ignored`), before a pull request.
    #[tokio::test]
    #[ignore = "downloads from the internet"]
    async fn the_pinned_library_downloads_and_reads() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path().join("settings.json"), Handle::current());
        let status = service.download_comeback_library().await.unwrap();
        let library = library_now(&service);
        println!(
            "mild {} strong {} sha256 {} source {}",
            status.mild, status.strong, library.sha256, library.source
        );
        assert_eq!(library.sha256, comeback::LIBRARY_SHA256);
        assert!(status.mild > 600 && status.strong > 900, "{status:?}");
        assert!(
            library
                .mild
                .iter()
                .chain(&library.strong)
                .all(|line| !line.is_empty() && line.chars().count() <= 200)
        );
    }
}
