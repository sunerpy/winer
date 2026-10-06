//! The log: one file per day (UTC, as its timestamps) under the app's log directory. It is what a
//! user attaches to a bug report, and how the in-client plugin is diagnosed (its log lines arrive
//! over the bridge and land here too).
//!
//! It is bounded three ways: the last [`KEEP_DAYS`] days, [`TOTAL_BYTES`] in all, and
//! [`FILE_BYTES`] a file, past which the day goes on in a file of its own
//! (`winer.2026-10-06.log`, then `winer.2026-10-06.1.log`, …). Past a limit the oldest files go
//! first; the one being written never does. The limits are enforced when winer starts, whenever a
//! file is begun, and once a day while it runs (`maintain`), a day without a line included.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use winer_core::view::DiskUse;

/// Days of log kept, today included: a week covers the problem a user reports days later.
pub(crate) const KEEP_DAYS: u32 = 7;
/// The whole log at most. A day of ordinary use writes well under a megabyte (15 KB over the first
/// two days on the owner's PC), so this leaves room for a week of a noisy one.
pub(crate) const TOTAL_BYTES: u64 = 50 * 1024 * 1024;
/// One file at most: a line repeating in a loop fills it in hours, and the day goes on in the next
/// file while the oldest go, so the newest lines are always kept.
pub(crate) const FILE_BYTES: u64 = 10 * 1024 * 1024;

const PREFIX: &str = "winer.";
const SUFFIX: &str = ".log";
/// Between two looks at whether a day has passed since the last pruning.
const MAINTAIN_EVERY: Duration = Duration::from_secs(60 * 60);

/// Keeps the writer thread alive; dropping it flushes and stops logging.
pub(crate) struct LogGuard(#[allow(dead_code)] WorkerGuard);

/// Sends every log line to the files in `logs`' directory, from a worker thread.
pub(crate) fn init(logs: &Logs) -> Result<LogGuard, Box<dyn std::error::Error>> {
    let (writer, guard) = tracing_appender::non_blocking(Writer(logs.clone()));
    let filter = EnvFilter::try_from_env("WINER_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,winer=debug,winer_core=debug,lcu=debug"));
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(false).with_writer(writer));
    #[cfg(debug_assertions)]
    let registry = registry.with(fmt::layer().with_writer(std::io::stderr));
    registry.try_init()?;
    Ok(LogGuard(guard))
}

/// The log's files, shared by the writer and the shell (the daily pruning, the cleanup).
#[derive(Clone)]
pub(crate) struct Logs {
    dir: PathBuf,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    file: Option<File>,
    /// The file being written.
    name: Option<Name>,
    bytes: u64,
    /// The day the files were last pruned on.
    pruned: Option<i64>,
}

impl Logs {
    /// The log in `dir`, its files already within the limits.
    pub(crate) fn open(dir: &Path) -> io::Result<Self> {
        Self::open_on(dir, today())
    }

    fn open_on(dir: &Path, today: i64) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let logs = Self {
            dir: dir.to_owned(),
            state: Arc::default(),
        };
        logs.prune(&mut logs.lock(), today);
        Ok(logs)
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Prunes the files once a day has passed since they last were, checking every hour, for as
    /// long as winer runs.
    pub(crate) fn maintain(&self) {
        let logs = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(MAINTAIN_EVERY).await;
                logs.maintain_on(today());
            }
        });
    }

    /// Every log file but the one being written goes. Returns what went.
    pub(crate) fn remove_old(&self) -> DiskUse {
        let state = self.lock();
        let mut went = DiskUse::default();
        for file in list(&self.dir) {
            if Some(file.name) != state.name && fs::remove_file(&file.path).is_ok() {
                went.files += 1;
                went.bytes += file.bytes;
            }
        }
        went
    }

    fn maintain_on(&self, today: i64) {
        let mut state = self.lock();
        if state.pruned != Some(today) {
            self.prune(&mut state, today);
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Removes the files past the limits, never the one being written.
    fn prune(&self, state: &mut State, today: i64) {
        for file in surplus(list(&self.dir), today, state.name) {
            // A file that cannot go now (another program has it open) goes at the next pruning.
            let _ = fs::remove_file(&file.path);
        }
        state.pruned = Some(today);
    }

    /// Writes `line` on `today`, beginning the next file first when the day changed or the file is
    /// full. A line is never split between two files.
    fn write_on(&self, line: &[u8], today: i64) -> io::Result<()> {
        let mut state = self.lock();
        let full = state.bytes > 0 && state.bytes + line.len() as u64 > FILE_BYTES;
        let stale = state.name.is_none_or(|name| name.day != today);
        if state.file.is_none() || full || stale {
            let filled = state.name.filter(|_| full && !stale);
            let (name, bytes) = next_file(&list(&self.dir), today, filled);
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.dir.join(name.file_name()))
                .or_else(|_| {
                    // The folder went (a cleanup tool): make it again.
                    fs::create_dir_all(&self.dir)?;
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(self.dir.join(name.file_name()))
                })?;
            *state = State {
                file: Some(file),
                name: Some(name),
                bytes,
                pruned: state.pruned,
            };
            self.prune(&mut state, today);
        }
        let file = state.file.as_mut().expect("a file was just opened");
        file.write_all(line)?;
        state.bytes += line.len() as u64;
        Ok(())
    }
}

/// What the worker thread writes through.
struct Writer(Logs);

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_on(buf, today())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.lock().file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// Today, as days since 1970-01-01 in UTC.
fn today() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| (since.as_secs() / 86_400) as i64)
}

/// A log file's name: its day, as days since 1970-01-01, and its part of that day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Name {
    day: i64,
    part: u32,
}

impl Name {
    /// `winer.2026-10-06.log` is the day's first part, `winer.2026-10-06.2.log` its third.
    fn parse(file_name: &str) -> Option<Self> {
        let stem = file_name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
        let (date, part) = match stem.split_once('.') {
            Some((date, part)) => (date, part.parse().ok().filter(|&part| part > 0)?),
            None => (stem, 0),
        };
        let mut fields = date.split('-');
        let mut field = |len: usize| {
            fields
                .next()
                .filter(|field| {
                    field.len() == len && field.bytes().all(|byte| byte.is_ascii_digit())
                })?
                .parse::<i64>()
                .ok()
        };
        let (year, month, day) = (field(4)?, field(2)?, field(2)?);
        if fields.next().is_some() {
            return None;
        }
        let days = days_from_civil(year, month, day);
        // A date that does not exist (2026-02-30) does not come back the same.
        (civil_from_days(days) == (year, month, day)).then_some(Self { day: days, part })
    }

    fn file_name(self) -> String {
        let (year, month, day) = civil_from_days(self.day);
        match self.part {
            0 => format!("{PREFIX}{year:04}-{month:02}-{day:02}{SUFFIX}"),
            part => format!("{PREFIX}{year:04}-{month:02}-{day:02}.{part}{SUFFIX}"),
        }
    }
}

/// A log file in the directory.
#[derive(Debug)]
struct Found {
    name: Name,
    path: PathBuf,
    bytes: u64,
}

/// The log files in `dir`, oldest first. Anything else there is left alone.
fn list(dir: &Path) -> Vec<Found> {
    let mut found: Vec<Found> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = Name::parse(entry.file_name().to_str()?)?;
            // Not the listing's own metadata: on Windows its size is updated only when a file is
            // closed, so the file being written would read as empty.
            let metadata = fs::symlink_metadata(entry.path())
                .ok()
                .filter(fs::Metadata::is_file)?;
            Some(Found {
                name,
                path: entry.path(),
                bytes: metadata.len(),
            })
        })
        .collect();
    found.sort_by_key(|file| file.name);
    found
}

/// The file to write next on `today`, and what it holds already: today's last part while it has
/// room (winer restarted), else a part after it. `filled` is a file that has just become full.
fn next_file(files: &[Found], today: i64, filled: Option<Name>) -> (Name, u64) {
    let last = files.iter().rfind(|file| file.name.day == today);
    match (last, filled) {
        (Some(last), None) if last.bytes < FILE_BYTES => (last.name, last.bytes),
        (last, filled) => {
            let after = last
                .map(|file| file.name.part)
                .max(filled.map(|name| name.part));
            let part = after.map_or(0, |part| part + 1);
            (Name { day: today, part }, 0)
        }
    }
}

/// The files past the limits on `today`, `files` oldest first: those of days before the last
/// [`KEEP_DAYS`], then the oldest while the rest come to more than [`TOTAL_BYTES`]. Never `active`.
fn surplus(files: Vec<Found>, today: i64, active: Option<Name>) -> Vec<Found> {
    let first_kept = today - i64::from(KEEP_DAYS) + 1;
    let (mut going, kept): (Vec<Found>, Vec<Found>) = files
        .into_iter()
        .partition(|file| file.name.day < first_kept && Some(file.name) != active);
    let mut total: u64 = kept.iter().map(|file| file.bytes).sum();
    for file in kept {
        if total <= TOTAL_BYTES {
            break;
        }
        if Some(file.name) != active {
            total -= file.bytes;
            going.push(file);
        }
    }
    going
}

/// The day counted from 1970-01-01 of a date of the proleptic Gregorian calendar (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date of a day counted from 1970-01-01 (`civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-06.
    const TODAY: i64 = 20_732;
    const MB: u64 = 1024 * 1024;

    fn name(day: i64, part: u32) -> Name {
        Name { day, part }
    }

    /// A file of `bytes` for `day`'s `part` in `dir`.
    fn put(dir: &Path, day: i64, part: u32, bytes: u64) {
        let file = File::create(dir.join(name(day, part).file_name())).unwrap();
        file.set_len(bytes).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        list(dir)
            .into_iter()
            .map(|file| file.name.file_name())
            .collect()
    }

    #[test]
    fn file_names_carry_the_day_and_its_part_and_nothing_else_is_taken_for_one() {
        assert_eq!(Name::parse("winer.2026-10-06.log"), Some(name(TODAY, 0)));
        assert_eq!(Name::parse("winer.2026-10-06.3.log"), Some(name(TODAY, 3)));
        assert_eq!(Name::parse("winer.1970-01-01.log"), Some(name(0, 0)));
        assert_eq!(Name::parse("winer.2000-02-29.log"), Some(name(11_016, 0)));
        for other in [
            "winer.2026-02-30.log",
            "winer.2026-10-06.0.log",
            "winer.2026-10-6.log",
            "winer.2026-10-06.txt",
            "winer.2026-10-06.x.log",
            "other.2026-10-06.log",
            "winer.2026-10-06-01.log",
            "winer.log",
        ] {
            assert_eq!(Name::parse(other), None, "{other}");
        }
        for day in [0, 11_016, TODAY, TODAY + 365 * 400] {
            for part in [0, 1, 12] {
                let named = name(day, part);
                assert_eq!(Name::parse(&named.file_name()), Some(named));
            }
        }
        assert_eq!(name(TODAY, 2).file_name(), "winer.2026-10-06.2.log");
    }

    #[test]
    fn files_of_days_before_the_last_seven_go_and_then_the_oldest_past_fifty_megabytes() {
        let dir = tempfile::tempdir().unwrap();
        for back in 0..10 {
            put(dir.path(), TODAY - back, 0, MB);
        }
        // Foreign files stay, whatever their name says.
        fs::write(dir.path().join("winer.txt"), "notes").unwrap();
        let logs = Logs::open_on(dir.path(), TODAY).unwrap();
        assert_eq!(names(dir.path()).len(), 7);
        assert_eq!(names(dir.path())[0], "winer.2026-09-30.log");
        assert!(dir.path().join("winer.txt").exists());

        // Six full files today: the total passes fifty megabytes and the oldest go, today's
        // first part among them, but never the file being written.
        for part in 1..6 {
            put(dir.path(), TODAY, part, FILE_BYTES);
        }
        logs.lock().name = Some(name(TODAY - 6, 0));
        logs.prune(&mut logs.lock(), TODAY);
        let left = list(dir.path());
        let total: u64 = left.iter().map(|file| file.bytes).sum();
        assert!(total <= TOTAL_BYTES + MB, "{total}");
        assert_eq!(
            left.first().map(|file| file.name),
            Some(name(TODAY - 6, 0)),
            "the file being written stays, oldest or not"
        );
        assert_eq!(left.last().map(|file| file.name), Some(name(TODAY, 5)));
        assert!(!dir.path().join(name(TODAY - 5, 0).file_name()).exists());
    }

    #[test]
    fn a_full_file_goes_on_in_the_next_part_and_a_new_day_in_a_file_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let logs = Logs::open_on(dir.path(), TODAY).unwrap();
        let mut line = vec![b'x'; 1024 * 1024 - 1];
        line.push(b'\n');
        for _ in 0..10 {
            logs.write_on(&line, TODAY).unwrap();
        }
        assert_eq!(names(dir.path()), ["winer.2026-10-06.log"]);
        // The eleventh megabyte would pass the cap: it begins the next part, whole.
        logs.write_on(&line, TODAY).unwrap();
        assert_eq!(
            names(dir.path()),
            ["winer.2026-10-06.log", "winer.2026-10-06.1.log"]
        );
        assert_eq!(list(dir.path())[1].bytes, line.len() as u64);

        logs.write_on(b"tomorrow\n", TODAY + 1).unwrap();
        assert_eq!(names(dir.path()).last().unwrap(), "winer.2026-10-07.log");

        // winer restarted the same day: it goes on in the day's last file while that has room.
        let again = Logs::open_on(dir.path(), TODAY + 1).unwrap();
        again.write_on(b"again\n", TODAY + 1).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(name(TODAY + 1, 0).file_name())).unwrap(),
            "tomorrow\nagain\n"
        );
        drop(again);
        // A full last file is not appended to.
        put(dir.path(), TODAY + 2, 0, FILE_BYTES);
        let full = Logs::open_on(dir.path(), TODAY + 2).unwrap();
        full.write_on(b"next\n", TODAY + 2).unwrap();
        assert_eq!(names(dir.path()).last().unwrap(), "winer.2026-10-08.1.log");
    }

    #[test]
    fn the_daily_pruning_needs_no_line_and_runs_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let logs = Logs::open_on(dir.path(), TODAY).unwrap();
        logs.write_on(b"line\n", TODAY).unwrap();
        put(dir.path(), TODAY - 6, 0, 10);
        logs.maintain_on(TODAY);
        assert!(
            dir.path().join(name(TODAY - 6, 0).file_name()).exists(),
            "pruned already today"
        );
        logs.maintain_on(TODAY + 1);
        assert!(!dir.path().join(name(TODAY - 6, 0).file_name()).exists());
        assert_eq!(names(dir.path()), ["winer.2026-10-06.log"]);
    }

    #[test]
    fn a_cleanup_removes_every_file_but_the_one_being_written() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), TODAY - 2, 0, 300);
        put(dir.path(), TODAY - 1, 0, 200);
        let logs = Logs::open_on(dir.path(), TODAY).unwrap();
        logs.write_on(b"line\n", TODAY).unwrap();
        assert_eq!(
            logs.remove_old(),
            DiskUse {
                files: 2,
                bytes: 500
            }
        );
        assert_eq!(names(dir.path()), ["winer.2026-10-06.log"]);
        logs.write_on(b"more\n", TODAY).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(name(TODAY, 0).file_name())).unwrap(),
            "line\nmore\n"
        );
    }
}
