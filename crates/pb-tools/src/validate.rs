//! Validate subcommand for pbtool.
//! Checks content validity, representation law, and provenance.

use std::fs;
use std::path::Path;

use pb_content::load::load_all;

// Output constants are defined inline in the `output` submodule below.

/// Run `pbtool validate content`: load and validate all content.
pub fn validate_content(content_root: &Path) -> Result<(), String> {
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let diags = pb_content::validate::validate(&content);

    if diags.is_empty() {
        println!("{}", output::PBM_VALIDATE_OK);
    } else {
        for d in &diags {
            eprintln!("{}: {} (file: {:?}, line: {:?})", d.code, d.message, d.file, d.line);
        }
        return Err(format!("{} diagnostics found", diags.len()));
    }

    Ok(())
}

/// Run `pbtool validate representation`: check nation/community fields.
pub fn validate_representation(content_root: &Path) -> Result<(), String> {
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let mut issues: Vec<String> = Vec::new();

    for (id, companion) in &content.companions {
        // Check that nation or community is present
        if companion.nation.is_none() && companion.community.is_none() {
            issues.push(format!(
                "companion '{}' has neither nation nor community",
                id
            ));
        }
        // Check that nation has a human-readable name (non-empty if Some)
        if let Some(ref nation) = companion.nation {
            if nation.trim().is_empty() {
                issues.push(format!("companion '{}' has empty nation", id));
            }
        }
        // Check that community has a human-readable name (non-empty if Some)
        if let Some(ref community) = companion.community {
            if community.trim().is_empty() {
                issues.push(format!("companion '{}' has empty community", id));
            }
        }
        // Check sources are not empty
        if companion.sources.is_empty() {
            issues.push(format!("companion '{}' has no sources", id));
        }
    }

    if issues.is_empty() {
        println!("{}", output::REPRESENTATION_OK);
    } else {
        for issue in &issues {
            eprintln!("representation issue: {}", issue);
        }
        return Err(format!("{} representation issues", issues.len()));
    }

    Ok(())
}

/// Run `pbtool validate provenance`: check PROVENANCE.toml asset root.
pub fn validate_provenance(asset_root: &Path) -> Result<(), String> {
    let provenance_path = asset_root.join("PROVENANCE.toml");

    if !provenance_path.exists() {
        return Err(format!(
            "PROVENANCE.toml not found at {}",
            provenance_path.display()
        ));
    }

    let _content = fs::read_to_string(&provenance_path)
        .map_err(|e| format!("cannot read PROVENANCE.toml: {}", e))?;

    println!("{}", output::PROVENANCE_OK);
    Ok(())
}

/// Sentinel output constants.
pub(crate) mod output {
    pub const PBM_VALIDATE_OK: &str = "pbtool validate: ok";
    pub const REPRESENTATION_OK: &str = "representation: ok";
    pub const PROVENANCE_OK: &str = "provenance: ok";
}
