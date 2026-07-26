//! Golden subcommand for pbtool.
//! Refreshes golden hashes for deterministic tests.

use std::path::Path;
use std::process::Command;

/// Run `pbtool golden refresh`.
pub fn golden_refresh(golden_path: &Path) -> Result<(), String> {
    // Check that the git tree is clean
    let status = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(golden_path)
        .output()
        .map_err(|e| format!("git status failed: {}", e))?;

    let output = String::from_utf8_lossy(&status.stdout);
    let has_changes = output.lines().any(|l| !l.is_empty());

    if has_changes {
        return Err(
            "git tree is not clean; commit or stash changes before refreshing golden hashes"
                .to_string(),
        );
    }

    // Look for golden hash files and refresh them
    let golden_dir = golden_path.join("golden");
    if golden_dir.exists() && golden_dir.is_dir() {
        for entry in
            fs::read_dir(&golden_dir).map_err(|e| format!("cannot read golden dir: {}", e))?
        {
            let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
            let path = entry.path();
            if path.is_file() {
                // Re-read the content and recompute hash
                let data = fs::read(&path)
                    .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
                let hash = pb_core::hash::hash_state(&data);
                let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
                fs::write(&path, hex.as_bytes())
                    .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
            }
        }
    }

    println!("{}", crate::validate::output::PBM_VALIDATE_OK);
    println!("pbtool golden: ok");
    Ok(())
}

use std::fs;
