//! Content loader — stubbed at EP-001. Full implementation at EP-003.
//! See ARCHITECTURE.md for this crate's place in the import law.
#![forbid(unsafe_code)]

/// Maximum allowed content file size in bytes (10 MB).
pub const MAX_CONTENT_SIZE: usize = 10_485_760;
