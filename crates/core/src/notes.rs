//! The user's own notes on other players: a tag and a line of text, kept on this PC only and shown
//! wherever the player appears (champ select, the game, the lobby, their history). A note follows
//! the other player's puuid, so it stays across the user's own accounts. Nothing here ever goes
//! into a chat message.

use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{settings::write_atomic, view::RiotId};

/// Notes kept at most; past it a new player's note is refused rather than an old one dropped.
pub const MAX_NOTES: usize = 5000;
/// Characters in a note's text at most.
pub const MAX_TEXT: usize = 200;

/// The user's verdict on a player, in the window's words 靠谱 / 坑 / 喷子 / 演员.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NoteTag {
    Reliable,
    Weak,
    Toxic,
    Troll,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerNote {
    pub tag: Option<NoteTag>,
    pub text: String,
    /// The Riot ID the player had when the note was last saved, for the list of notes.
    pub name: Option<RiotId>,
    /// Epoch milliseconds.
    pub updated_at: i64,
}

/// A note with the player it is on, for the list of notes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NoteEntry {
    pub puuid: String,
    pub note: PlayerNote,
}

/// Why a note was not saved.
#[derive(Debug, thiserror::Error)]
pub enum NoteError {
    #[error("a note holds {MAX_TEXT} characters at most")]
    TooLong,
    #[error("winer keeps {MAX_NOTES} notes at most; delete some first")]
    Full,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// `notes.json`, beside the settings. Writes are atomic, as the settings' are.
pub struct NoteStore {
    path: PathBuf,
    notes: Mutex<BTreeMap<String, PlayerNote>>,
}

impl NoteStore {
    /// Reads `path`; a missing file is an empty one, and a file that does not parse is moved aside
    /// rather than overwritten.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let notes = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                let aside = path.with_extension("json.invalid");
                tracing::warn!(%error, aside = %aside.display(), "notes file is invalid, starting empty");
                let _ = fs::rename(&path, aside);
                BTreeMap::new()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                tracing::warn!(%error, "notes file is unreadable, starting empty");
                BTreeMap::new()
            }
        };
        Self {
            path,
            notes: Mutex::new(notes),
        }
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, PlayerNote>> {
        self.notes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get(&self, puuid: &str) -> Option<PlayerNote> {
        self.lock().get(puuid).cloned()
    }

    /// Saves the note on `puuid`, the text trimmed. A note with neither a tag nor text is removed,
    /// and `None` returned.
    pub fn set(
        &self,
        puuid: &str,
        tag: Option<NoteTag>,
        text: &str,
        name: Option<RiotId>,
        now: i64,
    ) -> Result<Option<PlayerNote>, NoteError> {
        let text = text.trim();
        if text.chars().count() > MAX_TEXT {
            return Err(NoteError::TooLong);
        }
        let mut notes = self.lock();
        if tag.is_none() && text.is_empty() {
            if notes.contains_key(puuid) {
                let mut kept = notes.clone();
                kept.remove(puuid);
                self.write(&kept)?;
                *notes = kept;
            }
            return Ok(None);
        }
        if !notes.contains_key(puuid) && notes.len() >= MAX_NOTES {
            return Err(NoteError::Full);
        }
        let note = PlayerNote {
            tag,
            text: text.to_owned(),
            name,
            updated_at: now,
        };
        let mut kept = notes.clone();
        kept.insert(puuid.to_owned(), note.clone());
        self.write(&kept)?;
        *notes = kept;
        Ok(Some(note))
    }

    /// Removes the note on `puuid`; returns whether there was one.
    pub fn delete(&self, puuid: &str) -> io::Result<bool> {
        let mut notes = self.lock();
        if !notes.contains_key(puuid) {
            return Ok(false);
        }
        let mut kept = notes.clone();
        kept.remove(puuid);
        self.write(&kept)?;
        *notes = kept;
        Ok(true)
    }

    /// Every note, the newest first.
    pub fn list(&self) -> Vec<NoteEntry> {
        let mut entries: Vec<NoteEntry> = self
            .lock()
            .iter()
            .map(|(puuid, note)| NoteEntry {
                puuid: puuid.clone(),
                note: note.clone(),
            })
            .collect();
        entries.sort_by(|a, b| {
            b.note
                .updated_at
                .cmp(&a.note.updated_at)
                .then_with(|| a.puuid.cmp(&b.puuid))
        });
        entries
    }

    fn write(&self, notes: &BTreeMap<String, PlayerNote>) -> io::Result<()> {
        write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(notes).expect("notes serialize"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(game_name: &str) -> Option<RiotId> {
        RiotId::new(game_name, "1")
    }

    #[test]
    fn notes_round_trip_and_an_empty_one_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        let store = NoteStore::open(&path);
        let saved = store
            .set(
                "p1",
                Some(NoteTag::Weak),
                "  抢龙不看小地图 ",
                name("Ann"),
                5,
            )
            .unwrap()
            .unwrap();
        assert_eq!(saved.text, "抢龙不看小地图", "the text is trimmed");
        store.set("p2", None, "好人", name("Bo"), 9).unwrap();
        let again = NoteStore::open(&path);
        assert_eq!(again.get("p1"), Some(saved));
        assert_eq!(
            again
                .list()
                .iter()
                .map(|entry| entry.puuid.as_str())
                .collect::<Vec<_>>(),
            ["p2", "p1"],
            "the newest first"
        );
        assert_eq!(again.set("p2", None, "   ", None, 10).unwrap(), None);
        assert_eq!(NoteStore::open(&path).get("p2"), None, "removed on disk");
        assert!(again.delete("p1").unwrap());
        assert!(!again.delete("p1").unwrap());
        assert!(NoteStore::open(&path).list().is_empty());
    }

    #[test]
    fn a_long_text_or_a_full_store_is_refused_and_a_bad_file_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        let store = NoteStore::open(&path);
        let long = "字".repeat(MAX_TEXT + 1);
        assert!(matches!(
            store.set("p", None, &long, None, 1),
            Err(NoteError::TooLong)
        ));
        assert!(
            store
                .set("p", None, &"字".repeat(MAX_TEXT), None, 1)
                .is_ok(),
            "characters, not bytes"
        );
        {
            let mut notes = store.lock();
            for index in 0..MAX_NOTES - 1 {
                notes.insert(
                    format!("x{index}"),
                    PlayerNote {
                        tag: None,
                        text: "t".into(),
                        name: None,
                        updated_at: 0,
                    },
                );
            }
        }
        assert!(matches!(
            store.set("new", Some(NoteTag::Toxic), "", None, 2),
            Err(NoteError::Full)
        ));
        assert!(
            store.set("p", Some(NoteTag::Toxic), "", None, 2).is_ok(),
            "a note already kept can still change"
        );

        fs::write(&path, b"{ not json").unwrap();
        assert!(NoteStore::open(&path).list().is_empty());
        assert!(path.with_extension("json.invalid").exists());
    }
}
