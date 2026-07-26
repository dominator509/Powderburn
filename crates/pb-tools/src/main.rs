//! pbtool — POWDERBURN developer toolkit.
//! Subcommands: validate, golden, image, atlas, fuzz.

#![forbid(unsafe_code)]

use std::path::Path;

mod atlas;
mod fuzz;
mod golden;
mod image;
mod validate;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: pbtool <subcommand> [options]");
        eprintln!("Subcommands: validate, golden, image, atlas, fuzz");
        std::process::exit(1);
    }

    let subcommand = &args[1];

    let result = match subcommand.as_str() {
        "validate" => run_validate(&args[2..]),
        "golden" => run_golden(&args[2..]),
        "image" => run_image(&args[2..]),
        "atlas" => run_atlas(&args[2..]),
        "fuzz" => run_fuzz_command(&args[2..]),
        _ => {
            eprintln!("ERROR: unknown subcommand '{}'", subcommand);
            eprintln!("Subcommands: validate, golden, image, atlas, fuzz");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("ERROR: {}", e);
        std::process::exit(1);
    }
}

/// Dispatch validate sub-subcommands.
fn run_validate(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("content");
    let content_root = Path::new("content");

    match sub {
        "content" => {
            // Check for --with-fixture flag
            let fixture_path = get_flag_value(args, "--with-fixture");
            if let Some(fixture) = fixture_path {
                validate::validate_content_with_fixture(content_root, Path::new(&fixture))
            } else {
                validate::validate_content(content_root)
            }
        }
        "representation" => validate::validate_representation(content_root),
        "provenance" => {
            let asset_root = get_flag_value(args, "--asset-root")
                .map(|p| Path::new(&p).to_path_buf())
                .or_else(|| {
                    // Fallback: positional arg after "provenance"
                    if args.len() > 1 && !args[1].starts_with('-') {
                        Some(Path::new(&args[1]).to_path_buf())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| Path::new("assets").to_path_buf());
            validate::validate_provenance(&asset_root)
        }
        _ => {
            Err(format!(
                "unknown validate subcommand '{}'. Usage: pbtool validate <content|representation|provenance>",
                sub
            ))
        }
    }
}

/// Get the value of a --flag from the args slice (supports --flag <value>).
fn get_flag_value(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == flag {
            return iter.next().cloned();
        }
    }
    None
}

/// Dispatch golden sub-subcommands.
fn run_golden(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("refresh");
    let golden_path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };

    match sub {
        "refresh" => golden::golden_refresh(golden_path),
        _ => Err(format!("unknown golden subcommand '{}'", sub)),
    }
}

/// Dispatch image sub-subcommands.
fn run_image(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("stats");
    let image_path = args.get(1).map(Path::new);

    match sub {
        "stats" => {
            let path = image_path.ok_or_else(|| "image stats requires a file path".to_string())?;
            image::image_stats(path)
        }
        _ => Err(format!("unknown image subcommand '{}'", sub)),
    }
}

/// Dispatch atlas sub-subcommands.
fn run_atlas(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("pack");
    let output_dir = args
        .get(1)
        .map(Path::new)
        .unwrap_or_else(|| Path::new("atlas"));

    match sub {
        "pack" => {
            let input_files: Vec<String> = args[2..].to_vec();
            atlas::atlas_pack(output_dir, &input_files)
        }
        _ => Err(format!("unknown atlas subcommand '{}'", sub)),
    }
}

/// Simple arg-parsing helper: get a `--flag` value from a slice of args.
fn get_flag(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("--{}=", name);
    for a in args {
        if let Some(val) = a.strip_prefix(&prefix) {
            return Some(val.to_string());
        }
        if a == &format!("--{}", name) {
            // Value is the next argument
        }
    }
    // Also check positional after --flag
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == &format!("--{}", name) {
            return iter.next().cloned();
        }
    }
    None
}

/// Dispatch `pbtool fuzz` subcommands.
///
/// Usage:
///   pbtool fuzz content  --target <dir> --iters <n> --seed <n>
///   pbtool fuzz save     --target <dir> --iters <n> --seed <n>
///   pbtool fuzz journal  --target <dir> --iters <n> --seed <n>
fn run_fuzz_command(args: &[String]) -> Result<(), String> {
    let sub = args
        .first()
        .map(|s| s.as_str())
        .ok_or_else(|| "fuzz requires a subcommand: content, save, or journal".to_string())?;

    // Look for --target value: either --target=<path> or --target <path>
    let target = get_flag(&args[1..], "target").unwrap_or_else(|| {
        // Fallback: first non-flag arg after subcommand is the target path
        args.get(1)
            .filter(|s| !s.starts_with('-'))
            .cloned()
            .unwrap_or_else(|| ".".to_string())
    });
    let iters: usize = get_flag(&args[1..], "iters")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);
    let seed: u64 = get_flag(&args[1..], "seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(42);

    let target_path = Path::new(&target);

    match sub {
        "content" => {
            eprintln!("[pbtool fuzz content]");

            if target_path.exists() {
                eprintln!("  target: {}", target_path.display());
            } else {
                eprintln!("  target: {} (not found, generating synthetic data only)", target_path.display());
            }
            eprintln!("  iters:  {}", iters);
            eprintln!("  seed:   {}", seed);

            fuzz::run_fuzz(target_path, iters, seed);
            Ok(())
        }
        "save" => {
            eprintln!("[pbtool fuzz save]");

            if target_path.exists() {
                eprintln!("  target: {}", target_path.display());
            } else {
                eprintln!("  target: {} (not found, generating synthetic data only)", target_path.display());
            }
            eprintln!("  iters:  {}", iters);
            eprintln!("  seed:   {}", seed);

            fuzz::run_fuzz(target_path, iters, seed);
            Ok(())
        }
        "journal" => {
            eprintln!("[pbtool fuzz journal]");

            if target_path.exists() {
                eprintln!("  target: {}", target_path.display());
            } else {
                eprintln!("  target: {} (not found, generating synthetic data only)", target_path.display());
            }
            eprintln!("  iters:  {}", iters);
            eprintln!("  seed:   {}", seed);

            fuzz::run_fuzz(target_path, iters, seed);
            Ok(())
        }
        _ => Err(format!(
            "unknown fuzz subcommand '{}'. Usage: pbtool fuzz <content|save|journal> [--target <dir>] [--iters <n>] [--seed <n>]",
            sub
        )),
    }
}
