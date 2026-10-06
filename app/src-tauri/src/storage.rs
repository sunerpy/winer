//! What winer keeps on disk, for 设置 › 关于, and the cleanup there (`docs/site/privacy.md` lists
//! the same): the log (`logging.rs`, which also bounds it), the WebView's folder, the game settings
//! backups, Pengu Loader, the settings, the updater's installers in the temporary folder, and the
//! core's caches in memory (`winer_core`'s `service/caches.rs`).
//!
//! A cleanup removes only what is safe to: caches, old logs and spent installers, never a backup,
//! the settings or the remembered setups. The WebView's caches are in use while its window is
//! open, so a cleanup marks them, and they go when winer next starts, before the WebView does.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use tracing::{info, warn};
use winer_core::{
    Service, backup,
    view::{CleanupReport, DiskUse, StorageLimits, StorageReport},
};

use crate::{
    logging::{self, Logs},
    window,
};

/// Under the app's local data: the game settings backups (the core's `set_backup_dir`) and the
/// Pengu Loader winer manages (`plugin_host`).
pub(crate) const BACKUPS: &str = "game-settings";
pub(crate) const PENGU: &str = "pengu";
/// The WebView's folder, which WebView2 makes under the app's local data.
const WEBVIEW: &str = "EBWebView";
/// The caches in it, which Chromium rebuilds as it needs them: pages, compiled scripts and the
/// GPU's shaders. What the WebView keeps for the window (its local storage holds a few of the
/// window's choices) and what WebView2 downloads and updates itself (its components, most of the
/// folder) stay.
const WEBVIEW_CACHES: &[&str] = &[
    "Default/Cache",
    "Default/Code Cache",
    "Default/GPUCache",
    "Default/DawnGraphiteCache",
    "Default/DawnWebGPUCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "ShaderCache",
];
/// Left by a cleanup: the WebView's caches go at the next start.
const CLEAR_MARK: &str = "clear-webview-cache";
/// The updater writes each installer to `<temp>\winer-<version>-updater-<random>\` and runs it as
/// winer exits, so nothing removes it afterwards (`tauri-plugin-updater`).
const UPDATER_DIR: (&str, &str) = ("winer-", "-updater-");
const INSTALLER: (&str, &[&str]) = ("winer-", &["-installer.exe", "-installer.msi"]);

#[derive(Clone)]
pub(crate) struct Storage {
    logs: Logs,
    /// `%LOCALAPPDATA%\app.winer.desktop`: the WebView's folder, the backups and Pengu Loader.
    local: PathBuf,
    /// `%APPDATA%\app.winer.desktop`: the settings and the remembered setups.
    config: PathBuf,
    /// The system's temporary folder.
    temp: PathBuf,
}

impl Storage {
    pub(crate) fn new(logs: Logs, local: PathBuf, config: PathBuf, temp: PathBuf) -> Self {
        Self {
            logs,
            local,
            config,
            temp,
        }
    }

    /// Before the window's WebView starts: the caches the last cleanup marked go, and so do the
    /// installers of updates that have been installed since.
    pub(crate) fn at_start(&self) {
        let mark = self.local.join(CLEAR_MARK);
        if mark.exists() {
            let (went, left) = clear(&self.local.join(WEBVIEW));
            // Anything still held (a WebView of the last run not gone yet) goes at the next start.
            if left == 0
                && let Err(error) = fs::remove_file(&mark)
            {
                warn!(%error, "the WebView cache mark was not removed");
            }
            info!(
                files = went.files,
                bytes = went.bytes,
                left,
                "WebView caches cleared"
            );
        }
        let went = remove_installers(&self.temp);
        if went.files > 0 {
            info!(
                files = went.files,
                bytes = went.bytes,
                "update installers removed"
            );
        }
    }

    pub(crate) fn report(&self, service: &Service) -> StorageReport {
        let webview = self.local.join(WEBVIEW);
        StorageReport {
            logs: usage(self.logs.dir()),
            webview: usage(&webview),
            webview_cache: cache_usage(&webview),
            webview_clear_pending: self.local.join(CLEAR_MARK).exists(),
            backups: usage(&self.local.join(BACKUPS)),
            pengu: usage(&self.local.join(PENGU)),
            settings: usage(&self.config),
            updates: installers(&self.temp)
                .iter()
                .fold(DiskUse::default(), |sum, (_, size)| add(sum, *size)),
            memory: service.memory_use(),
            limits: StorageLimits {
                log_days: logging::KEEP_DAYS,
                log_bytes: logging::TOTAL_BYTES,
                log_file_bytes: logging::FILE_BYTES,
                webview_cache_bytes: window::DISK_CACHE_BYTES,
                backups: backup::KEEP as u32,
                image_bytes: service.image_limit(),
            },
        }
    }

    /// Removes the old logs, the spent installers and what is cached in memory, and marks the
    /// WebView's caches to go at the next start. Returns what went, or goes.
    pub(crate) fn clean(&self, service: &Service) -> io::Result<CleanupReport> {
        let logs = self.logs.remove_old();
        let updates = remove_installers(&self.temp);
        let webview_cache = cache_usage(&self.local.join(WEBVIEW));
        if webview_cache.files > 0 {
            fs::create_dir_all(&self.local)?;
            fs::write(self.local.join(CLEAR_MARK), b"")?;
        }
        let memory = service.clear_caches();
        info!(
            logs = logs.bytes,
            updates = updates.bytes,
            webview = webview_cache.bytes,
            memory = memory.entries,
            "cleanup"
        );
        Ok(CleanupReport {
            logs,
            updates,
            webview_cache,
            memory,
        })
    }
}

fn add(a: DiskUse, b: DiskUse) -> DiskUse {
    DiskUse {
        files: a.files.saturating_add(b.files),
        bytes: a.bytes.saturating_add(b.bytes),
    }
}

/// The files under `path`, however deep; links are not followed, and what cannot be read is
/// passed over.
fn usage(path: &Path) -> DiskUse {
    let mut total = DiskUse::default();
    let mut pending = vec![path.to_owned()];
    while let Some(next) = pending.pop() {
        let Ok(metadata) = fs::symlink_metadata(&next) else {
            continue;
        };
        if metadata.is_file() {
            total = add(
                total,
                DiskUse {
                    files: 1,
                    bytes: metadata.len(),
                },
            );
        } else if metadata.is_dir()
            && let Ok(entries) = fs::read_dir(&next)
        {
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    total
}

/// The WebView's caches in its folder `webview`.
fn cache_usage(webview: &Path) -> DiskUse {
    WEBVIEW_CACHES
        .iter()
        .map(|cache| usage(&webview.join(cache)))
        .fold(DiskUse::default(), add)
}

/// Removes the WebView's caches from its folder `webview`. Returns what went and how many files
/// could not go.
fn clear(webview: &Path) -> (DiskUse, u32) {
    let (mut went, mut left) = (DiskUse::default(), 0);
    for cache in WEBVIEW_CACHES {
        let dir = webview.join(cache);
        let before = usage(&dir);
        if before.files == 0 && !dir.exists() {
            continue;
        }
        if let Err(error) = fs::remove_dir_all(&dir)
            && error.kind() != io::ErrorKind::NotFound
        {
            warn!(%error, cache, "a WebView cache was not removed whole");
        }
        let after = usage(&dir);
        went = add(
            went,
            DiskUse {
                files: before.files.saturating_sub(after.files),
                bytes: before.bytes.saturating_sub(after.bytes),
            },
        );
        left += after.files;
    }
    (went, left)
}

/// The updater's installers in the temporary folder `temp`, each with its size.
fn installers(temp: &Path) -> Vec<(PathBuf, DiskUse)> {
    let (dir_prefix, dir_infix) = UPDATER_DIR;
    let (file_prefix, file_suffixes) = INSTALLER;
    let mut found = Vec::new();
    for dir in fs::read_dir(temp).into_iter().flatten().flatten() {
        let name = dir.file_name();
        let Some(name) = name.to_str() else { continue };
        let ours = name.starts_with(dir_prefix) && name.contains(dir_infix);
        if !ours || !dir.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        for file in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let file_name = file.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let installer = file_name.starts_with(file_prefix)
                && file_suffixes
                    .iter()
                    .any(|suffix| file_name.ends_with(suffix));
            if let Ok(metadata) = file.metadata()
                && installer
                && metadata.is_file()
            {
                let size = DiskUse {
                    files: 1,
                    bytes: metadata.len(),
                };
                found.push((file.path(), size));
            }
        }
    }
    found
}

/// Removes the updater's installers, and their folders once empty. One still running (the update
/// that just started this winer) cannot go yet and is left for the next start.
fn remove_installers(temp: &Path) -> DiskUse {
    let mut went = DiskUse::default();
    for (path, size) in installers(temp) {
        if fs::remove_file(&path).is_ok() {
            went = add(went, size);
            if let Some(dir) = path.parent() {
                // Fails while anything else is in it, which then stays.
                let _ = fs::remove_dir(dir);
            }
        }
    }
    went
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        storage: Storage,
        local: PathBuf,
        temp: PathBuf,
        logs: PathBuf,
    }

    fn put(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    /// What winer leaves after a day, in the layout measured on a Windows install.
    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let local = root.path().join("local");
        let (config, temp, logs) = (
            root.path().join("config"),
            root.path().join("temp"),
            local.join("logs"),
        );
        let log = Logs::open(&logs).unwrap();
        // An earlier day's log, named after the day it was written.
        put(&logs.join("winer.2026-10-05.log"), 300);
        let webview = local.join(WEBVIEW);
        put(&webview.join("Default/Cache/Cache_Data/data_0"), 100);
        put(&webview.join("Default/Code Cache/js/index"), 40);
        put(&webview.join("GrShaderCache/data_1"), 60);
        put(
            &webview.join("Default/Local Storage/leveldb/000003.log"),
            50,
        );
        put(&webview.join("WidevineCdm/4.10/manifest.json"), 1000);
        put(&local.join(BACKUPS).join("game-settings-1.json"), 20);
        put(&local.join(PENGU).join("core.dll"), 30);
        put(&config.join("settings.json"), 10);
        put(&config.join("loadouts.json"), 5);
        put(
            &temp.join("winer-0.0.3-updater-AbC123/winer-0.0.3-installer.exe"),
            500,
        );
        // Not the updater's: another program's folder, and a file of the updater's folder that is
        // no installer.
        put(&temp.join("winer-notes/winer-0.0.3-installer.exe"), 7);
        put(&temp.join("winer-0.0.2-updater-Xyz789/readme.txt"), 9);
        let storage = Storage::new(log, local.clone(), config, temp.clone());
        Fixture {
            root,
            storage,
            local,
            temp,
            logs,
        }
    }

    /// A service with no client, whose own files live apart from the fixture's.
    fn service(fixture: &Fixture) -> Service {
        Service::new(
            fixture.root.path().join("service").join("settings.json"),
            tauri::async_runtime::handle().inner().clone(),
        )
    }

    fn used(files: u32, bytes: u64) -> DiskUse {
        DiskUse { files, bytes }
    }

    #[test]
    fn the_report_sizes_each_thing_winer_keeps_with_its_limits() {
        let fixture = fixture();
        let report = fixture.storage.report(&service(&fixture));
        assert_eq!(report.logs, used(1, 300));
        assert_eq!(report.webview, used(5, 1250));
        assert_eq!(report.webview_cache, used(3, 200));
        assert!(!report.webview_clear_pending);
        assert_eq!(report.backups, used(1, 20));
        assert_eq!(report.pengu, used(1, 30));
        assert_eq!(report.settings, used(2, 15));
        assert_eq!(report.updates, used(1, 500));
        assert_eq!(report.memory.entries, 0);
        assert_eq!(
            report.limits,
            StorageLimits {
                log_days: 7,
                log_bytes: 50 * 1024 * 1024,
                log_file_bytes: 10 * 1024 * 1024,
                webview_cache_bytes: 32 * 1024 * 1024,
                backups: 10,
                image_bytes: 32 * 1024 * 1024,
            }
        );
    }

    #[test]
    fn a_cleanup_takes_caches_logs_and_installers_and_leaves_what_the_user_made() {
        let fixture = fixture();
        let service = service(&fixture);
        let report = fixture.storage.clean(&service).unwrap();
        assert_eq!(report.logs, used(1, 300));
        assert_eq!(report.updates, used(1, 500));
        assert_eq!(
            report.webview_cache,
            used(3, 200),
            "marked, to go at the next start"
        );
        assert!(!fixture.temp.join("winer-0.0.3-updater-AbC123").exists());
        assert!(
            fixture
                .temp
                .join("winer-notes/winer-0.0.3-installer.exe")
                .exists()
        );
        assert!(
            fixture
                .temp
                .join("winer-0.0.2-updater-Xyz789/readme.txt")
                .exists()
        );

        let after = fixture.storage.report(&service);
        assert_eq!(after.logs, used(0, 0));
        assert!(after.webview_clear_pending);
        assert_eq!(after.webview_cache, used(3, 200), "still in use until then");
        assert_eq!(
            (after.backups, after.pengu, after.settings),
            (used(1, 20), used(1, 30), used(2, 15))
        );

        // The next start: the caches go before the WebView starts, and only they.
        fixture.storage.at_start();
        let next = fixture.storage.report(&service);
        assert_eq!(next.webview_cache, used(0, 0));
        assert_eq!(
            next.webview,
            used(2, 1050),
            "local storage and WebView2's components"
        );
        assert!(!next.webview_clear_pending);
        assert!(fixture.logs.exists());
        let webview = fixture.local.join(WEBVIEW);
        assert!(
            webview
                .join("Default/Local Storage/leveldb/000003.log")
                .exists()
        );
        assert!(webview.join("WidevineCdm/4.10/manifest.json").exists());
    }

    #[test]
    fn a_start_without_a_cleanup_leaves_the_webview_alone_and_removes_spent_installers() {
        let fixture = fixture();
        fixture.storage.at_start();
        let report = fixture.storage.report(&service(&fixture));
        assert_eq!(report.webview_cache, used(3, 200));
        assert_eq!(report.updates, used(0, 0));
        assert_eq!(report.logs, used(1, 300), "the log keeps its own limits");
    }

    #[test]
    fn nothing_there_is_nothing_used() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(usage(&root.path().join("missing")), DiskUse::default());
        assert!(installers(&root.path().join("missing")).is_empty());
        assert_eq!(clear(&root.path().join("missing")), (DiskUse::default(), 0));
    }
}
