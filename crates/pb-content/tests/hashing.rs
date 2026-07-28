//! SPEC-002 section 8: ruleset and content hashes are stable and scoped.

#![allow(clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use pb_content::hash::{content_hash, hex, ruleset_hash};

fn shipped_content() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

fn test_root(name: &str) -> PathBuf {
    let cache = std::env::var_os("PB_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(".pbcache")
        });
    cache
        .join("test")
        .join(format!("{name}-{}", std::process::id()))
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn shipped_hashes_are_stable_and_well_formed() {
    let root = shipped_content();
    let rules_a = ruleset_hash(&root).expect("ruleset must hash");
    let rules_b = ruleset_hash(&root).expect("ruleset must hash repeatedly");
    let content_a = content_hash(&root).expect("content must hash");
    let content_b = content_hash(&root).expect("content must hash repeatedly");

    assert_eq!(rules_a, rules_b);
    assert_eq!(content_a, content_b);
    assert_eq!(hex(&rules_a).len(), 64);
    assert_eq!(hex(&content_a).len(), 64);
    assert_ne!(rules_a, [0; 32], "ruleset hash must not be a sentinel");
    assert_ne!(content_a, [0; 32], "content hash must not be a sentinel");
}

#[test]
fn bibliography_is_excluded_but_executable_content_is_not() {
    let root = test_root("hash-scope");
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join("rules")).expect("test tree must be created");
    fs::write(root.join("rules/weapons.ron"), b"weapons-v1").expect("rule fixture must be written");
    fs::write(root.join("scenario.ron"), b"scenario-v1").expect("content fixture must be written");
    fs::write(root.join("BIBLIOGRAPHY.md"), b"citation-v1")
        .expect("bibliography fixture must be written");

    let initial_rules = ruleset_hash(&root).expect("ruleset must hash");
    let initial_content = content_hash(&root).expect("content must hash");

    fs::write(root.join("BIBLIOGRAPHY.md"), b"citation-v2")
        .expect("bibliography fixture must change");
    assert_eq!(
        content_hash(&root).expect("content must rehash"),
        initial_content
    );

    fs::write(root.join("rules/weapons.ron"), b"weapons-v2").expect("rule fixture must change");
    assert_ne!(
        ruleset_hash(&root).expect("ruleset must rehash"),
        initial_rules
    );
    assert_ne!(
        content_hash(&root).expect("content must rehash"),
        initial_content
    );
}
