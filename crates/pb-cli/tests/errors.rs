//! SPEC-006: stable error codes and recovery-oriented process behavior.

#![allow(clippy::expect_used)]

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

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
        .expect("pbcli must execute")
}

fn player_diagnostic(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| line.starts_with("ERROR:") || line.starts_with("Usage:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn missing_required_flag_uses_e_cli_001_and_exit_two() {
    let output = run(&["replay"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        player_diagnostic(&output),
        concat!(
            "ERROR: E-CLI-001: replay requires --journal\n",
            "Usage: pbcli <sim|replay|capture|campaign|bench|selftest|a11y-report|serve-replay> [options]\n"
        )
    );
}

#[test]
fn missing_flag_value_uses_e_cli_001_and_exit_two() {
    let output = run(&["sim", "--seed"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        concat!(
            "ERROR: E-CLI-001: --seed/--company-seed requires a value\n",
            "Usage: pbcli <sim|replay|capture|campaign|bench|selftest|a11y-report|serve-replay> [options]\n"
        )
    );
}

#[test]
fn malformed_journal_emits_registered_parse_code() {
    let output = run(&[
        "replay",
        "--scenario",
        "content/scenarios/prov_called_shot.ron",
        "--journal",
        "tests/fixtures/adversarial/deep_nest.ron",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E-JOURNAL-PARSE:"),
        "unexpected diagnostic: {stderr}"
    );
}

#[test]
fn unknown_subcommand_is_nonzero_and_names_command() {
    let output = run(&["not-a-command"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown subcommand 'not-a-command'"));
    assert!(stderr.contains("Subcommands: sim, replay, campaign"));
}
