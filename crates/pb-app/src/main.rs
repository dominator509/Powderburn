//! See ARCHITECTURE.md for this crate's place in the import law.
#![forbid(unsafe_code)]

mod headless;
mod screens;
pub mod input;
pub mod settings;

use std::path::PathBuf;

/// Parse a --key=value style argument.
fn parse_arg(key: &str, args: &[String]) -> Option<String> {
    for arg in args {
        if let Some(value) = arg.strip_prefix(&format!("--{key}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn has_flag(key: &str, args: &[String]) -> bool {
    args.iter().any(|a| a == &format!("--{key}"))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check for headless mode
    if has_flag("headless", &args) {
        run_headless(&args).await;
        return;
    }

    // Default: print version
    println!("powderburn {}", env!("CARGO_PKG_VERSION"));
}

async fn run_headless(args: &[String]) {
    let mut config = headless::HeadlessConfig::default();

    if let Some(scenario) = parse_arg("scenario", args) {
        config.scenario = scenario;
    }
    if let Some(tick_str) = parse_arg("tick", args) {
        config.tick = tick_str.parse::<u64>().ok();
    }
    if let Some(capture_path) = parse_arg("capture", args) {
        config.capture = Some(PathBuf::from(capture_path));
    }

    match headless::run_headless_capture(&config).await {
        Ok(meta) => {
            println!("capture: wrote frame");
            println!("capture: dimensions {}x{}", meta.width, meta.height);
            println!("capture: checksum {}", meta.checksum);
        }
        Err(e) => {
            eprintln!("capture failed: {}", e);
            std::process::exit(1);
        }
    }
}
