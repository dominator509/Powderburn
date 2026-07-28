//! Capture subcommand for pbcli.
//! Renders a frame headlessly using wgpu and writes to --out.

#![allow(clippy::float_arithmetic)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use pb_sim::action::{Action, Command};
use pb_sim::clock::advance_to_next_actor;
use pb_sim::state::SimState;

use crate::args::Args;
use crate::output;

/// Run the `capture` subcommand.
///
/// Usage: pbcli capture --scenario <id> --seed <n> --out <path> [--bench]
pub fn run_capture(args: &Args) -> Result<(), String> {
    let scenario_arg = args
        .scenario
        .as_deref()
        .ok_or_else(|| "capture requires --scenario <id>".to_string())?;
    let scenario_id = crate::cmd_sim::resolve_scenario_id(scenario_arg);
    let state = state_for_capture(args, &scenario_id)?;

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
                .block_on(pb_render::capture::capture_state_frame(
                    device.clone(),
                    &config,
                    &state,
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
            .block_on(pb_render::capture::capture_state_frame(
                device,
                &config,
                &state,
                &output_path,
            ))
            .map_err(|e| format!("capture failed: {}", e))?;

        println!("{}{}", output::CAPTURE_OK, meta.checksum);
    }

    Ok(())
}

fn state_for_capture(args: &Args, scenario_id: &str) -> Result<SimState, String> {
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let seed = args.seed.unwrap_or(42);
    let requested_tick = args.suspend_at_tick.unwrap_or(0);
    let conventional = Path::new("tests/journals").join(format!("{scenario_id}.jrnl"));
    let journal = args
        .journal
        .as_deref()
        .or_else(|| conventional.is_file().then_some(conventional.as_path()));
    let mut state = if requested_tick > 0 {
        if let Some(journal) = journal {
            crate::cmd_sim::capture_reproduction(
                content_root,
                scenario_id,
                seed,
                journal,
                requested_tick,
            )?
            .state
        } else {
            crate::cmd_sim::construct_scenario_state(content_root, scenario_id, seed)?
        }
    } else {
        crate::cmd_sim::construct_scenario_state(content_root, scenario_id, seed)?
    };
    let mut steps = 0_u16;
    while state.tick.0 < requested_tick {
        steps = steps.saturating_add(1);
        if steps > 1_024 {
            return Err("E-CAPTURE-TICK: could not reach requested tick".to_string());
        }
        let Some(actor_id) = advance_to_next_actor(&mut state) else {
            break;
        };
        if state.tick.0 > requested_tick {
            break;
        }
        pb_sim::action::step(
            &mut state,
            Command {
                actor_id,
                action: Action::Hold,
            },
        )
        .map_err(|error| format!("E-CAPTURE-TICK: {error:?}"))?;
    }
    Ok(state)
}
