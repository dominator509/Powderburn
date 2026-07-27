//! Capture subcommand for pbcli.
//! Renders a frame headlessly using wgpu and writes to --out.

use std::path::PathBuf;
use std::time::Instant;

use crate::args::Args;
use crate::output;

/// Run the `capture` subcommand.
///
/// Usage: pbcli capture --scenario <id> --seed <n> --out <path> [--bench]
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

    // Measure timing if --bench is requested
    if args.capture_bench {
        // Run several frames and measure p95 render time
        let num_frames = 60usize;
        let mut frame_times_ms: Vec<f64> = Vec::with_capacity(num_frames);

        // Re-create scene for each frame via capture
        for _ in 0..num_frames {
            let frame_start = Instant::now();
            let _meta = rt
                .block_on(pb_render::capture::capture_frame(
                    device.clone(),
                    &config,
                    &output_path,
                ))
                .map_err(|e| format!("capture failed: {}", e))?;
            let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            frame_times_ms.push(elapsed_ms);
        }

        frame_times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p95_idx = ((num_frames as f64) * 0.95).ceil() as usize;
        let p95_idx = p95_idx.clamp(1, num_frames) - 1;
        let p95_ms = frame_times_ms[p95_idx];

        println!("{}{:.3}", output::P95_FRAME_MS, p95_ms);
        if p95_ms > 16.0 {
            eprintln!(
                "WARNING: p95 frame time {:.3}ms exceeds 16ms budget",
                p95_ms
            );
        }
    } else {
        // Normal capture: single frame, no timing
        let meta = rt
            .block_on(pb_render::capture::capture_frame(
                device,
                &config,
                &output_path,
            ))
            .map_err(|e| format!("capture failed: {}", e))?;

        println!("{}{}", output::CAPTURE_OK, meta.checksum);
    }

    Ok(())
}
