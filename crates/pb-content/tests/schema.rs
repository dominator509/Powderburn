//! SPEC-002 section 3: persisted entity schemas round-trip without data loss.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use pb_content::{
    load::load_all,
    schema::{CompanionData, SaveFileData, ScenarioData, SimSnapshotData, WeaponData},
};

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

fn assert_ron_round_trip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let encoded = ron::ser::to_string(value).expect("schema value must serialize");
    let decoded: T = ron::from_str(&encoded).expect("schema value must deserialize");
    let reencoded = ron::ser::to_string(&decoded).expect("decoded value must serialize");
    assert_eq!(reencoded, encoded, "RON round-trip changed the entity");
}

#[test]
fn shipped_entity_records_round_trip() {
    let content = load_all(&content_root()).expect("shipped content must parse");

    let scenario: &ScenarioData = content
        .scenarios
        .values()
        .next()
        .expect("at least one scenario is required");
    let weapon: &WeaponData = content
        .weapons
        .values()
        .next()
        .expect("at least one weapon is required");
    let companion: &CompanionData = content
        .companions
        .values()
        .next()
        .expect("at least one companion is required");

    assert_ron_round_trip(scenario);
    assert_ron_round_trip(weapon);
    assert_ron_round_trip(companion);
}

#[test]
fn save_container_schema_round_trips_optional_snapshot() {
    let save = SaveFileData {
        format_version: 1,
        ruleset_hash: "11".repeat(32),
        content_hash: "22".repeat(32),
        campaign_seed: 1867,
        ledger_head_hash: "00".repeat(32),
        ledger_weight: 0,
        ledger_entries: Vec::new(),
        campaign_flags: vec!["campaign_started".to_string()],
        completed_nodes: vec!["m01_elk_creek".to_string()],
        company: Vec::new(),
        sim_snapshot: Some(SimSnapshotData {
            state_ron: "(tick:7)".to_string(),
            state_hash: "33".repeat(32),
        }),
        written_at_tick: 7,
    };
    assert_ron_round_trip(&save);
}
