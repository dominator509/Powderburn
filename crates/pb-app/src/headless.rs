//! Headless mode for the powderburn binary.
//!
//! Runs the simulation and optionally captures frames without a window.

use std::path::PathBuf;

use pb_render::device::RenderDevice;
use pb_render::{CaptureMeta, RenderConfig};

/// Configuration for headless mode.
#[derive(Debug)]
pub struct HeadlessConfig {
    pub scenario: String,
    pub tick: Option<u64>,
    pub capture: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            scenario: String::from("prov_full_battle"),
            tick: None,
            capture: None,
            width: 1920,
            height: 1080,
        }
    }
}

/// Run a headless capture of the given scenario at the given tick.
///
/// Returns the capture metadata on success.
pub async fn run_headless_capture(config: &HeadlessConfig) -> Result<CaptureMeta, String> {
    // Initialize the render device
    let device = RenderDevice::new_headless().await?;

    // Build render config
    let render_config = RenderConfig {
        width: config.width,
        height: config.height,
        adapter_name: None,
    };

    // Capture the frame
    let capture_path = config
        .capture
        .as_ref()
        .ok_or_else(|| "no capture path specified".to_string())?;

    let meta = pb_render::capture::capture_frame(device, &render_config, capture_path).await?;

    Ok(meta)
}
