//! Offline observability primitives for the CLI and app leaf layers.
//!
//! This module deliberately lives outside the deterministic kernel boundary:
//! it owns wall-clock, environment, and filesystem interaction.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::args::Args;

const DEFAULT_ROTATE_BYTES: u64 = 8 * 1024 * 1024;

/// Short source revision embedded by this leaf crate's build script.
pub const BUILD_HASH: &str = env!("PB_BUILD_HASH");

/// Stable log severity vocabulary from SPEC-007.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

/// Closed field vocabulary. Callers cannot invent arbitrary field names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogField {
    Actor,
    Command,
    Event,
    Outcome,
    Scenario,
    Seed,
    Tick,
}

impl LogField {
    fn as_str(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::Command => "command",
            Self::Event => "event",
            Self::Outcome => "outcome",
            Self::Scenario => "scenario",
            Self::Seed => "seed",
            Self::Tick => "tick",
        }
    }
}

/// Required immutable fields carried by every log record and crash report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMetadata {
    pub build: String,
    pub ruleset: String,
    pub content: String,
}

impl BuildMetadata {
    /// Keep only the prefixes prescribed by SPEC-007.
    pub fn new(build: &str, ruleset: &str, content: &str) -> Self {
        Self {
            build: prefix(build, 12),
            ruleset: prefix(ruleset, 8),
            content: prefix(content, 8),
        }
    }
}

fn prefix(value: &str, chars: usize) -> String {
    value.chars().take(chars).collect()
}

/// Parsed `PB_LOG` filter with an INFO default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFilter {
    default: LogLevel,
    targets: BTreeMap<String, LogLevel>,
}

impl LogFilter {
    pub fn parse(directives: Option<&str>) -> Self {
        let mut filter = Self {
            default: LogLevel::Info,
            targets: BTreeMap::new(),
        };
        let Some(directives) = directives else {
            return filter;
        };
        for directive in directives
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            if let Some((target, level)) = directive.split_once('=') {
                if let Some(level) = LogLevel::parse(level) {
                    filter.targets.insert(target.trim().to_string(), level);
                }
            } else if let Some(level) = LogLevel::parse(directive) {
                filter.default = level;
            }
        }
        filter
    }

    fn enabled(&self, target: &str, level: LogLevel) -> bool {
        let threshold = self
            .targets
            .iter()
            .filter(|(candidate, _)| {
                target == candidate.as_str() || target.starts_with(&format!("{candidate}::"))
            })
            .max_by_key(|(candidate, _)| candidate.len())
            .map_or(self.default, |(_, threshold)| *threshold);
        level <= threshold
    }
}

/// Structured file-and-stderr logger with bounded rotation.
#[derive(Debug)]
pub struct Logger {
    path: PathBuf,
    install_root: PathBuf,
    config_root: PathBuf,
    metadata: BuildMetadata,
    filter: LogFilter,
    forbidden_values: Vec<String>,
    rotate_bytes: u64,
    lock: Mutex<()>,
}

impl Logger {
    pub fn new(
        config_root: &Path,
        install_root: &Path,
        metadata: BuildMetadata,
        filter: LogFilter,
        forbidden_values: Vec<String>,
    ) -> Self {
        Self {
            path: config_root.join("log/powderburn.log"),
            install_root: install_root.to_path_buf(),
            config_root: config_root.to_path_buf(),
            metadata,
            filter,
            forbidden_values,
            rotate_bytes: DEFAULT_ROTATE_BYTES,
            lock: Mutex::new(()),
        }
    }

    /// Override the rotation threshold for deterministic tests.
    pub fn with_rotation_limit(mut self, bytes: u64) -> Self {
        self.rotate_bytes = bytes.max(1);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write one record. Battle records should supply scenario, tick, and seed.
    pub fn log(
        &self,
        unix_seconds: i64,
        level: LogLevel,
        target: &str,
        fields: &[(LogField, String)],
        message: &str,
    ) -> Result<bool, String> {
        if !self.filter.enabled(target, level) {
            return Ok(false);
        }
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "E-LOG-LOCK: logger lock poisoned".to_string())?;
        let target = token_value(target);
        let message = self.sanitize(message);
        let mut line = format!(
            "{} {} {} build={} ruleset={} content={}",
            iso8601_utc(unix_seconds),
            level.as_str(),
            target,
            token_value(&self.metadata.build),
            token_value(&self.metadata.ruleset),
            token_value(&self.metadata.content)
        );
        for (field, value) in fields {
            line.push(' ');
            line.push_str(field.as_str());
            line.push('=');
            line.push_str(&token_value(&self.sanitize(value)));
        }
        line.push_str(" msg=\"");
        line.push_str(&quoted_value(&message));
        line.push_str("\"\n");

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("E-LOG-WRITE: cannot create log directory: {error}"))?;
        }
        self.rotate_if_needed(u64::try_from(line.len()).unwrap_or(u64::MAX))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("E-LOG-WRITE: cannot open log: {error}"))?;
        file.write_all(line.as_bytes())
            .map_err(|error| format!("E-LOG-WRITE: cannot append log: {error}"))?;
        eprint!("{line}");
        Ok(true)
    }

    fn rotate_if_needed(&self, incoming: u64) -> Result<(), String> {
        let current = fs::metadata(&self.path).map_or(0, |metadata| metadata.len());
        if current.saturating_add(incoming) <= self.rotate_bytes {
            return Ok(());
        }
        let second = self.path.with_extension("log.2");
        let first = self.path.with_extension("log.1");
        if second.exists() {
            fs::remove_file(&second)
                .map_err(|error| format!("E-LOG-ROTATE: cannot remove old log: {error}"))?;
        }
        if first.exists() {
            fs::rename(&first, &second)
                .map_err(|error| format!("E-LOG-ROTATE: cannot rotate backup: {error}"))?;
        }
        if self.path.exists() {
            fs::rename(&self.path, &first)
                .map_err(|error| format!("E-LOG-ROTATE: cannot rotate active log: {error}"))?;
        }
        Ok(())
    }

    pub fn sanitize(&self, text: &str) -> String {
        let mut value = text
            .replace(&self.config_root.to_string_lossy().to_string(), "<config>")
            .replace(
                &self.install_root.to_string_lossy().to_string(),
                "<install>",
            );
        for forbidden in &self.forbidden_values {
            if forbidden.len() >= 4 {
                value = value.replace(forbidden, "<redacted>");
            }
        }
        redact_absolute_tokens(&value)
    }
}

fn token_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':' | '/')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn quoted_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn redact_absolute_tokens(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let trimmed = word.trim_matches(|character: char| {
                matches!(character, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
            });
            let windows_absolute = trimmed.len() >= 3
                && trimmed.as_bytes().get(1) == Some(&b':')
                && matches!(trimmed.as_bytes().get(2), Some(b'\\' | b'/'));
            if (trimmed.starts_with('/') || windows_absolute)
                && !trimmed.starts_with("<config>")
                && !trimmed.starts_with("<install>")
            {
                word.replace(trimmed, "<path>")
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert Unix seconds to `YYYY-MM-DDTHH:MM:SSZ` without reading a clock.
pub fn iso8601_utc(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

/// Inputs for the self-contained crash reproduction bundle.
#[derive(Debug, Clone)]
pub struct CrashBundle<'a> {
    pub timestamp: i64,
    pub panic_message: &'a str,
    pub backtrace: &'a str,
    pub metadata: &'a BuildMetadata,
    pub events: &'a [String],
    pub seed: u64,
    pub scenario: &'a str,
    pub terminal_tick: u64,
    pub journal: &'a str,
    pub state_hash: &'a str,
}

/// Write the report plus deterministic reproduction triple.
pub fn write_crash_bundle(
    config_root: &Path,
    install_root: &Path,
    forbidden_values: &[String],
    bundle: &CrashBundle<'_>,
) -> Result<PathBuf, String> {
    let logger = Logger::new(
        config_root,
        install_root,
        bundle.metadata.clone(),
        LogFilter::parse(None),
        forbidden_values.to_vec(),
    );
    let stamp = iso8601_utc(bundle.timestamp).replace([':', '-'], "");
    let directory = config_root.join("crash").join(stamp);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("E-CRASH-WRITE: cannot create crash directory: {error}"))?;

    let events = bundle
        .events
        .iter()
        .rev()
        .take(200)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| logger.sanitize(line))
        .collect::<Vec<_>>()
        .join("\n");
    let report = format!(
        "panic: {}\nbacktrace:\n{}\nbuild: {}\nruleset: {}\ncontent: {}\nevents:\n{}\n",
        logger.sanitize(bundle.panic_message),
        logger.sanitize(bundle.backtrace),
        token_value(&bundle.metadata.build),
        token_value(&bundle.metadata.ruleset),
        token_value(&bundle.metadata.content),
        events
    );
    fs::write(directory.join("report.txt"), report)
        .map_err(|error| format!("E-CRASH-WRITE: cannot write report: {error}"))?;
    fs::write(directory.join("journal.jrnl"), bundle.journal)
        .map_err(|error| format!("E-CRASH-WRITE: cannot write journal: {error}"))?;
    fs::write(
        directory.join("meta.toml"),
        format!(
            "seed = {}\nscenario = \"{}\"\nterminal_tick = {}\ncontent = \"{}\"\nruleset = \"{}\"\nversion = \"{}\"\ncommit = \"{}\"\n",
            bundle.seed,
            token_value(bundle.scenario),
            bundle.terminal_tick,
            token_value(&bundle.metadata.content),
            token_value(&bundle.metadata.ruleset),
            env!("CARGO_PKG_VERSION"),
            token_value(&bundle.metadata.build)
        ),
    )
    .map_err(|error| format!("E-CRASH-WRITE: cannot write metadata: {error}"))?;
    fs::write(
        directory.join("state.hash"),
        format!("{}\n", token_value(bundle.state_hash)),
    )
    .map_err(|error| format!("E-CRASH-WRITE: cannot write state hash: {error}"))?;
    Ok(directory)
}

/// Current Unix time. Wall-clock access stays outside the kernel crates.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

pub fn config_root() -> PathBuf {
    std::env::var_os("PB_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".pbcache/config"))
}

pub fn install_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn forbidden_environment_values() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(_, value)| value.into_string().ok())
        .filter(|value| value.len() >= 4)
        .collect()
}

pub fn metadata(content_root: &Path) -> BuildMetadata {
    let ruleset = pb_content::hash::ruleset_hash(content_root)
        .map(|hash| pb_content::hash::hex(&hash))
        .unwrap_or_else(|_| "unknown".to_string());
    let content = pb_content::hash::content_hash(content_root)
        .map(|hash| pb_content::hash::hex(&hash))
        .unwrap_or_else(|_| "unknown".to_string());
    BuildMetadata::new(BUILD_HASH, &ruleset, &content)
}

pub fn logger_for(args: &Args) -> Logger {
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    Logger::new(
        &config_root(),
        &install_root(),
        metadata(content_root),
        LogFilter::parse(std::env::var("PB_LOG").ok().as_deref()),
        forbidden_environment_values(),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "powderburn-observability-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root must be creatable");
        root
    }

    #[test]
    fn unix_epoch_is_valid_iso_utc() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn filter_selects_longest_target_directive() {
        let filter = LogFilter::parse(Some("warn,pb_sim=debug,pb_sim::shot=trace"));
        assert!(!filter.enabled("pb_render", LogLevel::Info));
        assert!(filter.enabled("pb_sim", LogLevel::Debug));
        assert!(filter.enabled("pb_sim::shot", LogLevel::Trace));
    }

    #[test]
    fn logger_rotates_and_redacts() {
        let root = test_root("rotation");
        let install = root.join("install");
        let config = root.join("config");
        let logger = Logger::new(
            &config,
            &install,
            BuildMetadata::new(
                "abcdef012345",
                "11".repeat(32).as_str(),
                "22".repeat(32).as_str(),
            ),
            LogFilter::parse(Some("trace")),
            vec!["SecretPlayer".to_string()],
        )
        .with_rotation_limit(240);
        for tick in 0..8 {
            logger
                .log(
                    0,
                    LogLevel::Info,
                    "pb_sim",
                    &[(LogField::Tick, tick.to_string())],
                    "SecretPlayer at /outside/private/file",
                )
                .expect("log write must succeed");
        }
        assert!(logger.path().exists());
        assert!(logger.path().with_extension("log.1").exists());
        let current = fs::read_to_string(logger.path()).expect("log must read");
        assert!(!current.contains("SecretPlayer"));
        assert!(!current.contains("/outside"));
    }

    #[test]
    fn crash_bundle_has_four_files_and_only_fixed_report_sections() {
        let root = test_root("crash");
        let metadata = BuildMetadata::new(
            "abcdef012345",
            "11".repeat(32).as_str(),
            "22".repeat(32).as_str(),
        );
        let directory = write_crash_bundle(
            &root,
            Path::new("/game"),
            &["PlayerName".to_string()],
            &CrashBundle {
                timestamp: 0,
                panic_message: "forced panic for PlayerName",
                backtrace: "0: /private/source.rs",
                metadata: &metadata,
                events: &["event: safe".to_string()],
                seed: 42,
                scenario: "prov_full_battle",
                terminal_tick: 12,
                journal: "# deterministic\n",
                state_hash: "aa",
            },
        )
        .expect("bundle must write");
        for file in ["report.txt", "journal.jrnl", "meta.toml", "state.hash"] {
            assert!(directory.join(file).is_file(), "missing {file}");
        }
        let report = fs::read_to_string(directory.join("report.txt")).expect("report must read");
        let headings: Vec<_> = report
            .lines()
            .filter_map(|line| line.split_once(':').map(|(head, _)| head))
            .filter(|head| {
                [
                    "panic",
                    "backtrace",
                    "build",
                    "ruleset",
                    "content",
                    "events",
                ]
                .contains(head)
            })
            .collect();
        assert_eq!(
            headings,
            [
                "panic",
                "backtrace",
                "build",
                "ruleset",
                "content",
                "events"
            ]
        );
        assert!(!report.contains("PlayerName"));
        assert!(!report.contains("/private"));
    }
}
