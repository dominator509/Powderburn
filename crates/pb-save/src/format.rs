//! Binary save format: magic header + RON-serialized payload.
//!
//! Format: `PBSV` (4 bytes magic) + version u32 LE (4 bytes) + RON-encoded
//! `SaveFileData` bytes.

#![forbid(unsafe_code)]

use pb_content::schema::SaveFileData;

use crate::error::SaveError;

/// Magic bytes identifying a POWDERBURN save file.
const MAGIC: &[u8; 4] = b"PBSV";

/// Current save format version.
const FORMAT_VERSION: u32 = 1;

/// Serialize a `SaveFileData` into the binary save format.
///
/// Returns a `Vec<u8>` containing the magic header, version, and RON-encoded
/// payload.
pub fn serialize_save(save: &SaveFileData) -> Result<Vec<u8>, SaveError> {
    let mut buf = Vec::new();
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());

    let ron_data =
        ron::to_string(save).map_err(|e| SaveError::Format(format!("RON serialization: {}", e)))?;
    buf.extend_from_slice(ron_data.as_bytes());
    Ok(buf)
}

/// Deserialize a `SaveFileData` from the binary save format.
///
/// Validates the magic header and version before parsing the RON payload.
pub fn deserialize_save(data: &[u8]) -> Result<SaveFileData, SaveError> {
    if data.len() < 8 {
        return Err(SaveError::Format("data too short for header".into()));
    }

    if &data[..4] != MAGIC {
        return Err(SaveError::Format("invalid magic bytes".into()));
    }

    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != FORMAT_VERSION {
        return Err(SaveError::Version);
    }

    let ron_str =
        std::str::from_utf8(&data[8..]).map_err(|e| SaveError::Format(format!("UTF-8: {}", e)))?;

    let save: SaveFileData =
        ron::from_str(ron_str).map_err(|e| SaveError::Format(format!("RON deserialize: {}", e)))?;
    Ok(save)
}
