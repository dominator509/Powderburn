//! Atomic save file writer.
//!
//! Serializes a `SaveFileData` to the binary save format and writes it to
//! disk atomically (write to a temporary file, then rename).

#![forbid(unsafe_code)]

use std::io::Write;
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
    let start = Instant::now();
    let data = serialize_save(save)?;

    let save_size = match u64::try_from(data.len()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    };
    let registry = pb_core::metrics::MetricsRegistry::global();
    registry.set_save_size_bytes(save_size);

    let tmp_path = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut tmp = options.open(&tmp_path)?;
    tmp.write_all(&data)?;
    tmp.sync_all()?;
    drop(tmp);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp_path, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    let elapsed_ms = match u64::try_from(start.elapsed().as_millis()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    };
    registry.set_save_write_ms(elapsed_ms);

    Ok(())
}
