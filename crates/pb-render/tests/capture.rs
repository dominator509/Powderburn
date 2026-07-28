//! Real headless capture proofs for the renderer contract.

use std::path::PathBuf;

use pb_core::geom::TileXY;
use pb_render::capture::capture_state_frame;
use pb_render::device::RenderDevice;
use pb_render::RenderConfig;
use pb_sim::state::SimState;

fn output_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "powderburn-render-{}-{}-{}.png",
        std::process::id(),
        label,
        std::thread::current().name().unwrap_or("capture")
    ))
}

#[test]
fn empty_smoke_and_overlays_capture_without_panicking() -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let device = runtime.block_on(RenderDevice::new_headless())?;
    let config = RenderConfig {
        width: 256,
        height: 256,
        adapter_name: None,
    };
    let state = SimState::new(42, 1);
    let output = output_path("empty");

    let meta = runtime.block_on(capture_state_frame(device, &config, &state, &output))?;

    if meta.width != 256 || meta.height != 256 || meta.checksum.len() != 64 {
        return Err(format!(
            "unexpected capture metadata: {}x{}, hash length {}",
            meta.width,
            meta.height,
            meta.checksum.len()
        ));
    }
    let _ = std::fs::remove_file(output);
    Ok(())
}

#[test]
fn identical_state_has_identical_capture_hash_and_changed_state_does_not() -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let device = runtime.block_on(RenderDevice::new_headless())?;
    let config = RenderConfig {
        width: 256,
        height: 256,
        adapter_name: None,
    };
    let mut state = SimState::new(1867, 1);
    state.smoke_cols = 4;
    state.smoke_rows = 4;
    state.smoke_grid = vec![0; 16];
    let first = output_path("same-a");
    let second = output_path("same-b");
    let changed = output_path("changed");

    let first_meta =
        runtime.block_on(capture_state_frame(device.clone(), &config, &state, &first))?;
    let second_meta = runtime.block_on(capture_state_frame(
        device.clone(),
        &config,
        &state,
        &second,
    ))?;
    if first_meta.checksum != second_meta.checksum {
        return Err("identical state produced different capture hashes".to_string());
    }

    let mut changed_state = state;
    changed_state
        .terrain_tiles
        .insert(TileXY::new(1, 1), "Creek".to_string());
    changed_state.smoke_grid[10] = 192;
    let changed_meta = runtime.block_on(capture_state_frame(
        device,
        &config,
        &changed_state,
        &changed,
    ))?;
    if first_meta.checksum == changed_meta.checksum {
        return Err("changed state produced the identical capture hash".to_string());
    }

    for path in [first, second, changed] {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}
