//! Persistent file logging with dual output (stderr + daily log file).

use crate::paths;
use std::fs;
use std::path::PathBuf;

/// Returns the log directory path: `~/.local/share/lcars-voice/logs/`
pub fn get_log_dir() -> PathBuf {
    paths::app_data_dir().join("logs")
}

/// Stderr dispatch: `[LCARS] [INFO] message`
fn stderr_dispatch() -> fern::Dispatch {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!("[LCARS] [{}] {}", record.level(), message))
        })
        .chain(std::io::stderr())
}

/// Base dispatch with log levels and third-party filters.
fn base_dispatch() -> fern::Dispatch {
    fern::Dispatch::new()
        .level(log::LevelFilter::Debug)
        .level_for("reqwest", log::LevelFilter::Warn)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
}

/// Initialize logging with dual output: stderr and a daily log file.
///
/// - Stderr format: `[LCARS] [INFO] message`
/// - File format: `2026-03-01 14:30:45.123 [INFO] [lcars_voice] message`
///
/// Must be called before any `log` macros are used.
pub fn init_logging() {
    let log_dir = get_log_dir();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("[LCARS] Failed to create log directory {:?}: {}", log_dir, e);
        init_stderr_only();
        return;
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_file_path = log_dir.join(format!("lcars-voice-{}.log", today));

    let file = match fern::log_file(&log_file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[LCARS] Failed to open log file {:?}: {}", log_file_path, e);
            init_stderr_only();
            return;
        }
    };

    let file_dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                message
            ))
        })
        .chain(file);

    let result = base_dispatch()
        .chain(file_dispatch)
        .chain(stderr_dispatch())
        .apply();

    if let Err(e) = result {
        eprintln!("[LCARS] Failed to initialize logging: {}", e);
    }

    // Clean up old log files on a background thread (non-blocking)
    std::thread::spawn(move || cleanup_old_logs(&log_dir, 14));
}

/// Fallback: stderr-only logging when file logging fails.
fn init_stderr_only() {
    let _ = base_dispatch().chain(stderr_dispatch()).apply();
}

/// Delete `.log` files in `log_dir` older than `max_age_days` days.
fn cleanup_old_logs(log_dir: &std::path::Path, max_age_days: u64) {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);

    let entries = match fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                if modified < cutoff {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}
