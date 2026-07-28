//! pbcli — POWDERBURN Command-Line Interface
//! Subcommand dispatcher for simulation, replay, campaign, bench, and selftest.

#![forbid(unsafe_code)]

use std::process;

fn main() {
    let args = match pb_cli::args::Args::from_env() {
        Ok(a) => a,
        Err(e) => {
            report_error_and_exit(&e);
        }
    };
    let logger = pb_cli::observability::logger_for(&args);
    let now = pb_cli::observability::unix_now();
    let _ = logger.log(
        now,
        pb_cli::observability::LogLevel::Info,
        "pb_cli",
        &[(pb_cli::observability::LogField::Event, "start".to_string())],
        &format!("{} command started", args.subcommand),
    );

    let result = match args.subcommand.as_str() {
        "sim" => pb_cli::cmd_sim::run_sim(&args),
        "replay" => pb_cli::cmd_replay::run_replay(&args),
        "campaign" => run_campaign(&args),
        "bench" => run_bench(&args),
        "selftest" => pb_cli::cmd_selftest::run_selftest(&args),
        "a11y-report" => pb_cli::cmd_a11y::run_a11y_report(),
        "capture" => pb_cli::cmd_capture::run_capture(&args),
        "serve-replay" => run_replay_server(&args),
        "" => {
            eprintln!("Usage: pbcli <subcommand> [options]");
            eprintln!(
                "Subcommands: sim, replay, campaign, bench, selftest, a11y-report, capture, serve-replay"
            );
            process::exit(1);
        }
        other => {
            eprintln!("ERROR: unknown subcommand '{}'", other);
            eprintln!(
                "Subcommands: sim, replay, campaign, bench, selftest, a11y-report, capture, serve-replay"
            );
            process::exit(1);
        }
    };

    if let Err(e) = &result {
        let _ = logger.log(
            pb_cli::observability::unix_now(),
            pb_cli::observability::LogLevel::Error,
            "pb_cli",
            &[(
                pb_cli::observability::LogField::Event,
                "failure".to_string(),
            )],
            e,
        );
        report_error_and_exit(e);
    }
    let _ = logger.log(
        pb_cli::observability::unix_now(),
        pb_cli::observability::LogLevel::Info,
        "pb_cli",
        &[(
            pb_cli::observability::LogField::Event,
            "complete".to_string(),
        )],
        &format!("{} command completed", args.subcommand),
    );
    let trace_fields = [
        (
            pb_cli::observability::LogField::Scenario,
            args.scenario
                .clone()
                .unwrap_or_else(|| "health_selftest".to_string()),
        ),
        (
            pb_cli::observability::LogField::Tick,
            args.suspend_at_tick.unwrap_or(0).to_string(),
        ),
        (
            pb_cli::observability::LogField::Seed,
            args.seed.unwrap_or(42).to_string(),
        ),
    ];
    let _ = logger.log(
        pb_cli::observability::unix_now(),
        pb_cli::observability::LogLevel::Trace,
        "pb_sim",
        &trace_fields,
        "deterministic command boundary",
    );
}

#[cfg(feature = "replay-server")]
fn run_replay_server(args: &pb_cli::args::Args) -> Result<(), String> {
    pb_cli::cmd_replay_server::run(args)
}

#[cfg(not(feature = "replay-server"))]
fn run_replay_server(_args: &pb_cli::args::Args) -> Result<(), String> {
    Err(
        "E-FEATURE-001: serve-replay is disabled; rebuild pb-cli with --features replay-server"
            .to_string(),
    )
}

fn report_error_and_exit(message: &str) -> ! {
    let missing_required = message.contains(" requires --")
        || message.contains(" requires a value")
        || message.starts_with("missing required ");

    if missing_required {
        eprintln!("ERROR: E-CLI-001: {message}");
        eprintln!(
            "Usage: pbcli <sim|replay|capture|campaign|bench|selftest|a11y-report|serve-replay> [options]"
        );
        process::exit(2);
    }

    eprintln!("ERROR: {message}");
    process::exit(1);
}

/// Dispatch campaign sub-subcommands.
fn run_campaign(args: &pb_cli::args::Args) -> Result<(), String> {
    let sub = args.positional.get(1).map(|s| s.as_str()).unwrap_or("");
    match sub {
        "new" => pb_cli::cmd_campaign::run_campaign_new(args),
        "play" => pb_cli::cmd_campaign::run_campaign_play(args),
        "audit" => pb_cli::cmd_campaign::run_campaign_audit(args),
        _ => {
            eprintln!("Usage: pbcli campaign <new|play|audit> [options]");
            Err("unknown campaign subcommand".to_string())
        }
    }
}

/// Dispatch bench sub-subcommands.
fn run_bench(args: &pb_cli::args::Args) -> Result<(), String> {
    let sub = args.positional.get(1).map(|s| s.as_str()).unwrap_or("");
    match sub {
        "turn" => pb_cli::cmd_bench::run_bench(args),
        "frame" => pb_cli::cmd_bench::run_bench_frame(args),
        _ => {
            eprintln!("Usage: pbcli bench <turn|frame> [options]");
            Err("unknown bench subcommand".to_string())
        }
    }
}
