//! Adversarial tests for save loading security trust boundaries (EP-006 M2).
//!
//! Tests that malformed save files produce the correct named errors rather
//! than panics:
//!
//! - `huge_declared_len.pbsave` — a valid PBSV file containing 5000 ledger
//!   entries, exceeding [`MAX_LEDGER_ENTRIES`] (4096).  Must produce
//!   `E-SAVE-OVERSIZE`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use pb_save::error::SaveError;
use pb_save::load;

/// Locate the test fixtures directory.
fn fixtures_dir() -> std::path::PathBuf {
    let candidates = [
        Path::new("crates/pb-save/tests/fixtures").to_path_buf(),
        Path::new("../pb-save/tests/fixtures").to_path_buf(),
        Path::new("tests/fixtures").to_path_buf(),
        Path::new("../tests/fixtures").to_path_buf(),
    ];
    for c in &candidates {
        if c.join("huge_declared_len.pbsave").exists() {
            return c.clone();
        }
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = Path::new(&manifest).join("tests").join("fixtures");
        if p.join("huge_declared_len.pbsave").exists() {
            return p;
        }
    }
    panic!(
        "Cannot find fixtures directory. Tried: {:?}. CWD: {:?}",
        candidates,
        std::env::current_dir().unwrap()
    );
}

/// Convert a 64-char hex string to [u8; 32].
fn hex_to_bytes(s: &str) -> [u8; 32] {
    assert_eq!(s.len(), 64, "hex string must be 64 chars, got {}", s.len());
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("valid hex");
    }
    out
}

// ---------------------------------------------------------------------------
// huge_declared_len.pbsave — more ledger entries than MAX_LEDGER_ENTRIES
// ---------------------------------------------------------------------------
#[test]
fn huge_declared_len_returns_oversize_error() {
    let fixtures = fixtures_dir();
    let path = fixtures.join("huge_declared_len.pbsave");

    assert!(path.exists(), "fixture not found at {:?}", path);

    // These must match the hashes embedded in the fixture
    let ruleset_hash =
        hex_to_bytes("6d20a7bfd78db7eacd043356ccff7fb0b789653547a6468b3ac9ec7f38a25986");
    let content_hash =
        hex_to_bytes("1794ba116b894de6d4ddc436e5bc932ba76a456d6e5c1fe2dbd2e2b8231e7e38");

    let result = load::read(&path, &ruleset_hash, &content_hash);

    assert!(
        result.is_err(),
        "expected error for oversized ledger entries, got Ok"
    );

    match result {
        Err(SaveError::Oversize) => {} // expected
        Err(other) => {
            panic!(
                "expected E-SAVE-OVERSIZE for huge declared length, got: {:?}",
                other
            );
        }
        Ok(_) => unreachable!(),
    }
}
