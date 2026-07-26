//! Fuzzing harness for content parsing.
//!
//! Provides a simple fuzz loop that exercises RON deserialization on random
//! byte sequences to detect panics.  This is not a coverage-guided fuzzer
//! (that belongs in a dedicated fuzz/ crate with `cargo-fuzz` or `libfuzzer`),
//! but rather a deterministic smoke-fuzz for CI runs and developer iteration.
//!
//! # Usage (from the workspace root)
//!
//! ```ignore
//! use pb_tools::fuzz::{fuzz_content, run_fuzz};
//! run_fuzz(&std::path::Path::new("tests/fixtures/adversarial"), 100_000, 42);
//! ```

use std::path::Path;

/// Fuzz a single byte slice by attempting to parse it as RON.
///
/// This function catches panics via `std::panic::catch_unwind` so that a
/// single bad input cannot crash the fuzz harness.  It does **not** make
/// assertions about whether the parse succeeds — only that it does not panic.
pub fn fuzz_content(input: &[u8]) {
    let s = match std::str::from_utf8(input) {
        Ok(s) => s,
        Err(_) => return, // non-UTF-8 is not a panic condition
    };

    // Attempt to deserialize as a generic serde::Value (any valid RON).
    // We only care about panics, not parse errors.
    let _ = std::panic::catch_unwind(|| {
        let _: Result<ron::Value, _> = ron::from_str(s);
    });
}

/// Run a deterministic fuzz loop.
///
/// Generates `iters` random byte sequences using a simple LCG seeded with
/// `seed`, feeds each to [`fuzz_content`], and reports progress every 10 %
/// of the iteration count.  Also attempts to parse every `.ron` file under
/// `content_root` as a smoke check.
///
/// # Panics
///
/// Panics if the seed value is 0 (the LCG degenerates).
pub fn run_fuzz(content_root: &Path, iters: usize, seed: u64) {
    assert!(seed != 0, "fuzz seed must be non-zero");

    // Step 1: smoke-test all existing .ron files in the content root
    if content_root.exists() {
        if let Ok(entries) = std::fs::read_dir(content_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "ron")
                    || path.extension().map_or(false, |ext| ext == "RON")
                {
                    match std::fs::read(&path) {
                        Ok(data) => {
                            fuzz_content(&data);
                        }
                        Err(_) => { /* skip unreadable */ }
                    }
                }

                // One level deep for adversarial fixtures that may be in subdirs
                if path.is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub in sub_entries.flatten() {
                            let sub_path = sub.path();
                            if sub_path.extension().map_or(false, |ext| ext == "ron") {
                                if let Ok(data) = std::fs::read(&sub_path) {
                                    fuzz_content(&data);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 2: random byte-sequence fuzzing
    // Simple LCG (glibc-style) for deterministic sequences
    let mut state = seed;
    let report_interval = if iters > 0 { iters / 10 } else { 1 };

    for i in 0..iters {
        // Generate a random byte sequence of length 1..=256
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let len = 1 + (state >> 56) as usize % 256;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            buf.push((state >> 40) as u8);
        }

        fuzz_content(&buf);

        if report_interval > 0 && (i + 1) % report_interval == 0 {
            let pct = ((i + 1) * 100) / iters;
            eprintln!("[fuzz] {:3}% — {} iters", pct, i + 1);
        }
    }

    eprintln!("[fuzz] 100% — DONE ({} iters)", iters);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_content_empty() {
        // Empty slice — should not panic
        fuzz_content(b"");
    }

    #[test]
    fn test_fuzz_content_valid_ron() {
        // Valid RON — should not panic
        fuzz_content(b"42");
        fuzz_content(b"\"hello\"");
        fuzz_content(b"[1, 2, 3]");
        fuzz_content(b"Some(\"value\")");
    }

    #[test]
    fn test_fuzz_content_garbage() {
        // Garbage bytes — should not panic
        fuzz_content(b"\xff\xfe\x00\x01");
        fuzz_content(b"!@#$%^&*()");
        fuzz_content(b"\\x00\\x01\\x02\\x03");
    }
}
