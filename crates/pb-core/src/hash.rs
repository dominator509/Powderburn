//! Hashing utilities for the simulation kernel.
//!
//! Uses SHA-256 from the `sha2` crate for deterministic, cross-platform
//! hashing suitable for simulation-state hashing (ledger chaining, save
//! integrity, deterministic RNG seeding).

#![forbid(unsafe_code)]

use sha2::Digest;

/// Hash a byte slice into a 32-byte digest using SHA-256.
pub fn hash_state(data: &[u8]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Combine two 32-byte hashes into a single 32-byte hash.
pub fn hash_combine(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(left);
    combined[32..].copy_from_slice(right);
    hash_state(&combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_state_is_deterministic() {
        let data = b"hello world";
        let a = hash_state(data);
        let b = hash_state(data);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_state_matches_expected_sha256() {
        let data = b"hello world";
        let h = hash_state(data);
        let expected_hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let mut actual_hex = String::with_capacity(64);
        for b in &h {
            use std::fmt::Write;
            write!(actual_hex, "{0:02x}", b).unwrap();
        }
        assert_eq!(actual_hex, expected_hex);
    }
}
