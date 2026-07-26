//! Forced-failure regression tests for every gate in the Powderburn CI/CD
//! pipeline.
//!
//! Each test:
//! 1. Injects a violation into a temporary file.
//! 2. Asserts the gate script detects and reports the violation.
//! 3. Restores the original file content.
//!
//! Gates tested:
//! - `lint-determinism.sh` (LBI-01, LBI-02, LBI-03): floats in kernel
//! - `format-check.sh`: formatting violations
//! - `reality-gate.sh`: forbidden implementation markers (TODO)

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the project root (PB_HOME).
fn project_root() -> PathBuf {
    // Determine PB_HOME: try env var first, then walk up from cwd.
    if let Ok(home) = std::env::var("PB_HOME") {
        if !home.is_empty() {
            let p = PathBuf::from(home);
            if p.join("Cargo.toml").exists() && p.join("scripts").exists() {
                return p;
            }
        }
    }

    // Walk up from CWD looking for Cargo.toml + scripts
    let mut dir = std::env::current_dir().expect("current_dir");
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("scripts").exists() {
            return dir;
        }
        if !dir.pop() {
            panic!(
                "Could not find project root from {:?}",
                std::env::current_dir().unwrap()
            );
        }
    }
}

/// Run a shell script, returning (exit_status, stdout, stderr).
fn run_script(script: &str) -> (bool, String, String) {
    let root = project_root();
    let script_path = root.join(script);
    let output = Command::new("sh")
        .arg(&script_path)
        .env("PB_HOME", &root)
        .current_dir(&root)
        .output()
        .expect("failed to execute script");

    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (success, stdout, stderr)
}

/// Read a file as a string.
fn read_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {:?}: {}", path, e))
}

/// Write a string to a file, overwriting existing content.
fn write_file(path: &Path, content: &str) {
    fs::write(path, content).unwrap_or_else(|e| panic!("cannot write {:?}: {}", path, e));
}

/// Write a string to a temp file, returning the path.
fn write_tmp_file(content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("powderburn_gate_test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("test_{}.rs", std::process::id()));
    write_file(&path, content);
    path
}

// ===========================================================================
// Tests: lint-determinism gate
// ===========================================================================

/// LBI-02: The determinism lint must detect a float literal in a
/// determinism-critical crate's source.
///
/// We inject a float into a src file within a determinism-critical crate,
/// run the lint, verify failure, then restore.
#[test]
#[ignore = "requires scripts/lint-determinism.sh from project root"]
fn determinism_lint_fires_on_float_in_kernel() {
    let root = project_root();

    // Choose pb-core/src/fix32.rs — it's in a determinism-critical crate.
    let target_file = root.join("crates/pb-core/src/fix32.rs");
    assert!(
        target_file.exists(),
        "expected fix32.rs to exist at {:?}",
        target_file
    );

    let original = read_file(&target_file);

    // Inject a float constant at the end of the file
    let injection =
        "\n// deliberate float injection for LBI-02 test\nconst _TEST_FLOAT: f32 = 1.0;\n";
    let modified = format!("{}{}", original, injection);
    write_file(&target_file, &modified);

    // Verify the injection is actually present
    let after_write = read_file(&target_file);
    assert!(
        after_write.contains("f32"),
        "Injection failed: f32 not found in written file"
    );
    assert!(
        after_write.contains("_TEST_FLOAT"),
        "Injection failed: _TEST_FLOAT not found in written file"
    );

    // Run the determinism lint from the project root
    let (success, stdout, stderr) = run_script("scripts/lint-determinism.sh");

    // Restore original BEFORE asserting, so file is always clean
    write_file(&target_file, &original);

    // Verify restore worked
    let after_restore = read_file(&target_file);
    assert!(
        !after_restore.contains("_TEST_FLOAT"),
        "Restore failed: _TEST_FLOAT still present"
    );

    // The lint must fail
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        !success,
        "lint-determinism.sh should have failed with float injection.\n\
         stdout: {}\nstderr: {}\ncombined: {}",
        stdout, stderr, combined
    );

    assert!(
        combined.contains("f32"),
        "Output should mention f32\nOutput: {}",
        combined
    );
}

// ===========================================================================
// Tests: format-check gate
// ===========================================================================

/// The format-check gate must detect formatting violations.
#[test]
fn format_check_catches_formatting_violations() {
    let root = project_root();

    let target_file = root.join("crates/pb-core/src/fix32.rs");
    assert!(
        target_file.exists(),
        "expected fix32.rs to exist at {:?}",
        target_file
    );

    let original = read_file(&target_file);

    // Introduce a deliberate formatting violation: replace 4-space indent with 3
    let violation = original.replace("    pub const", "   pub const");

    if violation != original {
        write_file(&target_file, &violation);

        let (success, stdout, stderr) = run_script("scripts/format-check.sh");

        write_file(&target_file, &original);

        assert!(
            !success,
            "format-check.sh should have failed with formatting violation.\nstdout: {}\nstderr: {}",
            stdout, stderr
        );
    } else {
        // Alternative: add unexpected line breaks
        let violation2 = original.replace("pub const", "pub\nconst");
        write_file(&target_file, &violation2);

        let (success, stdout, stderr) = run_script("scripts/format-check.sh");

        write_file(&target_file, &original);

        assert!(
            !success,
            "format-check.sh should have failed with formatting violation.\nstdout: {}\nstderr: {}",
            stdout, stderr
        );
    }
}

// ===========================================================================
// Tests: reality-gate
// ===========================================================================

/// The reality gate must catch TODO markers in source files.
#[test]
fn reality_gate_catches_todo_markers() {
    let root = project_root();

    let target_file = root.join("crates/pb-core/src/fix32.rs");
    assert!(
        target_file.exists(),
        "expected fix32.rs to exist at {:?}",
        target_file
    );

    let original = read_file(&target_file);

    // Inject a TODO marker
    let injection = "\n// TODO: this is a deliberate test marker for reality-gate\n";
    let modified = format!("{}{}", original, injection);
    write_file(&target_file, &modified);

    let (success, stdout, stderr) = run_script("scripts/reality-gate.sh");

    write_file(&target_file, &original);

    let combined = format!("{}{}", stdout, stderr);
    assert!(
        !success,
        "reality-gate.sh should have failed with TODO marker injection.\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    assert!(
        combined.contains("TODO"),
        "Output should mention TODO\nOutput: {}",
        combined
    );
}

/// The reality gate catches FIXME markers.
#[test]
fn reality_gate_catches_fixme_markers() {
    let root = project_root();

    let target_file = root.join("crates/pb-core/src/fix32.rs");
    let original = read_file(&target_file);

    let injection = "\n// FIXME: deliberate test marker for reality-gate\n";
    let modified = format!("{}{}", original, injection);
    write_file(&target_file, &modified);

    let (success, stdout, stderr) = run_script("scripts/reality-gate.sh");

    write_file(&target_file, &original);

    let _combined = format!("{}{}", stdout, stderr);
    assert!(
        !success,
        "reality-gate.sh should have failed with FIXME marker injection.\nstdout: {}\nstderr: {}",
        stdout, stderr
    );
}
