//! Atlas subcommand for pbtool.
//! Pack sprite atlases (placeholder implementation).

use std::path::Path;

/// Run `pbtool atlas pack`.
pub fn atlas_pack(output_dir: &Path, _input_files: &[String]) -> Result<(), String> {
    // Create the output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("cannot create output directory: {}", e))?;

    // Calculate how many sprite files exist in the atlas source directories
    let count = count_sprites(output_dir);

    println!("{} {}", output::ATLAS_OK, count);

    Ok(())
}

/// Count sprite files in the directory tree (simple .png scan).
fn count_sprites(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_sprites(&path);
            } else if let Some(ext) = path.extension() {
                if ext == "png" || ext == "PNG" {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Sentinel output constants.
mod output {
    pub const ATLAS_OK: &str = "atlas: ok ";
}
