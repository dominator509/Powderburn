//! Representation-law validation shared by tooling and integration tests.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::ContentError;

/// Validate identity metadata, sources, and the forbidden-token corpus.
pub fn validate_tree(content_root: &Path) -> Result<Vec<String>, ContentError> {
    let content = crate::load::load_all(content_root)?;
    let mut issues = Vec::new();

    for (id, companion) in &content.companions {
        if companion.nation.is_none() && companion.community.is_none() {
            issues.push(format!("companion '{id}' has neither nation nor community"));
        }
        if companion
            .nation
            .as_ref()
            .is_some_and(|nation| nation.trim().is_empty())
        {
            issues.push(format!("companion '{id}' has empty nation"));
        }
        if companion
            .community
            .as_ref()
            .is_some_and(|community| community.trim().is_empty())
        {
            issues.push(format!("companion '{id}' has empty community"));
        }
        if companion.sources.is_empty() {
            issues.push(format!("companion '{id}' has no sources"));
        }
        if companion.nation.as_ref().is_some_and(|nation| {
            matches!(
                nation.trim().to_ascii_lowercase().as_str(),
                "native" | "native american" | "indian" | "indigenous"
            )
        }) {
            issues.push(format!(
                "companion '{id}' uses a generic nation instead of a specific nation"
            ));
        }
    }

    let token_path = content_root.join("lint/forbidden_tokens.txt");
    let token_text = fs::read_to_string(&token_path).map_err(|error| {
        ContentError::new(
            "E-CONTENT-001",
            format!("cannot read forbidden token list: {error}"),
        )
    })?;
    let tokens: Vec<_> = token_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    let mut files = Vec::new();
    collect_files(content_root, &token_path, &mut files)?;
    files.sort();
    for path in files {
        let text = fs::read_to_string(&path).map_err(|error| {
            ContentError::new(
                "E-CONTENT-001",
                format!("cannot read representation input: {error}"),
            )
        })?;
        for (line_index, line) in text.lines().enumerate() {
            for token in forbidden_tokens(line, &tokens) {
                let relative = path.strip_prefix(content_root).unwrap_or(&path);
                issues.push(format!(
                    "{}:{} contains forbidden token '{}'",
                    relative.display(),
                    line_index + 1,
                    token
                ));
            }
        }
    }

    Ok(issues)
}

/// Return forbidden tokens present in one line, using case-insensitive matching.
pub fn forbidden_tokens<'a>(line: &str, tokens: &[&'a str]) -> Vec<&'a str> {
    let lower = line.to_ascii_lowercase();
    tokens
        .iter()
        .copied()
        .filter(|token| lower.contains(&token.to_ascii_lowercase()))
        .collect()
}

fn collect_files(
    directory: &Path,
    excluded: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), ContentError> {
    for entry in fs::read_dir(directory).map_err(|error| {
        ContentError::new("E-CONTENT-001", format!("cannot scan content: {error}"))
    })? {
        let path = entry
            .map_err(|error| ContentError::new("E-CONTENT-001", error.to_string()))?
            .path();
        if path == excluded {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, excluded, output)?;
        } else if path.is_file() {
            output.push(path);
        }
    }
    Ok(())
}
