//! Save loader — stubbed at EP-001. Full implementation at EP-003.
//! See ARCHITECTURE.md for this crate's place in the import law.
#![forbid(unsafe_code)]

/// Maximum allowed save file size in bytes (1 MB).
pub const MAX_SAVE_SIZE: usize = 1_048_576;
