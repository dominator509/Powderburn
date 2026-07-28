//! SPEC-000 section 5.5: `HISTORICAL_FIXED` scenario records are immutable.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use pb_content::{history::actor_override_violations, load::load_all, schema::ActorData};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

#[test]
fn official_history_violation_fixture_is_rejected_precisely() {
    let root = workspace_root();
    let content = load_all(&root.join("content")).expect("shipped content must parse");
    let fixture_text =
        std::fs::read_to_string(root.join("tests/fixtures/violation_alters_history.ron"))
            .expect("official history fixture must be readable");
    let overrides: Vec<ActorData> =
        ron::from_str(&fixture_text).expect("official history fixture must parse");

    let violations = actor_override_violations(&content, &overrides);
    assert!(
        violations.iter().any(|(scenario, actor)| {
            scenario == "scn_m02_promontory" && actor == "m02_promontory_enemy_01"
        }),
        "fixed actor override was not detected: {violations:#?}"
    );
}

#[test]
fn unrelated_actor_record_is_not_falsely_rejected() {
    let root = workspace_root();
    let content = load_all(&root.join("content")).expect("shipped content must parse");
    let mut unrelated = content
        .scenarios
        .values()
        .next()
        .and_then(|scenario| scenario.actors.first())
        .expect("proof actor must exist")
        .clone();
    unrelated.id = "e_unrelated_mod_actor".to_string();

    assert!(actor_override_violations(&content, &[unrelated]).is_empty());
}
