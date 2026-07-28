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
    let graph = pb_content::campaign::build_graph(&content);
    let mut campaign_issues = pb_content::campaign::structural_issues(&graph, "m01_elk_creek");
    let mission_count = graph
        .nodes
        .values()
        .filter(|node| node.kind == "Mission")
        .count();
    if mission_count != 24 {
        campaign_issues.push(format!(
            "campaign must contain 24 missions, found {mission_count}"
        ));
    }
    let camp_count = graph
        .nodes
        .values()
        .filter(|node| node.kind == "Camp")
        .count();
    if camp_count != 12 {
        campaign_issues.push(format!(
            "campaign must contain 12 camp interludes, found {camp_count}"
        ));
    }

    if diags.is_empty() && campaign_issues.is_empty() {
        println!("{}", output::PBM_VALIDATE_OK);
    } else {
        for d in &diags {
            eprintln!(
                "{}: {} (file: {:?}, line: {:?})",
                d.code, d.message, d.file, d.line
            );
        }
        for issue in &campaign_issues {
            eprintln!("E-CAMPAIGN-GRAPH: {issue}");
        }
        return Err(format!(
            "{} diagnostics found",
            diags.len() + campaign_issues.len()
        ));
    }

    Ok(())
}

/// Run `pbtool validate representation`: check nation/community fields.
pub fn validate_representation(content_root: &Path) -> Result<(), String> {
    let issues = pb_content::representation::validate_tree(content_root)
        .map_err(|error| format!("content load error: {error}"))?;

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

/// Run `pbtool validate content --with-fixture <path>`: load content,
/// merge fixture, check for historical violations (E-HIST-001).
pub fn validate_content_with_fixture(
    content_root: &Path,
    fixture_path: &Path,
) -> Result<(), String> {
    let content = pb_content::load::load_all(content_root)
        .map_err(|e| format!("content load error: {}", e))?;

    // Load fixture file. The fixture is a list of actors that would alter a scenario.
    let fixture_data = std::fs::read_to_string(fixture_path)
        .map_err(|e| format!("cannot read fixture '{}': {}", fixture_path.display(), e))?;

    // Parse fixture as Vec<ActorData>
    let fixture_actors: Vec<pb_content::schema::ActorData> = ron::from_str(&fixture_data)
        .map_err(|e| format!("cannot parse fixture '{}': {}", fixture_path.display(), e))?;

    let violations = pb_content::history::actor_override_violations(&content, &fixture_actors);
    for (scenario_id, actor_id) in &violations {
        eprintln!(
            "E-HIST-001: fixture alters scenario '{scenario_id}' which is marked HISTORICAL_FIXED via campaign node; actor '{actor_id}' modified"
        );
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} historical violation(s) detected",
            violations.len()
        ))
    }
}

/// Sentinel output constants.
pub(crate) mod output {
    pub const PBM_VALIDATE_OK: &str = "pbtool validate: ok";
    pub const REPRESENTATION_OK: &str = "representation: ok";
    pub const PROVENANCE_OK: &str = "provenance: ok";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn real_content_representation_provenance_and_history_contracts_execute() {
        let root = repository_root();
        assert!(validate_content(&root.join("content")).is_ok());
        assert!(validate_representation(&root.join("content")).is_ok());
        assert!(validate_provenance(&root.join("assets")).is_ok());
        let historical = validate_content_with_fixture(
            &root.join("content"),
            &root.join("tests/fixtures/violation_alters_history.ron"),
        );
        assert!(historical.is_err());
        assert!(historical
            .err()
            .is_some_and(|error| error.contains("historical violation")));
    }
}
