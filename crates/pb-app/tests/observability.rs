//! EP-008 M3/M6: a real child-process panic writes a self-contained,
//! redacted bundle whose journal reproduces the recorded state hash.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

fn parse_meta(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .expect("meta must read")
        .lines()
        .map(|line| {
            let (key, value) = line.split_once('=').expect("meta line must contain =");
            (
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            )
        })
        .collect()
}

#[test]
fn forced_panic_bundle_is_redacted_and_reproducible() {
    let config =
        std::env::temp_dir().join(format!("powderburn-crash-proof-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&config);
    let output = Command::new(env!("CARGO_BIN_EXE_powderburn"))
        .current_dir(workspace_root())
        .env("PB_CONFIG_DIR", &config)
        .args([
            "--headless",
            "--scenario",
            "prov_full_battle",
            "--seed",
            "42",
            "--force-panic-at-tick",
            "200",
        ])
        .output()
        .expect("powderburn child must execute");
    assert!(
        !output.status.success(),
        "forced panic unexpectedly succeeded"
    );

    let crash_root = config.join("crash");
    let mut bundles: Vec<_> = std::fs::read_dir(&crash_root)
        .expect("crash root must exist")
        .map(|entry| entry.expect("bundle entry must read").path())
        .collect();
    bundles.sort();
    assert_eq!(bundles.len(), 1, "expected exactly one crash bundle");
    let bundle = &bundles[0];
    for required in ["report.txt", "journal.jrnl", "meta.toml", "state.hash"] {
        assert!(bundle.join(required).is_file(), "missing {required}");
    }

    let report = std::fs::read_to_string(bundle.join("report.txt")).expect("report must read");
    let section_names: Vec<_> = report
        .lines()
        .filter_map(|line| {
            [
                "panic:",
                "backtrace:",
                "build:",
                "ruleset:",
                "content:",
                "events:",
            ]
            .iter()
            .find(|heading| line.starts_with(**heading))
            .map(|heading| heading.trim_end_matches(':'))
        })
        .collect();
    assert_eq!(
        section_names,
        [
            "panic",
            "backtrace",
            "build",
            "ruleset",
            "content",
            "events"
        ]
    );
    for forbidden in [
        std::env::var("HOME").ok(),
        std::env::var("USER").ok(),
        std::env::var("USERNAME").ok(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    {
        assert!(
            !report.contains(&forbidden),
            "report leaked forbidden value"
        );
    }
    assert!(!report.contains("PB_CONFIG_DIR"));
    assert!(!report.contains(".env"));

    let metadata = parse_meta(&bundle.join("meta.toml"));
    let seed = metadata["seed"].parse::<u64>().expect("seed must parse");
    let tick = metadata["terminal_tick"]
        .parse::<u64>()
        .expect("tick must parse");
    let snapshot = pb_cli::cmd_sim::capture_reproduction(
        &workspace_root().join("content"),
        &metadata["scenario"],
        seed,
        &bundle.join("journal.jrnl"),
        tick,
    )
    .expect("bundle journal must replay");
    let expected =
        std::fs::read_to_string(bundle.join("state.hash")).expect("state hash must read");
    assert_eq!(snapshot.state_hash, expected.trim());
    assert_eq!(snapshot.terminal_tick, tick);
}
