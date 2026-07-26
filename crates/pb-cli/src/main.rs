//! pbcli — POWDERBURN Command-Line Interface
//! Subcommand dispatcher for simulation, replay, campaign, bench, and selftest.

#![forbid(unsafe_code)]

use std::process;

fn main() {
    let args = match pb_cli::args::Args::from_env() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            process::exit(1);
        }
    };

    let result = match args.subcommand.as_str() {
        "sim" => pb_cli::cmd_sim::run_sim(&args),
        "replay" => pb_cli::cmd_replay::run_replay(&args),
        "campaign" => run_campaign(&args),
        "bench" => run_bench(&args),
        "selftest" => pb_cli::cmd_selftest::run_selftest(&args),
        "" => {
            eprintln!("Usage: pbcli <subcommand> [options]");
            eprintln!("Subcommands: sim, replay, campaign, bench, selftest");
            process::exit(1);
        }
        other => {
            eprintln!("ERROR: unknown subcommand '{}'", other);
            eprintln!("Subcommands: sim, replay, campaign, bench, selftest");
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("ERROR: {}", e);
        process::exit(1);
    }
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
        _ => {
            eprintln!("Usage: pbcli bench turn [options]");
            Err("unknown bench subcommand".to_string())
        }
    }
}
