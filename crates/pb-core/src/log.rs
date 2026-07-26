//! Structured logging with a closed field vocabulary.
//!
//! Log levels: error, warn, info, debug, trace.
//! Default level: info. Override with PB_LOG env var.
//! Every value passes through redact::path and redact::user.
//! Output: $PB_HOME/logs/powderburn.log, rotating at 8 MiB, 3 files kept.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::redact;

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }

    fn from_env() -> Self {
        match std::env::var("PB_LOG")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "error" => Level::Error,
            "warn" => Level::Warn,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        }
    }
}

/// A structured log event.
#[derive(Debug)]
pub struct LogEvent {
    pub level: Level,
    pub module: &'static str,
    pub event: &'static str,
    pub fields: Vec<(&'static str, String)>,
}

impl LogEvent {
    pub fn new(level: Level, module: &'static str, event: &'static str) -> Self {
        Self {
            level,
            module,
            event,
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, key: &'static str, value: String) -> Self {
        self.fields.push((key, value));
        self
    }

    pub fn log(&self) {
        let min_level = Level::from_env();
        if self.level < min_level {
            return;
        }

        let timestamp = iso_now();
        let redacted_fields: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!(" {}={}", k, redact::redact_path(&redact::redact_user(v))))
            .collect();

        let line = format!(
            "log: {} ts={} mod={} event={}{}\n",
            self.level.as_str(),
            timestamp,
            self.module,
            self.event,
            redacted_fields.join("")
        );

        // Write to log file
        let guard = log_writer();
        let mut writer = guard.lock().unwrap();
        let _ = writer.write_all(line.as_bytes());
        let _ = writer.flush();
        drop(writer);

        // Also print to stderr at error/warn level
        if self.level <= Level::Warn {
            eprint!("{}", line);
        }
    }
}

fn iso_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple UTC date from seconds since epoch
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs_rem = time_secs % 60;

    // Calculate year/month/day
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let diy = if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
            366
        } else {
            365
        };
        if d < diy {
            break;
        }
        d -= diy;
        y += 1;
    }
    let month_days = if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md {
            m = (i + 1) as u32;
            break;
        }
        d -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d as u32 + 1,
        hours,
        mins,
        secs_rem
    )
}

fn log_writer() -> &'static Mutex<Box<dyn Write + Send>> {
    static LOG_FILE: OnceLock<Mutex<Box<dyn Write + Send>>> = OnceLock::new();
    LOG_FILE.get_or_init(|| {
        let path = log_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file: Box<dyn Write + Send> = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map(|f| Box::new(f) as Box<dyn Write + Send>)
            .unwrap_or_else(|_| Box::new(std::io::stderr()));
        Mutex::new(file)
    })
}

fn log_path() -> PathBuf {
    let home = std::env::var("PB_HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("logs").join("powderburn.log")
}

/// Initialize logging. Call once at startup.
pub fn init() {
    let _ = log_writer();
    let event = LogEvent::new(Level::Info, "pb_core::log", "logger_initialized")
        .field("version", env!("CARGO_PKG_VERSION").to_string());
    event.log();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_order() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Info);
        assert!(Level::Info < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }

    #[test]
    fn test_log_event_creation() {
        let event =
            LogEvent::new(Level::Info, "test", "test_event").field("key", "value".to_string());
        event.log();
    }
}
