//! Snapshots of the game's own settings (`/lol-game-settings/v1/…`): what one holds, what its file
//! is called, which to keep and what restoring it sends. The files themselves are the service's.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Snapshots kept; making one more removes the oldest.
pub const KEEP: usize = 10;
/// A snapshot is a few kilobytes; anything near this is not one.
pub const MAX_BYTES: usize = 1 << 20;

const PREFIX: &str = "game-settings-";
const EXTENSION: &str = ".json";
/// Written into every snapshot, so an import can tell one from any other JSON file.
const FORMAT: &str = "winer-game-settings";
const VERSION: u32 = 1;

pub const GAME_SETTINGS: &str = "/lol-game-settings/v1/game-settings";
pub const INPUT_SETTINGS: &str = "/lol-game-settings/v1/input-settings";
/// Writes what was patched to the game's own files.
pub const SAVE: &str = "/lol-game-settings/v1/save";

/// One half of the game's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BackupChannel {
    /// `game-settings`: interface, camera, sound, display (常规设置).
    General,
    /// `input-settings`: the key bindings (按键设置).
    Hotkeys,
}

impl BackupChannel {
    pub fn path(self) -> &'static str {
        match self {
            Self::General => GAME_SETTINGS,
            Self::Hotkeys => INPUT_SETTINGS,
        }
    }
}

/// A snapshot as the window lists it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// The name of its file, and the order it was made in.
    pub id: i64,
    /// Epoch milliseconds when the settings were read from the client.
    pub taken_at: i64,
    /// Bytes on disk.
    pub size: u64,
    pub channels: Vec<BackupChannel>,
}

/// A snapshot's file: the two documents exactly as the client sent them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub format: String,
    pub version: u32,
    pub taken_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_settings: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_settings: Option<Value>,
}

impl BackupFile {
    pub fn new(taken_at: i64, game_settings: Value, input_settings: Value) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            taken_at,
            game_settings: Some(game_settings),
            input_settings: Some(input_settings),
        }
    }

    pub fn channels(&self) -> Vec<BackupChannel> {
        let mut channels = Vec::with_capacity(2);
        if self.game_settings.is_some() {
            channels.push(BackupChannel::General);
        }
        if self.input_settings.is_some() {
            channels.push(BackupChannel::Hotkeys);
        }
        channels
    }

    fn document(&self, channel: BackupChannel) -> Option<&Value> {
        match channel {
            BackupChannel::General => self.game_settings.as_ref(),
            BackupChannel::Hotkeys => self.input_settings.as_ref(),
        }
    }

    pub fn info(&self, id: i64, size: u64) -> BackupInfo {
        BackupInfo {
            id,
            taken_at: self.taken_at,
            size,
            channels: self.channels(),
        }
    }

    /// The `PATCH` requests that put `channels` back, in the order given, without repeats; an
    /// error names a channel the snapshot does not hold.
    pub fn patches(
        &self,
        channels: &[BackupChannel],
    ) -> Result<Vec<(&'static str, &Value)>, String> {
        let mut patches: Vec<(&'static str, &Value)> = Vec::with_capacity(channels.len());
        for &channel in channels {
            if patches.iter().any(|(path, _)| *path == channel.path()) {
                continue;
            }
            let document = self
                .document(channel)
                .ok_or_else(|| format!("the snapshot holds no {channel:?} settings"))?;
            patches.push((channel.path(), document));
        }
        if patches.is_empty() {
            return Err("choose the settings to restore".into());
        }
        Ok(patches)
    }
}

/// Reads a snapshot, one of winer's own or one brought in from elsewhere: it must say it is one,
/// and each half it holds must be a JSON object, as the client's documents are.
pub fn parse(text: &str) -> Result<BackupFile, String> {
    if text.len() > MAX_BYTES {
        return Err("the file is too large to be a settings backup".into());
    }
    let file: BackupFile = serde_json::from_str(text)
        .map_err(|error| format!("not a winer settings backup: {error}"))?;
    if file.format != FORMAT {
        return Err("not a winer settings backup".into());
    }
    if file.version > VERSION {
        return Err(format!(
            "the backup was made by a newer winer (format {})",
            file.version
        ));
    }
    let objects = [&file.game_settings, &file.input_settings];
    if objects.iter().all(|document| document.is_none()) {
        return Err("the backup holds no settings".into());
    }
    if objects
        .iter()
        .any(|document| document.as_ref().is_some_and(|value| !value.is_object()))
    {
        return Err("the backup's settings are not the client's".into());
    }
    Ok(file)
}

/// The file a snapshot is kept in.
pub fn file_name(id: i64) -> String {
    format!("{PREFIX}{id}{EXTENSION}")
}

/// The snapshot a file in the folder holds, or `None` for any other file.
pub fn id_of(file_name: &str) -> Option<i64> {
    file_name
        .strip_prefix(PREFIX)?
        .strip_suffix(EXTENSION)?
        .parse()
        .ok()
        .filter(|id| *id > 0)
}

/// An id for a snapshot made at `now`: later than every one there is, so two made in the same
/// millisecond still get a file each, and the newest always sorts last.
pub fn fresh_id(now: i64, taken: &[i64]) -> i64 {
    taken
        .iter()
        .max()
        .map_or(now, |last| now.max(last + 1))
        .max(1)
}

/// The snapshots to delete so that `KEEP` remain: the oldest ones.
pub fn surplus(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.into_iter().skip(KEEP).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;

    fn snapshot() -> BackupFile {
        BackupFile::new(
            1_791_316_800_000,
            fixture("live/profile/game-settings.json"),
            fixture("live/profile/input-settings.json"),
        )
    }

    #[test]
    fn a_snapshot_names_its_file_by_id_and_nothing_else_parses_as_one() {
        assert_eq!(
            file_name(1_791_316_800_000),
            "game-settings-1791316800000.json"
        );
        assert_eq!(
            id_of("game-settings-1791316800000.json"),
            Some(1_791_316_800_000)
        );
        assert_eq!(id_of("game-settings-1791316800000.json.tmp"), None);
        assert_eq!(id_of("settings.json"), None);
        assert_eq!(id_of("game-settings-x.json"), None);
        assert_eq!(id_of("game-settings--5.json"), None);
    }

    #[test]
    fn ids_only_grow_and_the_oldest_go_beyond_ten() {
        assert_eq!(fresh_id(100, &[]), 100);
        assert_eq!(fresh_id(100, &[40, 100]), 101, "same millisecond");
        assert_eq!(fresh_id(100, &[250]), 251, "a clock set back");
        let ids: Vec<i64> = (1..=13).collect();
        assert_eq!(surplus(ids), vec![3, 2, 1]);
        assert_eq!(surplus(vec![5, 1, 9]), Vec::<i64>::new());
    }

    #[test]
    fn a_snapshot_round_trips_and_lists_both_halves() {
        let file = snapshot();
        let text = serde_json::to_string_pretty(&file).unwrap();
        let back = parse(&text).unwrap();
        assert_eq!(back, file);
        assert_eq!(
            back.info(7, text.len() as u64),
            BackupInfo {
                id: 7,
                taken_at: 1_791_316_800_000,
                size: text.len() as u64,
                channels: vec![BackupChannel::General, BackupChannel::Hotkeys],
            }
        );
        assert_eq!(
            back.game_settings.as_ref().unwrap()["HUD"]["ChatScale"],
            63,
            "the client's own document, untouched"
        );
    }

    #[test]
    fn restoring_patches_the_chosen_halves_once_each() {
        let file = snapshot();
        let patches = file
            .patches(&[
                BackupChannel::Hotkeys,
                BackupChannel::General,
                BackupChannel::Hotkeys,
            ])
            .unwrap();
        assert_eq!(
            patches.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
            vec![INPUT_SETTINGS, GAME_SETTINGS]
        );
        assert_eq!(patches[0].1["GameEvents"]["evtCastSpell1"], "[q]");
        assert!(file.patches(&[]).is_err());

        let general_only = BackupFile {
            input_settings: None,
            ..file
        };
        assert_eq!(general_only.channels(), vec![BackupChannel::General]);
        assert!(general_only.patches(&[BackupChannel::Hotkeys]).is_err());
    }

    #[test]
    fn an_import_must_be_a_snapshot_of_the_clients_documents() {
        assert!(parse("{}").is_err());
        assert!(parse("[1, 2]").is_err());
        assert!(
            parse(r#"{"General": {"WindowMode": 0}}"#).is_err(),
            "a bare document"
        );
        assert!(
            parse(r#"{"format": "winer-game-settings", "version": 1, "takenAt": 1}"#).is_err(),
            "nothing in it"
        );
        assert!(
            parse(
                r#"{"format": "winer-game-settings", "version": 1, "takenAt": 1, "gameSettings": [1]}"#
            )
            .is_err()
        );
        assert!(
            parse(
                r#"{"format": "winer-game-settings", "version": 9, "takenAt": 1, "gameSettings": {}}"#
            )
            .is_err(),
            "a newer format"
        );
        let hotkeys = parse(
            r#"{"format": "winer-game-settings", "version": 1, "takenAt": 5, "inputSettings": {"GameEvents": {}}}"#,
        )
        .unwrap();
        assert_eq!(hotkeys.channels(), vec![BackupChannel::Hotkeys]);
        assert!(parse(&" ".repeat(MAX_BYTES + 1)).is_err());
    }
}
