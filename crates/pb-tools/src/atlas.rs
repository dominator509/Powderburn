//! Deterministic PNG atlas packing for pbtool.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

const MAX_INPUT_FILES: usize = 1_024;
const MAX_ATLAS_DIMENSION: u32 = 16_384;

struct Sprite {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Pack input PNG files into a row-major, transparent RGBA atlas.
///
/// The output is `<output_dir>/atlas.png`. Inputs are sorted by path so the
/// same files always produce the same cell assignment. When no explicit
/// inputs are supplied, PNG files already in `output_dir` are used.
pub fn atlas_pack(output_dir: &Path, input_files: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("cannot create output directory: {error}"))?;
    let mut paths = if input_files.is_empty() {
        png_files(output_dir)?
    } else {
        input_files.iter().map(PathBuf::from).collect()
    };
    paths.sort();
    paths.dedup();
    paths.retain(|path| path.file_name().is_none_or(|name| name != "atlas.png"));
    if paths.len() > MAX_INPUT_FILES {
        return Err(format!(
            "atlas input count {} exceeds limit {MAX_INPUT_FILES}",
            paths.len()
        ));
    }
    if paths.is_empty() {
        println!("{} 0", output::ATLAS_OK);
        return Ok(());
    }

    let sprites = paths
        .iter()
        .map(|path| decode_rgba(path))
        .collect::<Result<Vec<_>, _>>()?;
    let cell_width = sprites.iter().map(|sprite| sprite.width).max().unwrap_or(1);
    let cell_height = sprites
        .iter()
        .map(|sprite| sprite.height)
        .max()
        .unwrap_or(1);
    let columns = integer_ceiling_sqrt(sprites.len()) as u32;
    let rows = (sprites.len() as u32).div_ceil(columns);
    let width = cell_width
        .checked_mul(columns)
        .ok_or_else(|| "atlas width overflow".to_string())?;
    let height = cell_height
        .checked_mul(rows)
        .ok_or_else(|| "atlas height overflow".to_string())?;
    if width > MAX_ATLAS_DIMENSION || height > MAX_ATLAS_DIMENSION {
        return Err(format!(
            "atlas dimensions {width}x{height} exceed {MAX_ATLAS_DIMENSION}"
        ));
    }
    let byte_len = usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(height as usize))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| "atlas byte size overflow".to_string())?;
    let mut atlas = vec![0_u8; byte_len];
    for (index, sprite) in sprites.iter().enumerate() {
        let cell_x = (index as u32 % columns) * cell_width;
        let cell_y = (index as u32 / columns) * cell_height;
        for row in 0..sprite.height {
            let source = row as usize * sprite.width as usize * 4;
            let destination = ((cell_y + row) as usize * width as usize + cell_x as usize) * 4;
            let length = sprite.width as usize * 4;
            atlas[destination..destination + length]
                .copy_from_slice(&sprite.rgba[source..source + length]);
        }
    }
    encode_rgba(&output_dir.join("atlas.png"), width, height, &atlas)?;
    println!("{} {}", output::ATLAS_OK, sprites.len());
    Ok(())
}

fn png_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot scan atlas input: {error}"))?;
    Ok(entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .collect())
}

fn decode_rgba(path: &Path) -> Result<Sprite, String> {
    let file = File::open(path)
        .map_err(|error| format!("cannot open sprite '{}': {error}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("cannot decode sprite '{}': {error}", path.display()))?;
    let mut buffer = vec![0_u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("cannot read sprite '{}': {error}", path.display()))?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(format!(
                "sprite '{}' remained indexed after expansion",
                path.display()
            ));
        }
    };
    Ok(Sprite {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn encode_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = File::create(path)
        .map_err(|error| format!("cannot create atlas '{}': {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("cannot write atlas header: {error}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|error| format!("cannot write atlas pixels: {error}"))
}

fn integer_ceiling_sqrt(value: usize) -> usize {
    let mut root = 1_usize;
    while root.saturating_mul(root) < value {
        root += 1;
    }
    root
}

mod output {
    pub const ATLAS_OK: &str = "atlas: ok";
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn packs_sorted_pngs_into_a_real_rgba_atlas() {
        let root = std::env::temp_dir().join(format!("powderburn-atlas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        let red = root.join("b.png");
        let blue = root.join("a.png");
        encode_rgba(&red, 2, 1, &[255, 0, 0, 255, 255, 0, 0, 255]).expect("red");
        encode_rgba(&blue, 1, 2, &[0, 0, 255, 255, 0, 0, 255, 255]).expect("blue");
        atlas_pack(
            &root,
            &[red.display().to_string(), blue.display().to_string()],
        )
        .expect("pack");
        let atlas = decode_rgba(&root.join("atlas.png")).expect("decode atlas");
        assert_eq!((atlas.width, atlas.height), (4, 2));
        assert_eq!(&atlas.rgba[..4], &[0, 0, 255, 255]);
        assert_eq!(&atlas.rgba[8..12], &[255, 0, 0, 255]);
        let _ = std::fs::remove_dir_all(root);
    }
}
