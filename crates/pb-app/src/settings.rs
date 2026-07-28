//! Settings persistence.
//!
//! Player settings stored as RON in $PB_CONFIG_DIR/settings.ron.

use std::path::{Path, PathBuf};

pub use pb_render::ui_contract::UiPalette;
use serde::{Deserialize, Serialize};

/// Maximum allowed settings file size in bytes (64 KB).
pub const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// Player settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub text_scale: u32,
    pub color_palette: String,
    pub slow_clock: bool,
    pub subtitles: bool,
    pub camera_shake: bool,
    pub flashing_effects: bool,
    pub screen_fill_effects: bool,
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
            flashing_effects: true,
            screen_fill_effects: true,
            fullscreen: false,
            resolution_width: 1920,
            resolution_height: 1080,
        }
    }
}

impl Settings {
    pub fn palette(&self) -> UiPalette {
        pb_render::ui_contract::palette_for_name(&self.color_palette)
    }
}

/// Resolve the only directory where the shipped client may write settings.
pub fn config_dir() -> Result<PathBuf, String> {
    let value = std::env::var_os("PB_CONFIG_DIR")
        .ok_or_else(|| "E-CONFIG-PATH: PB_CONFIG_DIR is not set".to_string())?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("E-CONFIG-PATH: PB_CONFIG_DIR must be absolute".to_string());
    }
    Ok(path)
}

/// Load settings, replacing each out-of-range field with its documented
/// default and returning the names of fields that were repaired.
pub fn load_from(path: &Path) -> Result<(Settings, Vec<&'static str>), String> {
    if !path.exists() {
        return Ok((Settings::default(), Vec::new()));
    }
    let metadata = std::fs::metadata(path).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    if metadata.len() > MAX_SETTINGS_BYTES {
        return Err("E-CONFIG-OVERSIZE: settings.ron exceeds 64 KiB".to_string());
    }
    let text = std::fs::read_to_string(path).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    let settings: Settings =
        ron::from_str(&text).map_err(|error| format!("E-CONFIG-PARSE: {error}"))?;
    Ok(normalize(settings))
}

pub fn load() -> Result<(Settings, Vec<&'static str>), String> {
    load_from(&config_dir()?.join("settings.ron"))
}

/// Persist settings under the configured root with mode 0644 on Unix.
pub fn save_to(path: &Path, settings: &Settings) -> Result<(), String> {
    let normalized = normalize(settings.clone()).0;
    let parent = path
        .parent()
        .ok_or_else(|| "E-CONFIG-PATH: settings path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    let text = ron::ser::to_string_pretty(&normalized, ron::ser::PrettyConfig::default())
        .map_err(|error| format!("E-CONFIG-FORMAT: {error}"))?;
    std::fs::write(path, text).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
            .map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    }
    Ok(())
}

pub fn save(settings: &Settings) -> Result<(), String> {
    save_to(&config_dir()?.join("settings.ron"), settings)
}

fn normalize(mut settings: Settings) -> (Settings, Vec<&'static str>) {
    let defaults = Settings::default();
    let mut repaired = Vec::new();
    if !settings.music_volume.is_finite() || !(0.0..=1.0).contains(&settings.music_volume) {
        settings.music_volume = defaults.music_volume;
        repaired.push("music_volume");
    }
    if !settings.sfx_volume.is_finite() || !(0.0..=1.0).contains(&settings.sfx_volume) {
        settings.sfx_volume = defaults.sfx_volume;
        repaired.push("sfx_volume");
    }
    if !(100..=200).contains(&settings.text_scale) {
        settings.text_scale = defaults.text_scale;
        repaired.push("text_scale");
    }
    if !matches!(
        settings.color_palette.as_str(),
        "default" | "deuteranopia" | "tritanopia"
    ) {
        settings.color_palette = defaults.color_palette;
        repaired.push("color_palette");
    }
    if !(800..=7680).contains(&settings.resolution_width) {
        settings.resolution_width = defaults.resolution_width;
        repaired.push("resolution_width");
    }
    if !(600..=4320).contains(&settings.resolution_height) {
        settings.resolution_height = defaults.resolution_height;
        repaired.push("resolution_height");
    }
    (settings, repaired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "powderburn-settings-{name}-{}.ron",
            std::process::id()
        ))
    }

    #[test]
    fn invalid_fields_are_repaired_individually_and_logged() {
        let path = path("repair");
        let settings = Settings {
            music_volume: f32::NAN,
            text_scale: 999,
            color_palette: "red-only".to_string(),
            resolution_width: 1,
            ..Settings::default()
        };
        let Ok(text) = ron::ser::to_string(&settings) else {
            panic!("fixture must serialize");
        };
        assert!(std::fs::write(&path, text).is_ok());
        let Ok((loaded, repaired)) = load_from(&path) else {
            panic!("settings should load");
        };
        let _ = std::fs::remove_file(path);

        assert_eq!(loaded.music_volume, Settings::default().music_volume);
        assert_eq!(loaded.text_scale, Settings::default().text_scale);
        assert_eq!(loaded.color_palette, "default");
        assert_eq!(loaded.resolution_width, 1920);
        assert!(repaired.contains(&"music_volume"));
        assert!(repaired.contains(&"text_scale"));
        assert!(repaired.contains(&"color_palette"));
        assert!(repaired.contains(&"resolution_width"));
    }

    #[test]
    fn settings_roundtrip_with_all_accessibility_switches() {
        let path = path("roundtrip");
        let settings = Settings {
            text_scale: 200,
            color_palette: "tritanopia".to_string(),
            slow_clock: true,
            subtitles: true,
            camera_shake: false,
            flashing_effects: false,
            screen_fill_effects: false,
            ..Settings::default()
        };
        assert!(save_to(&path, &settings).is_ok());
        let Ok((loaded, repaired)) = load_from(&path) else {
            panic!("settings must load");
        };
        let _ = std::fs::remove_file(path);
        assert_eq!(loaded, settings);
        assert!(repaired.is_empty());
    }

    #[test]
    fn all_three_runtime_palettes_are_distinct_and_high_contrast() {
        let mut settings = Settings::default();
        let mut palettes = Vec::new();
        for name in ["default", "deuteranopia", "tritanopia"] {
            settings.color_palette = name.to_string();
            let palette = settings.palette();
            assert!(contrast_ratio(palette.text, palette.background) >= 4.5);
            palettes.push(palette);
        }
        assert_ne!(palettes[0], palettes[1]);
        assert_ne!(palettes[1], palettes[2]);
        assert_ne!(palettes[0], palettes[2]);
    }

    fn contrast_ratio(foreground: [f32; 4], background: [f32; 4]) -> f32 {
        pb_render::ui_contract::contrast_ratio(foreground, background)
    }
}
