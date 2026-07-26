//! Simulation state hashing for determinism verification.

#![forbid(unsafe_code)]

/// Compute a hash of the current simulation state.
pub fn hash_state(_state: &crate::state::SimState) -> [u8; 32] {
    // Stub: returns a zero hash.
    [0u8; 32]
}
