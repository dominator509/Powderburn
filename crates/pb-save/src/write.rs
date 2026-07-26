//! Atomic save file writer.
//!
//! Serializes a `SaveFileData` to the binary save format and writes it to
//! disk atomically (write to a temporary file, then rename).

#![forbid(unsafe_code)]

use std::path::Path;

use crate::error::SaveError;
use crate::format::serialize_save;
use pb_content::schema::SaveFileData;

/// Serialize a save and write it to disk atomically.
///
/// The file is first written to a `.tmp` sibling, then renamed to the target
/// path. This prevents partial writes from producing a corrupt save file.
pub fn write(path: &Path, save: &SaveFileData) -> Result<(), SaveError> {
    let data = serialize_save(save)?;

    // Write to a temporary file in the same directory, then atomically rename.
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &data)?;
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}
