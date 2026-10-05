//! One log file per day under the app's log directory, a week kept. It is what a user attaches
//! to a bug report, and how the in-client plugin is diagnosed (its log lines arrive over the
//! bridge and land here too).

use std::path::Path;

use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};

/// Keeps the writer thread alive; dropping it flushes and stops logging.
pub(crate) struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub(crate) fn init(dir: &Path) -> Result<LogGuard, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let appender = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("winer")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
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
