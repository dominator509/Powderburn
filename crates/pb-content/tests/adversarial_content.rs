//! Adversarial tests for content loading security trust boundaries (EP-006 M2).
//!
//! Tests that malformed content files produce the correct named errors rather
//! than panics:
//!
//! - `bomb/rules/weapons.ron` — a file with 8193 weapon records, exceeding
//!   [`MAX_RECORDS_PER_FILE`] (8192).  Must produce `E-CONTENT-001`.
//! - `deep_nest/scenarios/deep_nest.ron` — a RON file with extreme nesting
//!   depth that exceeds RON's recursion limit.  Must produce `E-CONTENT-001`.

use std::path::Path;

use pb_content::error::ContentError;
use pb_content::load::load_all;

/// Path to the test fixtures directory relative to the test binary.
/// `cargo test` runs from the workspace root, but integration tests in a crate
/// run relative to the crate directory.  We try several strategies.
fn fixtures_dir() -> std::path::PathBuf {
    let candidates = [
        Path::new("crates/pb-content/tests/fixtures").to_path_buf(),
        Path::new("../pb-content/tests/fixtures").to_path_buf(),
        Path::new("tests/fixtures").to_path_buf(),
        Path::new("../tests/fixtures").to_path_buf(),
    ];
    for c in &candidates {
        if c.join("bomb").exists() {
            return c.clone();
        }
    }
    // Fallback: construct from CARGO_MANIFEST_DIR
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = Path::new(&manifest).join("tests").join("fixtures");
        if p.exists() {
            return p;
        }
    }
    panic!(
        "Cannot find fixtures directory. Tried: {:?}. CWD: {:?}",
        candidates,
        std::env::current_dir().unwrap()
    );
}

// ---------------------------------------------------------------------------
// bomb.ron — too many records
// ---------------------------------------------------------------------------
#[test]
fn bomb_oversized_record_count_returns_content_error() {
    let fixtures = fixtures_dir();
    let bomb_dir = fixtures.join("bomb");

    assert!(
        bomb_dir.exists(),
        "bomb fixture directory not found at {:?}",
        bomb_dir
    );

    let result = load_all(&bomb_dir);

    assert!(
        result.is_err(),
        "expected error for oversized record count, got Ok"
    );

    match result {
        Err(e) => {
            // The error code should be E-CONTENT-001
            assert!(
                e.code == "E-CONTENT-001",
                "expected code E-CONTENT-001, got {}",
                e.code
            );
            // The message should mention the record limit
            assert!(
                e.message.contains("8192") || e.message.contains("records exceeds limit"),
                "expected message about record limit, got: {}",
                e.message
            );
        }
        Ok(_) => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// deep_nest.ron — RON recursion depth exceeded
// ---------------------------------------------------------------------------
#[test]
fn deep_nest_exceeds_ron_recursion_limit() {
    let fixtures = fixtures_dir();
    let deep_nest_dir = fixtures.join("deep_nest");

    assert!(
        deep_nest_dir.exists(),
        "deep_nest fixture directory not found at {:?}",
        deep_nest_dir
    );

    let result = load_all(&deep_nest_dir);

    assert!(
        result.is_err(),
        "expected error for deeply nested RON, got Ok"
    );

    match result {
        Err(e) => {
            // The error code should be E-CONTENT-001 (parse error)
            assert!(
                e.code == "E-CONTENT-001",
                "expected code E-CONTENT-001, got {}",
                e.code
            );
            // The message should indicate a parse/recursion error
            assert!(
                e.message.contains("recursion")
                    || e.message.contains("RecursionLimit")
                    || e.message.contains("depth")
                    || e.message.contains("parse error")
                    || e.message.contains("invalid"),
                "expected message about recursion/parse error, got: {}",
                e.message
            );
        }
        Ok(_) => unreachable!(),
    }
}
