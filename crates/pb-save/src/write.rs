//! Atomic save file writer.
//!
//! Serializes a `SaveFileData` to the binary save format and writes it to
//! disk atomically (write to a temporary file, then rename).

#![forbid(unsafe_code)]

use std::path::Path;
use std::time::Instant;

use crate::error::SaveError;
use crate::format::serialize_save;
use pb_content::schema::SaveFileData;

/// Serialize a save and write it to disk atomically.
///
/// The file is first written to a `.tmp` sibling, then renamed to the target
/// path. This prevents partial writes from producing a corrupt save file.
pub fn write(path: &Path, save: &SaveFileData) -> Result<(), SaveError> {
    let _start = Instant::now();
    let data = serialize_save(save)?;

    // Record save size metric
    let registry = pb_core::metrics::MetricsRegistry::global();
    registry.set_save_size_bytes(data.len() as u64);

    // Write to a temporary file in the same directory, then atomically rename.
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &data)?;
    std::fs::rename(&tmp_path, path)?;

    // Record save write time metric
    let elapsed_ms = _start.elapsed().as_secs_f64() * 1000.0;
    registry.set_save_write_ms(elapsed_ms as u64);

    Ok(())
}
