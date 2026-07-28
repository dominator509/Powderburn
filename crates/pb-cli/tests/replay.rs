//! SPEC-003 section 3: replay equality and journal legality through the binary.

#![allow(clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const GOLDEN: &str = include_str!("../../../tests/golden/prov_full_battle.hash");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(workspace_root())
        .args(arguments)
        .output()
        .expect("pbcli replay must execute")
}

#[test]
fn real_journal_matches_committed_golden_hash() {
    let output = run(&[
        "replay",
        "--scenario",
        "content/scenarios/prov_full_battle.ron",
        "--seed",
        "1867",
        "--journal",
        "tests/journals/prov_full_battle.jrnl",
        "--expect",
        GOLDEN.trim(),
    ]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "replay: match\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains(" INFO pb_cli "));
}

#[test]
fn replay_reports_terminal_tick_for_hash_mismatch() {
    let output = run(&[
        "replay",
        "--scenario",
        "content/scenarios/prov_full_battle.ron",
        "--seed",
        "1867",
        "--journal",
        "tests/journals/prov_full_battle.jrnl",
        "--expect",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "replay: differ at tick 320\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains(" INFO pb_cli "));
}

#[test]
fn replay_and_sim_construct_identical_authored_state() {
    let common = [
        "--scenario",
        "content/scenarios/prov_full_battle.ron",
        "--seed",
        "1867",
        "--journal",
        "tests/journals/prov_full_battle.jrnl",
    ];
    let sim = run(&[
        "sim",
        common[0],
        common[1],
        common[2],
        common[3],
        common[4],
        common[5],
        "--emit-hash",
    ]);
    let replay = run(&[
        "replay", common[0], common[1], common[2], common[3], common[4], common[5],
    ]);
    assert!(
        sim.status.success(),
        "sim failed: {}",
        String::from_utf8_lossy(&sim.stderr)
    );
    assert!(
        replay.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&sim.stdout),
        String::from_utf8_lossy(&replay.stdout)
    );
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn replay_refuses_command_at_illegal_tick() {
    let cache = std::env::var_os("PB_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join(".pbcache"));
    let directory = cache
        .join("test")
        .join(format!("replay-illegal-{}", std::process::id()));
    let _cleanup = Cleanup(directory.clone());
    fs::create_dir_all(&directory).expect("test directory must be created");
    let journal = directory.join("wrong-tick.jrnl");
    fs::write(
        &journal,
        "1 1679563958 CalledShot target=347187621 loc=GunArm\n",
    )
    .expect("illegal journal fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(workspace_root())
        .args([
            "replay",
            "--scenario",
            "content/scenarios/prov_called_shot.ron",
            "--seed",
            "3",
            "--journal",
        ])
        .arg(&journal)
        .output()
        .expect("pbcli replay must execute");

    assert!(!output.status.success(), "illegal journal was accepted");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "E-JOURNAL-ILLEGAL: recorded tick=1 actor=1679563958, scheduler selected tick=0 actor=1679563958"
        ),
        "unexpected replay diagnostic: {stderr}"
    );
}
