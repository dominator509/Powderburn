//! Capture subcommand for pbcli.
//! Renders a frame headlessly using wgpu and writes to --out.

use std::path::PathBuf;

use crate::args::Args;
use crate::output;

/// Run the `capture` subcommand.
///
/// Usage: pbcli capture --scenario <id> --seed <n> --out <path>
pub fn run_capture(args: &Args) -> Result<(), String> {
    let _scenario_id = args
        .scenario
        .as_deref()
        .ok_or_else(|| "capture requires --scenario <id>".to_string())?;

    let _seed = args.seed.unwrap_or(42);

    let output_path: PathBuf = args
        .output
        .clone()
        .ok_or_else(|| "capture requires --out <path>".to_string())?;

    let config = pb_render::RenderConfig {
        width: 1920,
        height: 1080,
        adapter_name: args.adapter.clone(),
    };

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create tokio runtime: {}", e))?;

    let device = rt
        .block_on(pb_render::device::RenderDevice::new_headless())
        .map_err(|e| format!("failed to create headless device: {}", e))?;

    let meta = rt
        .block_on(pb_render::capture::capture_frame(
            device,
            &config,
            &output_path,
        ))
        .map_err(|e| format!("capture failed: {}", e))?;

    println!("{}{}", output::CAPTURE_OK, meta.checksum);
    Ok(())
}
