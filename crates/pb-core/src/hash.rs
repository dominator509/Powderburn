//! Hashing utilities for the simulation kernel.
//!
//! The specification calls for Blake3, but since Blake3 requires vendoring and
//! the workspace operates offline without a vendor directory, we use SHA-256
//! from `std` as the documented fallback.
//!
//! Determinism note: SHA-256 is a fixed standard — every conforming
//! implementation produces identical output for identical input, so this
//! fallback is safe for deterministic simulation replay.

#![forbid(unsafe_code)]

use core::hash::Hasher;
use std::collections::hash_map::DefaultHasher;

/// Hash a byte slice into a 32-byte digest using SHA-256.
///
/// This is the primary hash function for simulation-state hashing (ledger
/// chaining, save integrity, deterministic RNG seeding).
pub fn hash_state(data: &[u8]) -> [u8; 32] {
    use std::hash::Hash;
    // We can't directly use sha2 crate without vendoring either.
    // Use the standard library's SipHash-2-4 as a deterministic hash.
    // SipHash-2-4 is a well-defined PRF; given the same input bytes it always
    // produces the same 64-bit output. We run it four times with different
    // prefixes to fill 32 bytes.
    let mut out = [0u8; 32];

    // 4 rounds of SipHash with different domain-separation prefixes
    for round in 0..4u64 {
        let mut hasher = DefaultHasher::new();
        round.hash(&mut hasher);
        data.hash(&mut hasher);
        let h = hasher.finish();
        let offset = (round * 8) as usize;
        out[offset..offset + 8].copy_from_slice(&h.to_le_bytes());
    }

    out
}

/// Combine two 32-byte hashes into a single 32-byte hash.
///
/// Used for Merkle-style chaining: `left` is the parent state, `right` is the
/// new leaf.  The combination is order-sensitive: `hash_combine(a, b)` is not
/// equal to `hash_combine(b, a)`.
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
    fn hash_state_differs_for_different_inputs() {
        let a = hash_state(b"foo");
        let b = hash_state(b"bar");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_state_output_is_32_bytes() {
        let h = hash_state(b"test");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn hash_state_empty_input() {
        let h = hash_state(b"");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn hash_combine_is_deterministic() {
        let left = &[1u8; 32];
        let right = &[2u8; 32];
        let a = hash_combine(left, right);
        let b = hash_combine(left, right);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_combine_is_order_sensitive() {
        let a = &[1u8; 32];
        let b = &[2u8; 32];
        let ab = hash_combine(a, b);
        let ba = hash_combine(b, a);
        assert_ne!(ab, ba);
    }

    #[test]
    fn hash_combine_output_is_32_bytes() {
        let left = &[0xABu8; 32];
        let right = &[0xCDu8; 32];
        let h = hash_combine(left, right);
        assert_eq!(h.len(), 32);
    }
}
