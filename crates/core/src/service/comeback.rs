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
