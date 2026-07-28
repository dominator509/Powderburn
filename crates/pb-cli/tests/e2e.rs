//! End-to-end proofs for the shipped command surface.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("pb-cli must be nested under the workspace crates directory")
        .to_path_buf()
}

fn run_terminal_hash() -> String {
    let root = workspace_root();
    let output = Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(&root)
        .args([
            "sim",
            "--scenario",
            "content/scenarios/prov_full_battle.ron",
            "--seed",
            "1867",
            "--journal",
            "tests/journals/prov_full_battle.jrnl",
            "--emit-hash",
        ])
        .output()
        .expect("pbcli determinism proof must execute");

    assert!(
        output.status.success(),
        "pbcli determinism proof failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("pbcli output must be UTF-8")
        .lines()
        .find_map(|line| line.strip_prefix("state-hash: "))
        .expect("pbcli must emit a terminal state hash")
        .to_owned()
}

/// Same scenario, seed, and journal must produce the committed terminal hash
/// on three independent process runs.
#[test]
fn lb_integration_test_determinism() {
    let hashes = [
        run_terminal_hash(),
        run_terminal_hash(),
        run_terminal_hash(),
    ];
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[1], hashes[2]);

    let golden = include_str!("../../../tests/golden/prov_full_battle.hash").trim();
    assert_eq!(
        hashes[0], golden,
        "terminal hash drifted from golden corpus"
    );
}
