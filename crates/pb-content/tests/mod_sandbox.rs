//! SPEC-005 section 3: data mods cannot execute code or escape limits.

#![allow(clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use pb_content::{
    load::load_all,
    mods::{load_mod, load_mod_set, MAX_NEST_DEPTH},
};

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

fn minimal_mod(name: &str) -> (PathBuf, Cleanup) {
    let root = test_root(name);
    let cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join("rules")).expect("mod rules directory must be created");
    fs::write(root.join("rules/weapons.ron"), b"[]").expect("minimal rules file must be written");
    (root, cleanup)
}

fn write_manifest(root: &Path, id: &str, load_order: i32, dependencies: &[&str]) {
    let dependencies = dependencies
        .iter()
        .map(|dependency| format!("\"{dependency}\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        root.join("mod.ron"),
        format!("(id:\"{id}\",load_order:{load_order},dependencies:[{dependencies}])"),
    )
    .expect("mod manifest must be written");
}

fn create_weapon_override(
    mods_root: &Path,
    id: &str,
    load_order: i32,
    dependencies: &[&str],
    display_name: &str,
) {
    let root = mods_root.join(id);
    fs::create_dir_all(root.join("rules")).expect("mod rules directory must be created");
    write_manifest(&root, id, load_order, dependencies);
    let shipped = load_all(Path::new("../../content")).expect("shipped content must load");
    let mut weapon = shipped
        .weapons
        .get("colt_army_1860")
        .expect("shipped Colt must exist")
        .clone();
    weapon.display_name = display_name.to_string();
    let ron = ron::ser::to_string(&vec![weapon]).expect("weapon override must serialize");
    fs::write(root.join("rules/weapons.ron"), ron).expect("weapon override must be written");
}

#[test]
fn executable_magic_is_rejected() {
    for (name, bytes) in [
        ("elf", b"\x7fELFpayload".as_slice()),
        ("pe", b"MZpayload".as_slice()),
        ("script", b"#!/bin/sh\nexit 0\n".as_slice()),
    ] {
        let (root, _cleanup) = minimal_mod(name);
        fs::write(root.join(format!("{name}.bin")), bytes)
            .expect("executable fixture must be written");
        let error = load_mod(&root).expect_err("executable mod file must be refused");
        assert_eq!(error.code, "E-MOD-EXEC");
    }
}

#[test]
fn excessive_directory_depth_is_rejected() {
    let (root, _cleanup) = minimal_mod("deep-mod");
    let mut directory = root.clone();
    for index in 0..=MAX_NEST_DEPTH {
        directory = directory.join(format!("d{index}"));
        fs::create_dir(&directory).expect("nested fixture must be created");
    }
    fs::write(directory.join("record.ron"), b"[]").expect("deep record must be written");

    let error = load_mod(&root).expect_err("over-nested mod must be refused");
    assert_eq!(error.code, "E-MOD-PATH");
    assert!(
        error.message.contains("nesting depth"),
        "unexpected error: {}",
        error.message
    );
}

#[test]
fn dependencies_override_numeric_load_order_and_merge_atomically() {
    let mods_root = test_root("ordered-mods");
    let _cleanup = Cleanup(mods_root.clone());
    create_weapon_override(&mods_root, "foundation", 100, &[], "Foundation Colt");
    create_weapon_override(
        &mods_root,
        "dependent",
        -100,
        &["foundation"],
        "Dependent Colt",
    );
    let shipped = load_all(Path::new("../../content")).expect("shipped content must load");

    let loaded = load_mod_set(
        &shipped,
        &mods_root,
        &["dependent".to_string(), "foundation".to_string()],
    )
    .expect("valid dependency graph must load");

    assert_eq!(loaded.load_order, ["foundation", "dependent"]);
    assert_eq!(
        loaded.content.weapons["colt_army_1860"].display_name,
        "Dependent Colt"
    );
    assert_eq!(
        shipped.weapons["colt_army_1860"].display_name, "Colt Model 1860 Army",
        "the shipped input must remain unchanged"
    );
}

#[test]
fn missing_dependency_and_cycle_are_refused() {
    let missing_root = test_root("missing-dependency");
    let _missing_cleanup = Cleanup(missing_root.clone());
    create_weapon_override(&missing_root, "child", 0, &["absent"], "Child Colt");
    let shipped = load_all(Path::new("../../content")).expect("shipped content must load");
    let error = load_mod_set(&shipped, &missing_root, &["child".to_string()])
        .expect_err("missing dependency must fail");
    assert_eq!(error.code, "E-MOD-PATH");
    assert!(error.message.contains("missing dependencies"));

    let cycle_root = test_root("dependency-cycle");
    let _cycle_cleanup = Cleanup(cycle_root.clone());
    create_weapon_override(&cycle_root, "alpha", 0, &["beta"], "Alpha Colt");
    create_weapon_override(&cycle_root, "beta", 0, &["alpha"], "Beta Colt");
    let error = load_mod_set(
        &shipped,
        &cycle_root,
        &["alpha".to_string(), "beta".to_string()],
    )
    .expect_err("dependency cycle must fail");
    assert_eq!(error.code, "E-MOD-PATH");
    assert!(error.message.contains("cycle"));
}

#[test]
fn historical_fixed_scenario_override_is_refused() {
    let mods_root = test_root("historical-override");
    let _cleanup = Cleanup(mods_root.clone());
    let root = mods_root.join("history_rewrite");
    fs::create_dir_all(root.join("scenarios")).expect("scenario directory must be created");
    write_manifest(&root, "history_rewrite", 0, &[]);
    let shipped = load_all(Path::new("../../content")).expect("shipped content must load");
    let scenario = shipped
        .scenarios
        .get("scn_m02_promontory")
        .expect("historical scenario must exist");
    let ron = ron::ser::to_string(scenario).expect("scenario override must serialize");
    fs::write(root.join("scenarios/override.ron"), ron).expect("scenario override must be written");

    let error = load_mod_set(&shipped, &mods_root, &["history_rewrite".to_string()])
        .expect_err("historical rewrite must fail");
    assert_eq!(error.code, "E-HIST-001");
}
