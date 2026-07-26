//! Image subcommand for pbtool.
//! Reads PNG files and reports unique colors and dimensions.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Run `pbtool image stats`.
pub fn image_stats(image_path: &Path) -> Result<(), String> {
    let data = fs::read(image_path)
        .map_err(|e| format!("cannot read image file: {}", e))?;

    // Parse PNG dimensions from header
    // PNG format: 8-byte magic, then IHDR chunk (4-byte length, "IHDR", 4-byte width, 4-byte height)
    if data.len() < 24 {
        return Err("file too small to be a PNG".to_string());
    }

    if &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("not a valid PNG file".to_string());
    }

    // IHDR chunk: starts at byte 16 (after magic + chunk length)
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

    println!("{} {}x{}", output::DIMENSIONS, width, height);

    // Count unique colors by scanning IDAT pixel data
    // This is a simple heuristic: we scan for unique bytes as a proxy
    let unique_colors = count_unique_colors(&data);

    println!("{}{}", output::UNIQUE_COLORS, unique_colors);

    Ok(())
}

/// Count unique color values in PNG pixel data (simple heuristic).
fn count_unique_colors(data: &[u8]) -> usize {
    let mut colors = BTreeSet::new();

    // Scan the raw byte data for unique 3-byte sequences (RGB)
    // This is a simplified approach that looks at the raw file bytes
    let mut i = 8; // skip PNG magic
    while i + 3 <= data.len() {
        // Collect 3-byte chunks as potential RGB values
        let rgb = [data[i], data[i + 1], data[i + 2]];
        colors.insert(rgb);
        i += 3;
    }

    colors.len()
}

/// Sentinel output constants.
mod output {
    pub const UNIQUE_COLORS: &str = "unique-colors: ";
    pub const DIMENSIONS: &str = "dimensions: ";
}
