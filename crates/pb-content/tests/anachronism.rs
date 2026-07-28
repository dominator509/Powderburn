//! SPEC-001 section 11: weapon availability is enforced against scenario date.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use pb_content::{load::load_all, validate::validate};

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

#[test]
fn shipped_weapon_loadouts_are_period_legal() {
    let content = load_all(&content_root()).expect("shipped content must parse");
    let diagnostics = validate(&content);

    let violations: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "E-ANACHRONISM-001")
        .collect();
    assert!(
        violations.is_empty(),
        "shipped scenarios contain anachronistic weapons: {violations:#?}"
    );
}

#[test]
fn weapon_introduced_after_scenario_date_is_rejected() {
    let mut content = load_all(&content_root()).expect("shipped content must parse");
    let scenario = content
        .scenarios
        .get("prov_full_battle")
        .expect("proof scenario must exist");
    let scenario_year: u16 = scenario.date[..4]
        .parse()
        .expect("proof scenario date must begin with a year");
    let weapon_id = scenario.actors[0]
        .equipped_primary
        .clone()
        .expect("proof actor must have a primary weapon");

    content
        .weapons
        .get_mut(&weapon_id)
        .expect("equipped weapon must resolve")
        .first_year_available = scenario_year + 1;

    let diagnostics = validate(&content);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E-ANACHRONISM-001"
                && diagnostic.message.contains(&weapon_id)
                && diagnostic.message.contains("prov_full_battle")
        }),
        "validator did not reject a post-date weapon: {diagnostics:#?}"
    );
}
