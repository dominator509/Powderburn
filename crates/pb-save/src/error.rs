//! Save error type with SPEC-006 error codes.

use std::fmt;

/// Save operation errors.
#[derive(Debug, Clone)]
pub enum SaveError {
    /// E-SAVE-VERSION: unsupported save format version.
    Version,
    /// E-SAVE-OVERSIZE: save file or declared section exceeds the maximum.
    Oversize,
    /// E-SAVE-INCOMPAT: ruleset_hash or content_hash does not match.
    Incompat,
    /// E-SAVE-TAMPERED: ledger chain integrity check failed.
    Tampered,
    /// I/O error wrapped with E-SAVE-IO.
    Io(String),
    /// Format/deserialization error.
    Format(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Version => write!(f, "E-SAVE-VERSION"),
            SaveError::Oversize => write!(f, "E-SAVE-OVERSIZE"),
            SaveError::Incompat => write!(f, "E-SAVE-INCOMPAT"),
            SaveError::Tampered => write!(f, "E-SAVE-TAMPERED"),
            SaveError::Io(msg) => write!(f, "E-SAVE-IO: {}", msg),
            SaveError::Format(msg) => write!(f, "E-SAVE-FORMAT: {}", msg),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e.to_string())
    }
}
