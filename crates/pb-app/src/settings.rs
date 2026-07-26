//! Settings persistence.
//!
//! Player settings stored as RON in $PB_CONFIG_DIR/settings.ron.

use serde::{Deserialize, Serialize};

/// Maximum allowed settings file size in bytes (64 KB).
pub const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// Player settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub text_scale: u32,
    pub color_palette: String,
    pub slow_clock: bool,
    pub subtitles: bool,
    pub camera_shake: bool,
    pub fullscreen: bool,
    pub resolution_width: u32,
    pub resolution_height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_volume: 0.7,
            sfx_volume: 0.8,
            text_scale: 100,
            color_palette: "default".to_string(),
            slow_clock: false,
            subtitles: true,
            camera_shake: true,
            fullscreen: false,
            resolution_width: 1920,
            resolution_height: 1080,
        }
    }
}
