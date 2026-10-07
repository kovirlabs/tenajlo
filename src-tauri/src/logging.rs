//! Log files (spec §9): `$DATA/logs/tenajlo.<date>.log`, one per day, the last 7 kept.
//!
//! Every log site must already pass URLs and git output through `redact()` (CLAUDE.md rule 3);
//! the file gets exactly what the console gets, without colour codes.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer as _;
use tracing_subscriber::{fmt, EnvFilter};

/// How many daily log files to keep.
pub const KEEP_DAYS: usize = 7;

/// Keeps the background log writer alive; dropping it flushes and stops file logging.
pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

/// Logs to the console and to daily files in `log_dir`. If the folder can't be used, logs to
/// the console only (and says so). Call once, at startup.
pub fn init(log_dir: &Path) -> Option<LogGuard> {
    let filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("tenajlo_lib=info"));
    let console = fmt::layer().with_filter(filter());
    match appender(log_dir) {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let file = fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_filter(filter());
            tracing_subscriber::registry()
                .with(console)
                .with(file)
                .init();
            Some(LogGuard(guard))
        }
        Err(e) => {
            tracing_subscriber::registry().with(console).init();
            tracing::error!(error = %e, dir = %log_dir.display(), "log files unavailable");
            None
        }
    }
}

/// Daily `tenajlo.<date>.log` files; older ones beyond [`KEEP_DAYS`] are deleted on rotation.
fn appender(log_dir: &Path) -> Result<RollingFileAppender, tracing_appender::rolling::InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("tenajlo")
        .filename_suffix("log")
        .max_log_files(KEEP_DAYS)
        .build(log_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn writes_dated_files_in_the_log_folder() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let mut appender = appender(&logs).unwrap();
        writeln!(appender, "hello").unwrap();
        appender.flush().unwrap();
        let names: Vec<String> = std::fs::read_dir(&logs)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(
            names[0].starts_with("tenajlo.") && names[0].ends_with(".log"),
            "{names:?}"
        );
    }
}
