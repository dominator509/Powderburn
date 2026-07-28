//! Reproduce a crash bundle by replaying its exact journal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run_repro(directory: &Path) -> Result<(), String> {
    for required in ["report.txt", "journal.jrnl", "meta.toml", "state.hash"] {
        if !directory.join(required).is_file() {
            return Err(format!("E-REPRO-FORMAT: missing {required}"));
        }
    }
    let metadata = parse_metadata(&directory.join("meta.toml"))?;
    let scenario = required(&metadata, "scenario")?;
    let seed = required(&metadata, "seed")?;
    let terminal_tick = required(&metadata, "terminal_tick")?;
    let expected = std::fs::read_to_string(directory.join("state.hash"))
        .map_err(|error| format!("E-REPRO-READ: cannot read state hash: {error}"))?
        .trim()
        .to_string();

    let executable = sibling_pbcli()?;
    let output = Command::new(&executable)
        .args(["sim", "--scenario", scenario, "--seed", seed, "--journal"])
        .arg(directory.join("journal.jrnl"))
        .arg("--emit-hash")
        .output()
        .map_err(|error| format!("E-REPRO-RUN: cannot execute pbcli: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "E-REPRO-RUN: pbcli refused the bundle: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "E-REPRO-RUN: pbcli output was not UTF-8".to_string())?;
    let actual = stdout
        .lines()
        .find_map(|line| line.strip_prefix("state-hash: "))
        .ok_or_else(|| "E-REPRO-RUN: pbcli emitted no state hash".to_string())?;
    if actual == expected {
        println!("repro: match");
        Ok(())
    } else {
        println!("repro: differ at tick {terminal_tick}");
        Err(format!(
            "E-REPRO-DIFFER: expected {expected}, reproduced {actual}"
        ))
    }
}

fn parse_metadata(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("E-REPRO-READ: cannot read metadata: {error}"))?;
    let mut values = BTreeMap::new();
    for (line_number, line) in raw.lines().enumerate() {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "E-REPRO-FORMAT: malformed metadata line {}",
                line_number + 1
            ));
        };
        let key = key.trim().to_string();
        let value = value.trim().trim_matches('"').to_string();
        if key.is_empty() || value.is_empty() || values.insert(key, value).is_some() {
            return Err(format!(
                "E-REPRO-FORMAT: invalid metadata line {}",
                line_number + 1
            ));
        }
    }
    Ok(values)
}

fn required<'a>(metadata: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    metadata
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("E-REPRO-FORMAT: missing metadata key {key}"))
}

fn sibling_pbcli() -> Result<PathBuf, String> {
    let current = std::env::current_exe()
        .map_err(|error| format!("E-REPRO-RUN: cannot locate pbtool: {error}"))?;
    let parent = current
        .parent()
        .ok_or_else(|| "E-REPRO-RUN: pbtool has no parent directory".to_string())?;
    let candidate = parent.join(if cfg!(windows) { "pbcli.exe" } else { "pbcli" });
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err("E-REPRO-RUN: pbcli is not installed beside pbtool".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_parser_rejects_duplicate_keys() {
        let path =
            std::env::temp_dir().join(format!("powderburn-repro-meta-{}.toml", std::process::id()));
        assert!(std::fs::write(&path, "seed = 1\nseed = 2\n").is_ok());
        assert!(parse_metadata(&path).is_err());
        let _ = std::fs::remove_file(path);
    }
}
