//! PNG image inspection for pbtool.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

const MAX_IMAGE_BYTES: u64 = 256 * 1024 * 1024;

/// Decode a PNG and report its real dimensions and unique decoded colors.
pub fn image_stats(image_path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(image_path)
        .map_err(|error| format!("cannot stat image file: {error}"))?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(format!("image exceeds {MAX_IMAGE_BYTES} byte limit"));
    }
    let file =
        File::open(image_path).map_err(|error| format!("cannot read image file: {error}"))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("not a valid PNG file: {error}"))?;
    let mut data = vec![0_u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut data)
        .map_err(|error| format!("cannot decode PNG pixels: {error}"))?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err("indexed PNG remained indexed after expansion".to_string());
        }
    };
    let unique_colors = data[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|pixel| pixel.to_vec())
        .collect::<BTreeSet<_>>()
        .len();
    println!("{} {}x{}", output::DIMENSIONS, info.width, info.height);
    println!("{}{}", output::UNIQUE_COLORS, unique_colors);
    Ok(())
}

mod output {
    pub const UNIQUE_COLORS: &str = "unique-colors: ";
    pub const DIMENSIONS: &str = "dimensions:";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_png_input() {
        let path = std::env::temp_dir().join(format!("powderburn-not-png-{}", std::process::id()));
        assert!(std::fs::write(&path, b"not png").is_ok());
        let error = image_stats(&path);
        assert!(error.is_err());
        let _ = std::fs::remove_file(path);
    }
}
