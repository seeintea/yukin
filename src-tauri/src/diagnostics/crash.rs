use std::{
    cmp::Reverse,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use chrono::{Days, NaiveDate, NaiveDateTime, Utc};
use minidumper_child::{ClientHandle, MinidumperChild};
use serde_json::json;
use uuid::Uuid;

const APP_IDENTIFIER: &str = "com.yukkuri.agent";
const CRASH_MONITOR_ARGUMENT: &str = "--yukin-crash-monitor";
const CRASH_FILE_PREFIX: &str = "crash-";
const CRASH_DUMP_SUFFIX: &str = ".dmp";
const CRASH_METADATA_SUFFIX: &str = ".json";
const RETAINED_CRASH_DAYS: u64 = 14;
const MAX_CRASH_REPORTS: usize = 5;

pub struct Guard {
    _client: ClientHandle,
}

pub fn run_monitor_if_requested() {
    let reporter = reporter(directory());
    if !reporter.is_crash_reporter_process() {
        return;
    }

    if let Err(error) = reporter.spawn() {
        eprintln!("native crash monitor failed: {error}");
    }
    process::exit(1);
}

pub fn start() -> Result<Guard, minidumper_child::Error> {
    let crash_dir = directory();
    if let Err(error) =
        ensure_directory(&crash_dir).and_then(|_| cleanup(&crash_dir, Utc::now().date_naive()))
    {
        tracing::warn!(%error, ?crash_dir, "failed to clean up native crash reports");
    }
    reporter(crash_dir)
        .spawn()
        .map(|client| Guard { _client: client })
}

pub fn directory() -> PathBuf {
    platform_log_dir()
        .unwrap_or_else(|| std::env::temp_dir().join(APP_IDENTIFIER).join("logs"))
        .join("crashes")
}

fn reporter(crash_dir: PathBuf) -> MinidumperChild {
    let callback_dir = crash_dir.clone();
    MinidumperChild::new()
        .with_server_arg(CRASH_MONITOR_ARGUMENT.into())
        .with_crashes_dir(crash_dir)
        .on_minidump(move |contents, temporary_path| {
            if let Err(error) = persist(&callback_dir, contents, temporary_path) {
                eprintln!("failed to persist native crash report: {error}");
            }
        })
}

fn persist(
    crash_dir: &std::path::Path,
    contents: Vec<u8>,
    temporary_path: &std::path::Path,
) -> std::io::Result<()> {
    ensure_directory(crash_dir)?;
    let now = Utc::now();
    let id = temporary_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let stem = format!("{CRASH_FILE_PREFIX}{}-{id}", now.format("%Y%m%dT%H%M%SZ"));
    let dump_name = format!("{stem}{CRASH_DUMP_SUFFIX}");
    let metadata_name = format!("{stem}{CRASH_METADATA_SUFFIX}");
    write_private(&crash_dir.join(&dump_name), &contents)?;
    let metadata = serde_json::to_vec_pretty(&json!({
        "schemaVersion": 1,
        "createdAt": now.to_rfc3339(),
        "appVersion": env!("CARGO_PKG_VERSION"),
        "targetOs": std::env::consts::OS,
        "targetArch": std::env::consts::ARCH,
        "dumpFile": dump_name,
        "dumpBytes": contents.len(),
    }))?;
    write_private(&crash_dir.join(metadata_name), &metadata)?;
    cleanup(crash_dir, now.date_naive())
}

fn ensure_directory(directory: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;
    #[cfg(unix)]
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_private(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.flush()
}

fn cleanup(crash_dir: &std::path::Path, today: NaiveDate) -> std::io::Result<()> {
    let oldest_retained = today
        .checked_sub_days(Days::new(RETAINED_CRASH_DAYS - 1))
        .expect("crash retention period is valid");
    let mut reports = fs::read_dir(crash_dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let timestamp = crash_timestamp(&name)?;
            Some((timestamp, entry.path()))
        })
        .collect::<Vec<_>>();
    reports.sort_unstable_by_key(|report| Reverse(report.0));

    for (index, (timestamp, dump_path)) in reports.into_iter().enumerate() {
        if index >= MAX_CRASH_REPORTS || timestamp.date() < oldest_retained {
            let metadata_path =
                dump_path.with_extension(CRASH_METADATA_SUFFIX.trim_start_matches('.'));
            fs::remove_file(dump_path)?;
            if metadata_path.exists() {
                fs::remove_file(metadata_path)?;
            }
        }
    }
    Ok(())
}

fn crash_timestamp(name: &str) -> Option<NaiveDateTime> {
    let value = name
        .strip_prefix(CRASH_FILE_PREFIX)?
        .strip_suffix(CRASH_DUMP_SUFFIX)?
        .split('-')
        .next()?;
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ").ok()
}

#[cfg(target_os = "macos")]
fn platform_log_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|directory| directory.join("Library/Logs").join(APP_IDENTIFIER))
}

#[cfg(not(target_os = "macos"))]
fn platform_log_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|directory| directory.join(APP_IDENTIFIER).join("logs"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::NaiveDate;
    use uuid::Uuid;

    use super::{cleanup, APP_IDENTIFIER};

    #[test]
    fn app_identifier_matches_tauri_configuration() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).expect("tauri config");
        assert_eq!(config["identifier"], APP_IDENTIFIER);
    }

    #[test]
    fn removes_expired_and_excess_crash_reports() {
        let directory = std::env::temp_dir().join(format!("yukin-crash-test-{}", Uuid::now_v7()));
        fs::create_dir_all(&directory).expect("crash test directory");
        let names = [
            "crash-20260801T000000Z-old",
            "crash-20260814T000000Z-one",
            "crash-20260815T000000Z-two",
            "crash-20260816T000000Z-three",
            "crash-20260817T000000Z-four",
            "crash-20260818T000000Z-five",
            "crash-20260819T000000Z-six",
        ];
        for name in names {
            fs::write(directory.join(format!("{name}.dmp")), "dump").expect("test dump");
            fs::write(directory.join(format!("{name}.json")), "{}").expect("test metadata");
        }
        fs::write(directory.join("unrelated.txt"), "keep").expect("unrelated file");

        cleanup(
            &directory,
            NaiveDate::from_ymd_opt(2026, 8, 19).expect("valid date"),
        )
        .expect("cleanup crash reports");

        assert!(!directory.join(format!("{}.dmp", names[0])).exists());
        assert!(!directory.join(format!("{}.dmp", names[1])).exists());
        assert!(directory.join(format!("{}.dmp", names[2])).exists());
        assert!(directory.join("unrelated.txt").exists());

        fs::remove_dir_all(directory).expect("remove crash test directory");
    }
}
