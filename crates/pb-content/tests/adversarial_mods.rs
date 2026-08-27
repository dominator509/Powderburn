//! Adversarial tests for mod loading security trust boundaries (EP-006 M2).
//!
//! Tests that malicious mod structures produce the correct named errors rather
//! than panics:
//!
//! - `zip_slip.mod` — a mod directory containing a symlink that resolves
//!   outside the mod root.  Must produce `E-MOD-PATH`.
//! - `symlink_escape.mod` — a mod directory containing a symlink that points
//!   to an absolute path outside the mod root.  Must produce `E-MOD-PATH`.
//!
//! Symlinks are created in test setup functions because git cannot track them
//! portably across platforms.

#![allow(unused_imports, clippy::expect_used)]

#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::Path;

use pb_content::error::ModError;
use pb_content::mods::load_mod;

/// Helper: create a minimal mod directory structure with a rules/ directory
/// and a single weapons.ron file so that `load_mod` won't fail on missing
/// content before it gets to the path-confinement check.
fn create_minimal_mod(base: &Path) {
    let rules_dir = base.join("rules");
    std::fs::create_dir_all(&rules_dir).expect("failed to create rules dir");

    let weapons_path = rules_dir.join("weapons.ron");
    let minimal_weapons = r#"[]"#;
    std::fs::write(&weapons_path, minimal_weapons).expect("failed to write minimal weapons.ron");
}

/// Helper: clean up a test mod directory.
fn cleanup_mod(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// "temp_dir" analog — return a unique path under the system temp dir.
fn temp_mod_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pb_mod_test_{}_{}", label, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ---------------------------------------------------------------------------
// zip_slip.mod — symlink inside mod pointing outside via relative ".."
// ---------------------------------------------------------------------------
#[cfg(unix)]
#[test]
fn zip_slip_symlink_returns_mod_path_error() {
    let mod_dir = temp_mod_dir("zip_slip");
    create_minimal_mod(&mod_dir);

    // Create a symlink inside the mod that points outside via ".."
    let outside_target = mod_dir.join("..").join("..").join("etc").join("passwd");
    let link_path = mod_dir.join("escape_link");
    symlink(&outside_target, &link_path).expect("failed to create zip_slip symlink");

    let result = load_mod(&mod_dir);
    cleanup_mod(&mod_dir);

    match result {
        Err(e) => {
            assert!(
                e.code == "E-MOD-PATH",
                "expected E-MOD-PATH code, got {}",
                e.code
            );
        }
        Ok(_) => panic!("expected E-MOD-PATH error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// symlink_escape.mod — symlink pointing to an absolute path outside the mod
// ---------------------------------------------------------------------------
#[cfg(unix)]
#[test]
fn symlink_escape_returns_mod_path_error() {
    let mod_dir = temp_mod_dir("symlink_escape");
    create_minimal_mod(&mod_dir);

    // Create a symlink inside the mod that points to an absolute path outside
    let etc_path = Path::new("/etc/passwd");
    let link_path = mod_dir.join("evil_link");
    symlink(etc_path, &link_path).expect("failed to create symlink escape");

    let result = load_mod(&mod_dir);
    cleanup_mod(&mod_dir);

    match result {
        Err(e) => {
            assert!(
                e.code == "E-MOD-PATH",
                "expected E-MOD-PATH code, got {}",
                e.code
            );
        }
        Ok(_) => panic!("expected E-MOD-PATH error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// Valid minimal mod loads without error
// ---------------------------------------------------------------------------
#[test]
fn minimal_valid_mod_loads_ok() {
    let mod_dir = temp_mod_dir("valid_mod");
    create_minimal_mod(&mod_dir);

    let result = load_mod(&mod_dir);
    cleanup_mod(&mod_dir);

    match result {
        Ok(content) => {
            // A minimal mod with empty weapons list should produce empty content
            assert!(
                content.weapons.is_empty(),
                "expected empty weapons in minimal mod"
            );
        }
        Err(e) => panic!("expected Ok for valid mod, got: {}: {}", e.code, e.message),
    }
}
