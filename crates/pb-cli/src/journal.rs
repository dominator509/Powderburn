//! Journal file parser.
//!
//! Parses deterministic journal files that record commands applied during
//! a simulation run. Format per line:
//!   <tick> <actor_id> <command_name> [<arg>=<value>...]
//!
//! Returns E-JOURNAL-PARSE on bad line syntax or E-JOURNAL-ILLEGAL on
//! illegal commands at a tick.

use std::fmt;
use std::fs;
use std::path::Path;

/// Maximum number of lines allowed in a journal file.
pub const MAX_JOURNAL_LINES: usize = 262144;

use pb_core::event::HitLocationType;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_sim::action::{Action, Command};

/// Errors that can occur during journal parsing.
#[derive(Debug, Clone)]
pub enum JournalError {
    /// E-JOURNAL-PARSE: malformed line.
    Parse(String),
    /// E-JOURNAL-ILLEGAL: unrecognized command or invalid arguments.
    Illegal(String),
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::Parse(msg) => write!(f, "E-JOURNAL-PARSE: {}", msg),
            JournalError::Illegal(msg) => write!(f, "E-JOURNAL-ILLEGAL: {}", msg),
        }
    }
}

impl std::error::Error for JournalError {}

/// Parse a journal file, returning ordered (tick, actor_id, Command) tuples.
pub fn parse_journal(path: &Path) -> Result<Vec<(u64, u32, Command)>, JournalError> {
    let content = fs::read_to_string(path)
        .map_err(|e| JournalError::Parse(format!("cannot read journal file: {}", e)))?;

    let mut entries: Vec<(u64, u32, Command)> = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(JournalError::Parse(format!(
                "line {}: expected at least 3 fields, got {}",
                line_no + 1,
                parts.len()
            )));
        }

        let tick: u64 = parts[0].parse().map_err(|e| {
            JournalError::Parse(format!("line {}: invalid tick: {}", line_no + 1, e))
        })?;
        let actor_id: u32 = parts[1].parse().map_err(|e| {
            JournalError::Parse(format!("line {}: invalid actor_id: {}", line_no + 1, e))
        })?;
        let cmd_name = parts[2];

        let action = parse_action(cmd_name, &parts[3..], line_no)?;

        entries.push((
            tick,
            actor_id,
            Command {
                actor_id: ActorId(actor_id),
                action,
            },
        ));
    }

    Ok(entries)
}

/// Parse an Action from a command name and arguments.
fn parse_action(name: &str, args: &[&str], line_no: usize) -> Result<Action, JournalError> {
    match name {
        "hold" | "Hold" => Ok(Action::Hold),
        "reload" | "Reload" => Ok(Action::Reload),
        "useitem" | "UseItem" => Ok(Action::UseItem),
        "move" | "Move" => {
            let (x, y) = parse_xy(args, "Move", line_no)?;
            Ok(Action::Move(TileXY::new(x, y)))
        }
        "snapshot" | "SnapShot" => {
            let target = parse_target(args, "SnapShot", line_no)?;
            Ok(Action::SnapShot(target))
        }
        "aimedshot" | "AimedShot" => {
            let target = parse_target(args, "AimedShot", line_no)?;
            Ok(Action::AimedShot(target))
        }
        "calledshot" | "CalledShot" => {
            let target = parse_target(args, "CalledShot", line_no)?;
            let loc = parse_location(args, line_no)?;
            Ok(Action::CalledShot(target, loc))
        }
        "bandage" | "Bandage" => {
            let target = parse_target(args, "Bandage", line_no)?;
            Ok(Action::Bandage(target))
        }
        "melee" | "Melee" => {
            let target = parse_target(args, "Melee", line_no)?;
            Ok(Action::Melee(target))
        }
        "throwdynamite" | "ThrowDynamite" => {
            let (x, y) = parse_xy(args, "ThrowDynamite", line_no)?;
            Ok(Action::ThrowDynamite(TileXY::new(x, y)))
        }
        _ => Err(JournalError::Illegal(format!(
            "line {}: unknown command '{}'",
            line_no + 1,
            name
        ))),
    }
}

/// Parse x,y from args (format: x=<i16> y=<i16> or just two integers).
fn parse_xy(args: &[&str], cmd: &str, line_no: usize) -> Result<(i16, i16), JournalError> {
    if args.len() >= 2 {
        // Try name=value format
        let mut x: Option<i16> = None;
        let mut y: Option<i16> = None;
        for a in args {
            if let Some(val) = a.strip_prefix("x=") {
                x = Some(val.parse().map_err(|e| {
                    JournalError::Parse(format!("line {}: {} invalid x: {}", line_no + 1, cmd, e))
                })?);
            } else if let Some(val) = a.strip_prefix("y=") {
                y = Some(val.parse().map_err(|e| {
                    JournalError::Parse(format!("line {}: {} invalid y: {}", line_no + 1, cmd, e))
                })?);
            } else if let Some(_val) = a.strip_prefix("target=") {
                // skip target field
            } else if let Some(_val) = a.strip_prefix("loc=") {
                // skip loc field
            }
        }
        if let (Some(xv), Some(yv)) = (x, y) {
            return Ok((xv, yv));
        }
        // Fall back to positional: first two args as x,y
        let xv: i16 = args[0].parse().map_err(|e| {
            JournalError::Parse(format!(
                "line {}: {} invalid position arg: {}",
                line_no + 1,
                cmd,
                e
            ))
        })?;
        let yv: i16 = args[1].parse().map_err(|e| {
            JournalError::Parse(format!(
                "line {}: {} invalid position arg: {}",
                line_no + 1,
                cmd,
                e
            ))
        })?;
        Ok((xv, yv))
    } else {
        Err(JournalError::Parse(format!(
            "line {}: {} requires x, y target",
            line_no + 1,
            cmd
        )))
    }
}

/// Parse a target ActorId from args.
fn parse_target(args: &[&str], cmd: &str, line_no: usize) -> Result<ActorId, JournalError> {
    for a in args {
        if let Some(val) = a.strip_prefix("target=") {
            let id: u32 = val.parse().map_err(|e| {
                JournalError::Parse(format!(
                    "line {}: {} invalid target: {}",
                    line_no + 1,
                    cmd,
                    e
                ))
            })?;
            return Ok(ActorId(id));
        }
    }
    // Fall back: first arg as raw number
    if let Ok(id) = args.first().copied().unwrap_or("0").parse::<u32>() {
        return Ok(ActorId(id));
    }
    Err(JournalError::Parse(format!(
        "line {}: {} requires target=<actor_id>",
        line_no + 1,
        cmd
    )))
}

/// Parse a hit location from args.
fn parse_location(args: &[&str], line_no: usize) -> Result<HitLocationType, JournalError> {
    for a in args {
        if let Some(val) = a.strip_prefix("loc=") {
            return match val.to_lowercase().as_str() {
                "head" => Ok(HitLocationType::Head),
                "eyes" => Ok(HitLocationType::Eyes),
                "torso" => Ok(HitLocationType::Torso),
                "vitals" => Ok(HitLocationType::Vitals),
                "gunarm" => Ok(HitLocationType::GunArm),
                "offarm" => Ok(HitLocationType::OffArm),
                "legs" => Ok(HitLocationType::Legs),
                _ => Err(JournalError::Illegal(format!(
                    "line {}: unknown hit location '{}'",
                    line_no + 1,
                    val
                ))),
            };
        }
    }
    Ok(HitLocationType::Torso) // default
}

/// Extract a branch choice from a script/script file.
///
/// Looks for a line matching `# choice: <value>` (case-insensitive `choice:`)
/// and returns the value if found.  Returns `None` if no such line exists.
pub fn extract_choice_from_script(path: &Path) -> Result<Option<String>, JournalError> {
    let content = fs::read_to_string(path)
        .map_err(|e| JournalError::Parse(format!("cannot read choice file: {}", e)))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("# choice:") {
            let val = val.trim();
            if !val.is_empty() {
                return Ok(Some(val.to_string()));
            }
        } else if let Some(val) = trimmed.strip_prefix("#choice:") {
            let val = val.trim();
            if !val.is_empty() {
                return Ok(Some(val.to_string()));
            }
        }
    }
    Ok(None)
}
