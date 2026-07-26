//! pbtool — POWDERBURN developer toolkit.
//! Subcommands: validate, golden, image, atlas.

#![forbid(unsafe_code)]

use std::path::Path;

mod validate;
mod golden;
mod image;
mod atlas;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: pbtool <subcommand> [options]");
        eprintln!("Subcommands: validate, golden, image, atlas");
        std::process::exit(1);
    }

    let subcommand = &args[1];

    let result = match subcommand.as_str() {
        "validate" => run_validate(&args[2..]),
        "golden" => run_golden(&args[2..]),
        "image" => run_image(&args[2..]),
        "atlas" => run_atlas(&args[2..]),
        _ => {
            eprintln!("ERROR: unknown subcommand '{}'", subcommand);
            eprintln!("Subcommands: validate, golden, image, atlas");
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
        "content" => validate::validate_content(content_root),
        "representation" => validate::validate_representation(content_root),
        "provenance" => {
            let asset_root = if args.len() > 1 {
                Path::new(&args[1])
            } else {
                Path::new("assets")
            };
            validate::validate_provenance(asset_root)
        }
        _ => {
            Err(format!(
                "unknown validate subcommand '{}'. Usage: pbtool validate <content|representation|provenance>",
                sub
            ))
        }
    }
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
        _ => Err(format!(
            "unknown golden subcommand '{}'",
            sub
        )),
    }
}

/// Dispatch image sub-subcommands.
fn run_image(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("stats");
    let image_path = args.get(1).map(|s| Path::new(s));

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
    let output_dir = args.get(1).map(|s| Path::new(s)).unwrap_or_else(|| Path::new("atlas"));

    match sub {
        "pack" => {
            let input_files: Vec<String> = args[2..].to_vec();
            atlas::atlas_pack(output_dir, &input_files)
        }
        _ => Err(format!("unknown atlas subcommand '{}'", sub)),
    }
}
