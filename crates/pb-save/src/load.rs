//! Save file loader with integrity verification.
//!
//! Reads a binary save file, deserializes it, and verifies:
//! - File size limits
//! - Ruleset hash and content hash match expectations
//! - Ledger chain integrity

#![forbid(unsafe_code)]

use std::path::Path;

use pb_content::schema::{LedgerEntryData, SaveFileData};

use crate::error::SaveError;
use crate::format::deserialize_save;
use crate::ledger::{LedgerChain, LedgerEntry};

/// Maximum allowed save file size in bytes (32 MB).
pub const MAX_SAVE_BYTES: u64 = 32 * 1024 * 1024;

/// Maximum allowed number of ledger entries.
pub const MAX_LEDGER_ENTRIES: usize = 4096;

/// Read and verify a save file from disk.
///
/// # Errors
///
/// - `E-SAVE-OVERSIZE` if the file on disk exceeds [`MAX_SAVE_BYTES`] or if
///   a declared section exceeds its limit.
/// - `E-SAVE-INCOMPAT` if `ruleset_hash` or `content_hash` does not match the
///   hashes stored in the save file.
/// - `E-SAVE-TAMPERED` if the ledger chain integrity check fails (any
///   recomputed hash does not match the stored hash, or a chain link is
///   broken).
/// - `E-SAVE-VERSION` if the save format version is not supported.
/// - `E-SAVE-FORMAT` if the data cannot be deserialized.
/// - `E-SAVE-IO` for I/O errors.
pub fn read(
    path: &Path,
    ruleset_hash: &[u8; 32],
    content_hash: &[u8; 32],
) -> Result<SaveFileData, SaveError> {
    let raw = std::fs::read(path)?;

    if raw.len() > MAX_SAVE_BYTES as usize {
        return Err(SaveError::Oversize);
    }

    let save: SaveFileData = deserialize_save(&raw)?;

    // Verify ruleset_hash
    let expected_ruleset = hex_to_bytes(&save.ruleset_hash)?;
    if expected_ruleset != *ruleset_hash {
        return Err(SaveError::Incompat);
    }

    // Verify content_hash
    let expected_content = hex_to_bytes(&save.content_hash)?;
    if expected_content != *content_hash {
        return Err(SaveError::Incompat);
    }

    // Rebuild the LedgerChain from stored entries and verify integrity.
    let chain = build_chain_from_data(&save.ledger_entries)?;
    if !chain.verify_chain() {
        return Err(SaveError::Tampered);
    }

    // Cross-check: the ledger_head_hash in the save must match the chain head.
    let stored_head = hex_to_bytes(&save.ledger_head_hash)?;
    let computed_head = chain.head_hash();
    if stored_head != computed_head {
        return Err(SaveError::Tampered);
    }

    Ok(save)
}

/// Build a [`LedgerChain`] from the serializable [`LedgerEntryData`] records
/// stored in a save file.
fn build_chain_from_data(entries: &[LedgerEntryData]) -> Result<LedgerChain, SaveError> {
    if entries.len() > MAX_LEDGER_ENTRIES {
        return Err(SaveError::Oversize);
    }

    let mut chain = LedgerChain::new();
    for e in entries {
        let prev_hash: [u8; 32] = hex_to_bytes(&e.prev_hash)?;
        let hash: [u8; 32] = hex_to_bytes(&e.hash)?;
        chain.entries.push(LedgerEntry {
            index: e.index,
            prev_hash,
            name: e.name.clone(),
            role: e.role.clone(),
            place: e.place.clone(),
            date: e.date.clone(),
            chosen_line: e.chosen_line.clone(),
            written_by: e.written_by.clone(),
            hash,
        });
    }
    Ok(chain)
}

/// Convert a 64-character hex string to a 32-byte array.
fn hex_to_bytes(s: &str) -> Result<[u8; 32], SaveError> {
    if s.len() != 64 {
        return Err(SaveError::Format(format!(
            "hex string length {} != 64",
            s.len()
        )));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let byte_str = &s[i * 2..i * 2 + 2];
        out[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| SaveError::Format(format!("invalid hex at position {}", i * 2)))?;
    }
    Ok(out)
}

/// Convert a 32-byte array to a 64-character lowercase hex string.
#[allow(dead_code)]
pub(crate) fn bytes_to_hex(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
