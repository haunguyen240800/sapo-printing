use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

use chrono::{DateTime, Local, NaiveDate};
use tracing::Level;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

const LOG_RETENTION_DAYS: u64 = 7;

const LEVELS: [&str; 4] = ["debug", "info", "warn", "error"];

static INIT_ONCE: Once = Once::new();

#[derive(Debug)]
pub enum InitLoggingResult {
    Ok,
    AlreadyInitialized,
    Failed,
}

pub fn init_logging(log_dir: &Path) -> InitLoggingResult {
    let mut result = InitLoggingResult::AlreadyInitialized;

    INIT_ONCE.call_once(|| match init_logging_inner(log_dir) {
        Ok(()) => result = InitLoggingResult::Ok,
        Err(e) => {
            eprintln!("[sapo-printer] Failed to initialize logging: {}", e);
            result = InitLoggingResult::Failed;
        }
    });

    result
}

fn init_logging_inner(log_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(log_dir)
        .map_err(|e| format!("Failed to create log directory {:?}: {}", log_dir, e))?;

    cleanup_old_logs(log_dir, LOG_RETENTION_DAYS);

    let debug_layer = level_file_layer(log_dir, "debug", Level::DEBUG);
    let info_layer = level_file_layer(log_dir, "info", Level::INFO);
    let warn_layer = level_file_layer(log_dir, "warn", Level::WARN);
    let error_layer = level_file_layer(log_dir, "error", Level::ERROR);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .compact()
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(error_layer)
        .with(warn_layer)
        .with(info_layer)
        .with(debug_layer)
        .with(console_layer)
        .init();

    Ok(())
}

fn level_file_layer<S>(log_dir: &Path, level_name: &'static str, level: Level) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let appender = LevelAppender::new(log_dir.to_path_buf(), level_name);
    fmt::layer()
        .json()
        .with_writer(appender)
        .with_ansi(false)
        .with_filter(filter_fn(move |meta| *meta.level() == level))
}

#[derive(Clone)]
struct LevelAppender {
    shared: Arc<Shared>,
}

struct Shared {
    logs_dir: PathBuf,
    level: &'static str,
    state: Mutex<Option<State>>,
}

struct State {
    date: NaiveDate,
    file: File,
}

impl LevelAppender {
    fn new(logs_dir: PathBuf, level: &'static str) -> Self {
        Self {
            shared: Arc::new(Shared {
                logs_dir,
                level,
                state: Mutex::new(None),
            }),
        }
    }
}

impl Shared {
    fn active_path(&self) -> PathBuf {
        self.logs_dir.join(format!("{}.log", self.level))
    }

    fn archive_dir(&self) -> PathBuf {
        self.logs_dir.join(self.level)
    }

    fn archive(&self, active: &Path, date: NaiveDate) -> io::Result<()> {
        let dir = self.archive_dir();
        fs::create_dir_all(&dir)?;

        let stamp = date.format("%Y-%m-%d");
        let mut target = dir.join(format!("{}.{}.log", self.level, stamp));
        let mut n = 1;
        while target.exists() {
            target = dir.join(format!("{}.{}.{}.log", self.level, stamp, n));
            n += 1;
        }
        fs::rename(active, &target)
    }

    fn ensure_current(&self, state: &mut Option<State>) -> io::Result<()> {
        let today = Local::now().date_naive();

        // Roll over an already-open file whose day has passed.
        if let Some(s) = state.as_mut() {
            if s.date != today {
                let old_date = s.date;
                let _ = s.file.flush();
                *state = None; // close the handle before renaming
                let active = self.active_path();
                if active.exists() {
                    self.archive(&active, old_date)?;
                }
            }
        }

        if state.is_none() {
            let active = self.active_path();
            // If a stale file from a previous day already exists on disk, roll it first.
            if active.exists() {
                if let Ok(meta) = fs::metadata(&active) {
                    if let Ok(modified) = meta.modified() {
                        let mdate = DateTime::<Local>::from(modified).date_naive();
                        if mdate < today {
                            self.archive(&active, mdate)?;
                        }
                    }
                }
            }
            let file = OpenOptions::new().create(true).append(true).open(&active)?;
            *state = Some(State { date: today, file });
        }

        Ok(())
    }
}

struct LevelWriter {
    shared: Arc<Shared>,
}

impl Write for LevelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        self.shared.ensure_current(&mut guard)?;
        guard
            .as_mut()
            .expect("state initialized by ensure_current")
            .file
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(s) => s.file.flush(),
            None => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for LevelAppender {
    type Writer = LevelWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LevelWriter {
            shared: self.shared.clone(),
        }
    }
}

fn cleanup_old_logs(log_dir: &Path, retention_days: u64) {
    let cutoff = Local::now().date_naive() - chrono::Duration::days(retention_days as i64);

    let Some(re) = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})").ok() else {
        return;
    };

    for level in LEVELS {
        let dir = log_dir.join(level);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            if let Some(caps) = re.captures(&filename_str) {
                if let Ok(file_date) = NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d") {
                    if file_date < cutoff {
                        match fs::remove_file(&path) {
                            Ok(()) => {
                                eprintln!(
                                    "[sapo-printer] Deleted old log file: {}/{}",
                                    level, filename_str
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "[sapo-printer] Failed to delete old log file {}/{}: {}",
                                    level, filename_str, e
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn test_init_logging_is_idempotent() {
        let log_dir = std::env::temp_dir().join("sapo_init_logging_test");

        let result1 = init_logging(&log_dir);
        assert!(matches!(result1, InitLoggingResult::Ok));

        let result2 = init_logging(&log_dir);
        assert!(matches!(result2, InitLoggingResult::AlreadyInitialized));

        let result3 = init_logging(&log_dir);
        assert!(matches!(result3, InitLoggingResult::AlreadyInitialized));
    }

    #[test]
    fn test_cleanup_old_logs_removes_expired_files() {
        let temp_dir = std::env::temp_dir().join("sapo_log_cleanup_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let info_dir = temp_dir.join("info");
        fs::create_dir_all(&info_dir).unwrap();

        let today = Local::now()
            .date_naive()
            .format("info.%Y-%m-%d.log")
            .to_string();
        let today_path = info_dir.join(&today);
        File::create(&today_path).unwrap();

        let old_date = (Local::now().date_naive() - chrono::Duration::days(10))
            .format("info.%Y-%m-%d.log")
            .to_string();
        let old_path = info_dir.join(&old_date);
        File::create(&old_path).unwrap();

        let other_path = info_dir.join("other_file.txt");
        File::create(&other_path).unwrap();

        cleanup_old_logs(&temp_dir, 7);

        assert!(today_path.exists(), "Today's archive should not be deleted");
        assert!(
            !old_path.exists(),
            "Archive older than 7 days should be deleted"
        );
        assert!(other_path.exists(), "Non-dated files should not be deleted");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_keeps_recent_files() {
        let temp_dir = std::env::temp_dir().join("sapo_log_keep_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let warn_dir = temp_dir.join("warn");
        fs::create_dir_all(&warn_dir).unwrap();

        let recent_date = (Local::now().date_naive() - chrono::Duration::days(3))
            .format("warn.%Y-%m-%d.log")
            .to_string();
        let recent_path = warn_dir.join(&recent_date);
        File::create(&recent_path).unwrap();

        cleanup_old_logs(&temp_dir, 7);

        assert!(
            recent_path.exists(),
            "Archive within 7-day retention should be kept"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_env_filter_parses_rust_log() {
        let test_cases = [
            "debug",
            "info",
            "warn",
            "error",
            "sapo_printer=debug",
            "sapo_printer=info,tracing=warn",
        ];

        for case in test_cases {
            let result = EnvFilter::try_new(case);
            assert!(
                result.is_ok(),
                "EnvFilter should parse '{}', got {:?}",
                case,
                result
            );
        }
    }

    #[test]
    fn test_level_appender_writes_json_to_root_file() {
        use tracing_subscriber::{fmt, layer::SubscriberExt};

        let temp_dir = std::env::temp_dir().join("sapo_level_appender_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let appender = LevelAppender::new(temp_dir.clone(), "info");

        let subscriber = tracing_subscriber::registry().with(
            fmt::layer()
                .json()
                .with_writer(appender)
                .with_ansi(false)
                .with_filter(filter_fn(|meta| *meta.level() == Level::INFO)),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                target = "sapo_printer::test",
                test_field = "test_value",
                "info entry"
            );
            tracing::warn!(target = "sapo_printer::test", "warn entry");
        });

        std::thread::sleep(Duration::from_millis(500));

        let active = temp_dir.join("info.log");
        assert!(
            active.exists(),
            "info.log should be created at the log root"
        );

        let content = fs::read_to_string(&active).unwrap_or_default();
        assert!(
            content.contains("info entry"),
            "info file should contain the info event"
        );
        assert!(
            !content.contains("warn entry"),
            "info file must not contain warn events"
        );
        assert!(
            content.contains("\"timestamp\""),
            "should be JSON with a timestamp field"
        );
        assert!(
            content.contains("\"level\""),
            "should be JSON with a level field"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_old_archived_files_deleted() {
        let temp_dir = std::env::temp_dir().join("sapo_archive_cleanup_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let error_dir = temp_dir.join("error");
        fs::create_dir_all(&error_dir).unwrap();

        for days_ago in 1..=10 {
            let date = (Local::now().date_naive() - chrono::Duration::days(days_ago))
                .format("error.%Y-%m-%d.log")
                .to_string();
            let mut f = File::create(error_dir.join(&date)).unwrap();
            writeln!(f, "log for {days_ago} days ago").unwrap();
        }

        assert_eq!(fs::read_dir(&error_dir).unwrap().count(), 10);

        cleanup_old_logs(&temp_dir, 7);

        assert_eq!(fs::read_dir(&error_dir).unwrap().count(), 7);

        for days_ago in 1..=7 {
            let date = (Local::now().date_naive() - chrono::Duration::days(days_ago))
                .format("error.%Y-%m-%d.log")
                .to_string();
            assert!(
                error_dir.join(&date).exists(),
                "day {days_ago} should be kept"
            );
        }
        for days_ago in 8..=10 {
            let date = (Local::now().date_naive() - chrono::Duration::days(days_ago))
                .format("error.%Y-%m-%d.log")
                .to_string();
            assert!(
                !error_dir.join(&date).exists(),
                "day {days_ago} should be deleted"
            );
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
