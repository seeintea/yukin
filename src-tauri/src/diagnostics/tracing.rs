use std::{backtrace::Backtrace, fs, panic, path::Path, thread};

use chrono::{Days, NaiveDate, Utc};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

const LOG_FILE_PREFIX: &str = "yukin.";
const LOG_FILE_SUFFIX: &str = ".log";
const RETAINED_LOG_DAYS: u64 = 14;

pub struct Guard {
    _file_writer: Option<WorkerGuard>,
}

pub fn init(log_dir: Option<&Path>) -> Guard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let fallback = if cfg!(debug_assertions) {
            "warn,yukin=debug"
        } else {
            "warn,yukin=info"
        };
        EnvFilter::new(fallback)
    });

    let Some(log_dir) = log_dir else {
        init_console(filter);
        install_panic_hook();
        return Guard { _file_writer: None };
    };

    let cleanup_error = fs::create_dir_all(log_dir)
        .and_then(|_| cleanup_expired_logs(log_dir, Utc::now().date_naive()))
        .err();
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX.trim_end_matches('.'))
        .filename_suffix(LOG_FILE_SUFFIX.trim_start_matches('.'))
        .build(log_dir);

    match appender {
        Ok(appender) => {
            let (file_writer, file_guard) = tracing_appender::non_blocking(appender);
            let console_layer = tracing_subscriber::fmt::layer();
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer);
            Registry::default()
                .with(filter)
                .with(console_layer)
                .with(file_layer)
                .init();
            install_panic_hook();

            if let Some(error) = cleanup_error {
                tracing::warn!(%error, ?log_dir, "failed to clean up expired log files");
            }
            tracing::info!(
                ?log_dir,
                retained_days = RETAINED_LOG_DAYS,
                "file logging initialized"
            );
            Guard {
                _file_writer: Some(file_guard),
            }
        }
        Err(error) => {
            init_console(filter);
            install_panic_hook();
            tracing::error!(%error, ?log_dir, "failed to initialize file logging; using console logging only");
            Guard { _file_writer: None }
        }
    }
}

fn install_panic_hook() {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let message = panic_info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| {
                panic_info
                    .payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
            })
            .unwrap_or("non-string panic payload");
        let location = panic_info.location();
        let current_thread = thread::current();
        let thread_name = current_thread.name().unwrap_or("unnamed");
        let backtrace = Backtrace::force_capture();

        tracing::error!(
            panic.message = message,
            panic.file = location.map(|value| value.file()),
            panic.line = location.map(|value| value.line()),
            panic.column = location.map(|value| value.column()),
            panic.thread = thread_name,
            %backtrace,
            "rust panic"
        );
        previous_hook(panic_info);
    }));
}

fn init_console(filter: EnvFilter) {
    Registry::default()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn cleanup_expired_logs(log_dir: &Path, today: NaiveDate) -> std::io::Result<()> {
    let oldest_retained = today
        .checked_sub_days(Days::new(RETAINED_LOG_DAYS - 1))
        .expect("log retention period is valid");

    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if log_date(&path).is_some_and(|date| date < oldest_retained) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn log_date(path: &Path) -> Option<NaiveDate> {
    let name = path.file_name()?.to_str()?;
    let value = name
        .strip_prefix(LOG_FILE_PREFIX)?
        .strip_suffix(LOG_FILE_SUFFIX)?;
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::NaiveDate;
    use uuid::Uuid;

    use super::cleanup_expired_logs;

    #[test]
    fn removes_only_yukin_logs_older_than_retention_window() {
        let directory = std::env::temp_dir().join(format!("yukin-log-test-{}", Uuid::now_v7()));
        fs::create_dir_all(&directory).expect("test log directory");
        for name in [
            "yukin.2026-08-05.log",
            "yukin.2026-08-06.log",
            "yukin.2026-08-19.log",
            "other.2026-01-01.log",
            "yukin.invalid.log",
        ] {
            fs::write(directory.join(name), "log").expect("test log file");
        }

        cleanup_expired_logs(
            &directory,
            NaiveDate::from_ymd_opt(2026, 8, 19).expect("valid date"),
        )
        .expect("cleanup logs");

        assert!(!directory.join("yukin.2026-08-05.log").exists());
        assert!(directory.join("yukin.2026-08-06.log").exists());
        assert!(directory.join("yukin.2026-08-19.log").exists());
        assert!(directory.join("other.2026-01-01.log").exists());
        assert!(directory.join("yukin.invalid.log").exists());

        fs::remove_dir_all(directory).expect("remove test log directory");
    }
}
