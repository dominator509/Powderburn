//! LBI-07: dead companions have no ungated reachable dialogue references.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use pb_content::{
    campaign::{build_graph, check_permadeath_propagation},
    load::load_all,
};

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

fn proof_flags() -> Vec<String> {
    vec![
        "a1_treaty_witnessed".to_string(),
        "a2_promontory_present".to_string(),
        "a3_adobe_walls_survived".to_string(),
        "a4_salt_war_sided_with_ruelas".to_string(),
    ]
}

#[test]
fn whitehorse_death_has_zero_dangling_reachable_dialogue() {
    let content = load_all(&content_root()).expect("shipped content must parse");
    let graph = build_graph(&content);
    let violations = check_permadeath_propagation(
        &graph,
        &content,
        &["c_whitehorse".to_string()],
        &proof_flags(),
    );
    assert!(
        violations.is_empty(),
        "dead companion remains reachable: {violations:#?}"
    );
}

#[test]
fn missing_alive_gate_is_detected() {
    let mut content = load_all(&content_root()).expect("shipped content must parse");
    let scene = content
        .dialogue
        .get_mut("dlg_whitehorse_a1")
        .expect("White Horse act-one scene must exist");
    scene.requires_alive.clear();
    // Remove the enclosing campaign gate too, leaving a genuinely ungated
    // scene that the propagation check must reject.
    scene.node_id = None;
    let graph = build_graph(&content);

    let violations = check_permadeath_propagation(
        &graph,
        &content,
        &["c_whitehorse".to_string()],
        &proof_flags(),
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("dlg_whitehorse_a1")),
        "ungated dead speaker was not detected: {violations:#?}"
    );
}
