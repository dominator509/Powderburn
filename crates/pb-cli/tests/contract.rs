//! SPEC-003 section 1: exact machine-readable CLI sentinels.

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

fn successful_stdout(arguments: &[&str]) -> String {
    let output = run(arguments);
    assert!(
        output.status.success(),
        "pbcli {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert!(fields.len() >= 7, "malformed structured log: {line}");
        assert!(fields[0].ends_with('Z'), "log timestamp is not UTC: {line}");
        assert!(
            matches!(fields[1], "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE"),
            "invalid log level: {line}"
        );
        assert!(fields.iter().any(|field| field.starts_with("build=")));
        assert!(fields.iter().any(|field| field.starts_with("ruleset=")));
        assert!(fields.iter().any(|field| field.starts_with("content=")));
        assert!(fields.iter().any(|field| field.starts_with("msg=\"")));
    }
    String::from_utf8(output.stdout).expect("CLI stdout must be UTF-8")
}

#[test]
fn selftest_emits_exact_hash_and_success_lines() {
    let stdout = successful_stdout(&["selftest", "--emit-hash"]);
    assert_eq!(
        stdout,
        concat!(
            "state-hash: 14ff9dfa13c6612760ad12b1865fe472a6bcb0445d35f47ec290346ccaec3d91\n",
            "selftest: ok\n"
        )
    );
}

#[test]
fn accessibility_report_emits_all_seven_checks_and_terminal_sentinel() {
    let stdout = successful_stdout(&["a11y-report"]);
    assert_eq!(
        stdout,
        concat!(
            "a11y: PASS color-only\n",
            "a11y: PASS text-scale\n",
            "a11y: PASS keyboard\n",
            "a11y: PASS palette\n",
            "a11y: PASS flashing\n",
            "a11y: PASS subtitles\n",
            "a11y: PASS slow-clock\n",
            "a11y: ok\n"
        )
    );
}

#[test]
fn called_shot_event_lines_are_byte_for_byte_stable() {
    let stdout = successful_stdout(&[
        "sim",
        "--scenario",
        "content/scenarios/prov_called_shot.ron",
        "--seed",
        "3",
        "--journal",
        "tests/journals/prov_called_shot.jrnl",
        "--emit-events",
    ]);
    assert_eq!(
        stdout,
        concat!(
            "event: TurnBegin actor=e_shooter tick=0\n",
            "event: Fired actor=e_shooter target=e_bandit_02\n",
            "event: ShotHit actor=e_shooter target=e_bandit_02 hit=true\n",
            "event: HitLocation actor=e_bandit_02 location=GunArm\n",
            "event: DamageApplied actor=e_bandit_02 damage=10\n",
            "event: WoundApplied actor=e_bandit_02 wound=Broken\n",
            "event: WeaponDropped actor=e_bandit_02 item=colt_army_1860\n",
            "event: SandLost actor=e_bandit_02 amount=4\n",
            "event: SmokeDeposited tile=(5, 5) density=3\n",
            "event: SmokeDeposited tile=(6, 4) density=1\n",
            "event: SmokeDeposited tile=(7, 3) density=1\n",
            "event: TurnEnd actor=e_shooter tick=0\n",
            "event: TurnBegin actor=e_bandit_02 tick=0\n",
            "event: TurnEnd actor=e_bandit_02 tick=0\n",
            "event: ObjectiveComplete id=obj_called_shot\n",
            "event: ScenarioEnded outcome=VICTORY\n"
        )
    );
}
