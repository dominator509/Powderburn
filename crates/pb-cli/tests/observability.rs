//! SPEC-007 sections 3 and 4: health sentinel and complete metric vocabulary.

#![allow(clippy::expect_used)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

#[test]
fn selftest_emits_every_locked_metric_once() {
    let output = Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(workspace_root())
        .args(["selftest", "--emit-metrics"])
        .output()
        .expect("pbcli selftest must execute");
    assert!(
        output.status.success(),
        "selftest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "selftest: ok\n");

    let stderr = String::from_utf8(output.stderr).expect("metrics must be UTF-8");
    let mut metrics = BTreeMap::new();
    for line in stderr.lines().filter(|line| line.starts_with("metric: ")) {
        let mut fields = line.split_whitespace();
        assert_eq!(fields.next(), Some("metric:"));
        let name = fields.next().expect("metric name is required");
        let value = fields.next().expect("metric value is required");
        assert!(fields.next().is_none(), "unexpected metric fields: {line}");
        assert!(
            value.parse::<u64>().is_ok(),
            "metric value is not an integer: {line}"
        );
        assert!(
            metrics.insert(name, value).is_none(),
            "duplicate metric: {name}"
        );
    }

    let names: Vec<_> = metrics.keys().copied().collect();
    assert_eq!(
        names,
        vec![
            "ai.turn.ms",
            "content.load.ms",
            "render.frame.ms",
            "rng.draws.per_turn",
            "save.size.bytes",
            "save.write.ms",
            "sim.events.per_turn",
            "sim.step.ms",
            "smoke.volumes.live",
        ]
    );
}

#[test]
fn structured_logging_honors_target_filter_and_required_fields() {
    let root = std::env::temp_dir().join(format!("powderburn-cli-log-{}", std::process::id()));
    let default_dir = root.join("default");
    let trace_dir = root.join("trace");
    let run = |config: &Path, filter: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pbcli"));
        command
            .current_dir(workspace_root())
            .args(["selftest"])
            .env("PB_CONFIG_DIR", config);
        if let Some(filter) = filter {
            command.env("PB_LOG", filter);
        }
        let output = command.output().expect("pbcli must execute");
        assert!(
            output.status.success(),
            "selftest failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::read_to_string(config.join("log/powderburn.log"))
            .expect("structured log must exist")
    };
    let default_log = run(&default_dir, None);
    let trace_log = run(&trace_dir, Some("pb_sim=trace"));
    assert!(
        trace_log.lines().count() > default_log.lines().count(),
        "pb_sim=trace did not increase output volume"
    );
    for line in trace_log.lines() {
        assert!(line.contains(" build="), "missing build field: {line}");
        assert!(line.contains(" ruleset="), "missing ruleset field: {line}");
        assert!(line.contains(" content="), "missing content field: {line}");
    }
    let battle = trace_log
        .lines()
        .find(|line| line.contains(" TRACE pb_sim "))
        .expect("trace filter must emit a battle record");
    assert!(battle.contains(" scenario="));
    assert!(battle.contains(" tick="));
    assert!(battle.contains(" seed="));
}

#[test]
fn production_rotation_moves_a_nine_mib_log_and_keeps_a_new_active_file() {
    use pb_cli::observability::{BuildMetadata, LogFilter, LogLevel, Logger};

    let root = std::env::temp_dir().join(format!("powderburn-log-rotation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let logger = Logger::new(
        &root,
        &workspace_root(),
        BuildMetadata::new("123456789abc", "11223344", "55667788"),
        LogFilter::parse(None),
        Vec::new(),
    );
    let log_path = logger.path();
    std::fs::create_dir_all(log_path.parent().expect("log parent"))
        .expect("log directory must be created");
    let oversized = std::fs::File::create(log_path).expect("active log must be created");
    oversized
        .set_len(9 * 1024 * 1024)
        .expect("9 MiB proof log must be written");
    drop(oversized);

    assert!(logger
        .log(0, LogLevel::Info, "pb_cli", &[], "rotation proof")
        .expect("logger must rotate"));
    assert!(log_path.is_file(), "new active log is missing");
    assert!(
        log_path.with_extension("log.1").is_file(),
        "9 MiB log was not moved to powderburn.log.1"
    );
    assert!(
        std::fs::metadata(log_path)
            .expect("new active metadata")
            .len()
            < 8 * 1024 * 1024
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trace_exposes_all_shot_stages_ai_candidates_and_rng_addresses_without_perturbing_hash() {
    let base_arguments = [
        "sim",
        "--scenario",
        "prov_called_shot",
        "--seed",
        "3",
        "--journal",
        "tests/journals/prov_called_shot.jrnl",
        "--emit-hash",
    ];
    let plain = Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(workspace_root())
        .args(base_arguments)
        .output()
        .expect("plain sim must execute");
    assert!(plain.status.success());

    let traced = Command::new(env!("CARGO_BIN_EXE_pbcli"))
        .current_dir(workspace_root())
        .args(base_arguments)
        .args(["--trace-shot", "--trace-rng", "--trace-actor", "e_shooter"])
        .output()
        .expect("traced sim must execute");
    assert!(
        traced.status.success(),
        "trace failed: {}",
        String::from_utf8_lossy(&traced.stderr)
    );
    let plain_stdout = String::from_utf8(plain.stdout).expect("plain output must be UTF-8");
    let traced_stdout = String::from_utf8(traced.stdout).expect("trace output must be UTF-8");
    let plain_hash = plain_stdout
        .lines()
        .find(|line| line.starts_with("state-hash: "))
        .expect("plain hash is required");
    let traced_hash = traced_stdout
        .lines()
        .find(|line| line.starts_with("state-hash: "))
        .expect("traced hash is required");
    assert_eq!(plain_hash, traced_hash, "trace perturbed simulation state");

    let stages: Vec<_> = traced_stdout
        .lines()
        .filter(|line| line.starts_with("stage: "))
        .collect();
    assert_eq!(stages.len(), 10, "one shot must expose all ten stages");
    for number in 1..=10 {
        assert!(
            stages
                .iter()
                .any(|line| line.starts_with(&format!("stage: {number} "))),
            "missing stage {number}"
        );
    }
    assert!(traced_stdout.contains("candidate: actor=e_shooter"));
    assert!(traced_stdout.contains("chosen: actor=e_shooter"));
    let draws: Vec<_> = traced_stdout
        .lines()
        .filter(|line| line.starts_with("draw "))
        .collect();
    assert!(!draws.is_empty(), "real shot emitted no RNG draws");
    for draw in draws {
        for field in [
            "seed=",
            "scenario=",
            "tick=",
            "actor=",
            "stream=",
            "index=",
            "lo=",
            "hi=",
            "value=",
        ] {
            assert!(draw.contains(field), "draw is missing {field}: {draw}");
        }
    }
}
