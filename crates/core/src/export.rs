//! Files the window exports (match history as CSV or JSON), written into a folder the shell names,
//! the user's Downloads. The window chooses the name's stem; the extension, the size and what may
//! be in a name are decided here, and an existing file is never overwritten.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use ts_rs::TS;

use crate::CoreError;

/// An export's contents at most, in bytes of UTF-8.
pub const MAX_EXPORT_BYTES: usize = 10 * 1024 * 1024;
/// Characters in a name's stem at most.
pub const MAX_STEM: usize = 80;
/// `name (2)` … `name (99)` are tried after `name` itself.
const MAX_COPIES: u32 = 99;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}

/// Names Windows keeps for devices, with or without an extension.
const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// The stem, trimmed, when it can name a file in the folder and nothing else: no separator, no
/// character Windows refuses, no device name, no dot or space at the end.
pub fn check_stem(stem: &str) -> Result<&str, CoreError> {
    let stem = stem.trim();
    let invalid = |why: &str| Err(CoreError::Invalid(format!("not a file name: {why}")));
    let length = stem.chars().count();
    if length == 0 || length > MAX_STEM {
        return invalid("empty or too long");
    }
    if stem.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return invalid("a character Windows refuses");
    }
    if stem == "." || stem == ".." || stem.ends_with('.') || stem.ends_with(' ') {
        return invalid("dots or spaces at the end");
    }
    let base = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .trim_end()
        .to_ascii_uppercase();
    let numbered = |prefix: &str| {
        base.strip_prefix(prefix)
            .is_some_and(|digit| digit.len() == 1 && matches!(digit.as_bytes()[0], b'1'..=b'9'))
    };
    if RESERVED.contains(&base.as_str()) || numbered("COM") || numbered("LPT") {
        return invalid("a device name");
    }
    Ok(stem)
}

/// Writes `contents` as `<stem>.<extension>` in `dir`, or as `<stem> (2)` and so on when that one
/// is taken; a CSV starts with a byte order mark, so Excel reads it as UTF-8. Returns the path
/// written. A file left half written by a failure is removed.
pub fn write_export(
    dir: &Path,
    stem: &str,
    format: ExportFormat,
    contents: &str,
) -> Result<PathBuf, CoreError> {
    let stem = check_stem(stem)?;
    if contents.len() > MAX_EXPORT_BYTES {
        return Err(CoreError::Invalid(format!(
            "an export holds {} MB at most",
            MAX_EXPORT_BYTES / 1024 / 1024
        )));
    }
    let extension = format.extension();
    for copy in 1..=MAX_COPIES {
        let name = if copy == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({copy}).{extension}")
        };
        let path = dir.join(name);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let written = (|| -> io::Result<()> {
            if format == ExportFormat::Csv {
                file.write_all("\u{feff}".as_bytes())?;
            }
            file.write_all(contents.as_bytes())?;
            file.sync_all()
        })();
        if let Err(error) = written {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(error.into());
        }
        return Ok(path);
    }
    Err(CoreError::Invalid(format!(
        "{stem}.{extension} and {MAX_COPIES} numbered copies are all taken"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stem_names_a_file_in_the_folder_and_nothing_else() {
        assert_eq!(
            check_stem("  winer 战绩 Ann#1 2026-10-08 ").unwrap(),
            "winer 战绩 Ann#1 2026-10-08"
        );
        for bad in [
            "",
            "   ",
            "a/b",
            "a\\b",
            "..",
            ".",
            "name.",
            "a:b",
            "a*b",
            "a?b",
            "a\"b",
            "a|b",
            "a<b",
            "tab\there",
            "CON",
            "con",
            "nul.csv",
            "COM1",
            "lpt9",
            "Aux ",
        ] {
            assert!(check_stem(bad).is_err(), "{bad:?}");
        }
        assert!(check_stem("COM0").is_ok(), "only COM1 to COM9 are devices");
        assert!(check_stem("CONSOLE").is_ok());
        assert!(check_stem(&"字".repeat(MAX_STEM)).is_ok());
        assert!(check_stem(&"字".repeat(MAX_STEM + 1)).is_err());
    }

    #[test]
    fn an_export_never_overwrites_and_a_csv_starts_with_a_bom() {
        let dir = tempfile::tempdir().unwrap();
        let first = write_export(dir.path(), "games", ExportFormat::Csv, "a,b\n").unwrap();
        assert_eq!(first, dir.path().join("games.csv"));
        assert_eq!(fs::read(&first).unwrap(), "\u{feff}a,b\n".as_bytes());
        let second = write_export(dir.path(), "games", ExportFormat::Csv, "c\n").unwrap();
        assert_eq!(second, dir.path().join("games (2).csv"));
        assert_eq!(
            fs::read(&first).unwrap(),
            "\u{feff}a,b\n".as_bytes(),
            "the first is left as it was"
        );
        let json = write_export(dir.path(), "games", ExportFormat::Json, "[]").unwrap();
        assert_eq!(json, dir.path().join("games.json"));
        assert_eq!(fs::read_to_string(&json).unwrap(), "[]", "no mark in JSON");

        for copy in 3..=MAX_COPIES {
            fs::write(dir.path().join(format!("games ({copy}).csv")), b"").unwrap();
        }
        assert!(write_export(dir.path(), "games", ExportFormat::Csv, "x").is_err());

        let big = "x".repeat(MAX_EXPORT_BYTES + 1);
        assert!(write_export(dir.path(), "big", ExportFormat::Json, &big).is_err());
        assert!(!dir.path().join("big.json").exists());
        assert!(write_export(dir.path(), "../escape", ExportFormat::Json, "[]").is_err());
    }
}
