//! Verify that the 4 new adversarial fixtures produce the expected errors.
//! Run with: cargo test --test verify_adversarial_fixtures

use std::path::Path;

// ---------------------------------------------------------------------------
// 1. zip_slip.mod — regular file with '../../etc/passwd' reference
// ---------------------------------------------------------------------------
#[test]
fn zip_slip_mod_returns_e_mod_path() {
    let path = Path::new("tests/fixtures/adversarial/zip_slip.mod");
    let result = pb_content::mods::load_mod(path);
    assert!(result.is_err(), "zip_slip.mod should fail to load");
    let err = result.unwrap_err();
    assert_eq!(
        err.code, "E-MOD-PATH",
        "Expected E-MOD-PATH, got {}",
        err.code
    );
    // The file is not a directory, so read_dir fails
    assert!(
        err.message.contains("cannot read mod directory")
            || err.message.contains("cannot access mod directory"),
        "Expected directory read error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// 2. symlink_escape.mod — created at runtime by test setup
// ---------------------------------------------------------------------------
#[test]
fn symlink_escape_mod_returns_e_mod_path() {
    use std::os::unix;

    // Create the symlink in a temp dir so we don't pollute the repo
    let tmp = std::env::temp_dir().join("powderburn_symlink_escape_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    // Create a "mod root" directory with a symlink that escapes
    let mod_root = tmp.join("symlink_escape.mod");
    std::fs::create_dir_all(&mod_root).expect("create mod root");

    // Create a symlink inside the mod that points outside (to /etc/passwd)
    let bad_file = mod_root.join("escape_me");
    unix::fs::symlink("/etc/passwd", &bad_file).expect("create symlink");

    let result = pb_content::mods::load_mod(&mod_root);
    assert!(result.is_err(), "symlink_escape.mod should fail to load");
    let err = result.unwrap_err();
    assert_eq!(
        err.code, "E-MOD-PATH",
        "Expected E-MOD-PATH, got {}",
        err.code
    );
    // The canonicalized path of the symlink target (/etc/passwd) escapes the mod root
    assert!(
        err.message.contains("escapes the mod root"),
        "Expected path escape error, got: {}",
        err.message
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// 3. huge_declared_len.pbsave — save file with > 4096 ledger entries
// ---------------------------------------------------------------------------
#[test]
fn huge_declared_len_returns_e_save_oversize() {
    let path = Path::new("tests/fixtures/adversarial/huge_declared_len.pbsave");

    // These match the values embedded in the fixture file
    let ruleset_hash = [0xaa; 32];
    let content_hash = [0xbb; 32];

    let result = pb_save::load::read(path, &ruleset_hash, &content_hash);
    assert!(result.is_err(), "huge_declared_len.pbsave should fail");
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("E-SAVE-OVERSIZE"),
        "Expected E-SAVE-OVERSIZE, got: {}",
        err_str
    );
}

// ---------------------------------------------------------------------------
// 4. nan_field.ron — weapons.ron with float (1.5) where i32 belongs
// ---------------------------------------------------------------------------
#[test]
fn nan_field_ron_returns_parse_error() {
    let path = Path::new("tests/fixtures/adversarial/nan_field.ron");
    let content = std::fs::read_to_string(path).expect("read nan_field.ron");

    let result: Result<Vec<pb_content::schema::WeaponData>, ron::Error> = ron::from_str(&content);
    assert!(result.is_err(), "nan_field.ron should fail to parse");
    let err = result.unwrap_err();
    let err_str = err.to_string();

    // The error should mention the field name where the type mismatch occurred
    assert!(
        err_str.contains("accuracy"),
        "Expected parse error mentioning 'accuracy', got: {}",
        err_str
    );
}
