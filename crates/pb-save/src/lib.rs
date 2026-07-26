//! POWDERBURN save system: binary format, Ledger chain, and integrity
//! verification.
//!
//! This crate provides:
//! - **format**: Binary serialization (`PBSV` magic + version + RON payload).
//! - **ledger**: Hash-chained ledger for save integrity.
//! - **load**: Verified read from disk with hash and chain checks.
//! - **write**: Atomic write to disk.

#![forbid(unsafe_code)]

pub mod error;
pub mod format;
pub mod ledger;
pub mod load;
pub mod write;
