//! SPEC-000 section 7: specific identity, sources, and forbidden language.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use pb_content::{
    load::load_all,
    representation::{forbidden_tokens, validate_tree},
};

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

#[test]
fn shipped_content_satisfies_the_full_representation_law() {
    let issues = validate_tree(&content_root()).expect("representation scan must execute");
    assert!(issues.is_empty(), "representation violations: {issues:#?}");
}

#[test]
fn native_companion_names_a_specific_nation_and_sources() {
    let content = load_all(&content_root()).expect("shipped content must parse");
    let whitehorse = content
        .companions
        .get("c_whitehorse")
        .expect("White Horse must be in the roster");
    assert_eq!(whitehorse.nation.as_deref(), Some("Kiowa"));
    assert!(!whitehorse.sources.is_empty());
}

#[test]
fn forbidden_token_match_is_case_insensitive() {
    let tokens = ["savage", "red_man"];
    assert_eq!(
        forbidden_tokens("A SAVAGE stereotype", &tokens),
        vec!["savage"]
    );
    assert_eq!(
        forbidden_tokens("the red_man token is prohibited", &tokens),
        vec!["red_man"]
    );
    assert!(forbidden_tokens("specific nation named", &tokens).is_empty());
}
