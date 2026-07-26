//! Merkle-style hash chain ledger for save integrity.
//!
//! Each entry is cryptographically bound to its predecessor via a hash of the
//! entry index, previous hash, and content fields. Entry 0's `prev_hash` is
//! all zeros. The chain is verified by recomputing every hash starting from
//! entry 0.

#![forbid(unsafe_code)]

use pb_core::hash::hash_state;

/// A single entry in the hash chain.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    /// Sequential index (0-based).
    pub index: u32,
    /// Hash of the previous entry (all zeros for entry 0).
    pub prev_hash: [u8; 32],
    /// Name of the person recorded.
    pub name: String,
    /// Role / rank / profession.
    pub role: String,
    /// Place where the event occurred.
    pub place: String,
    /// Date of the event.
    pub date: String,
    /// The chosen-line epitaph for the Ledger.
    pub chosen_line: String,
    /// Who wrote this entry (companion ID or "System").
    pub written_by: String,
    /// Computed hash: hash_state(index || prev_hash || name || role || place
    /// || date || chosen_line). The `written_by` field is NOT included in the
    /// hash per the Ledger specification.
    pub hash: [u8; 32],
}

/// A hash-chained ledger that guards save integrity.
///
/// Build it with [`new`](LedgerChain::new) and [`add_entry`](LedgerChain::add_entry),
/// then call [`verify_chain`](LedgerChain::verify_chain) to check integrity.
#[derive(Debug, Clone, Default)]
pub struct LedgerChain {
    /// Ordered list of ledger entries.
    pub entries: Vec<LedgerEntry>,
}

impl LedgerChain {
    /// Create an empty ledger chain.
    pub fn new() -> Self {
        LedgerChain {
            entries: Vec::new(),
        }
    }

    /// Append a new entry to the ledger chain.
    ///
    /// The hash is computed automatically using `pb_core::hash::hash_state`.
    /// Entry 0's `prev_hash` is `[0u8; 32]`. Each subsequent entry's
    /// `prev_hash` is the hash of the previous entry.
    pub fn add_entry(
        &mut self,
        name: &str,
        role: &str,
        place: &str,
        date: &str,
        chosen_line: &str,
        written_by: &str,
    ) {
        let index: u32 = self.entries.len() as u32;

        let prev_hash: [u8; 32] = if index == 0 {
            [0u8; 32]
        } else {
            // SAFETY: index > 0 implies at least one entry exists.
            self.entries[index as usize - 1].hash
        };

        let hash = compute_entry_hash(index, &prev_hash, name, role, place, date, chosen_line);

        self.entries.push(LedgerEntry {
            index,
            prev_hash,
            name: name.to_owned(),
            role: role.to_owned(),
            place: place.to_owned(),
            date: date.to_owned(),
            chosen_line: chosen_line.to_owned(),
            written_by: written_by.to_owned(),
            hash,
        });
    }

    /// Verify the integrity of the entire hash chain.
    ///
    /// Recomputes every entry's hash from scratch starting at entry 0 and
    /// returns `false` if any entry's stored hash does not match the
    /// recomputed hash, or if any entry's `prev_hash` does not match its
    /// predecessor's hash (except entry 0, which must have all-zero
    /// `prev_hash`).
    pub fn verify_chain(&self) -> bool {
        let mut expected_prev = [0u8; 32];

        for entry in &self.entries {
            // Entry 0 must have all-zero prev_hash.
            if entry.index == 0 {
                if entry.prev_hash != [0u8; 32] {
                    return false;
                }
            } else if entry.prev_hash != expected_prev {
                // Chain link broken: prev_hash doesn't match predecessor's hash.
                return false;
            }

            let computed = compute_entry_hash(
                entry.index,
                &entry.prev_hash,
                &entry.name,
                &entry.role,
                &entry.place,
                &entry.date,
                &entry.chosen_line,
            );

            if computed != entry.hash {
                return false;
            }

            expected_prev = entry.hash;
        }

        true
    }

    /// Return the hash of the most recent entry, or `[0u8; 32]` if the chain
    /// is empty.
    pub fn head_hash(&self) -> [u8; 32] {
        match self.entries.last() {
            Some(e) => e.hash,
            None => [0u8; 32],
        }
    }
}

/// Compute the hash for a ledger entry.
///
/// Hash input (concatenated):
/// - `index` as u32 little-endian bytes
/// - `prev_hash` as 32 raw bytes
/// - `name`, `role`, `place`, `date`, `chosen_line` as UTF-8 bytes
///
/// `written_by` is excluded from the hash per specification.
fn compute_entry_hash(
    index: u32,
    prev_hash: &[u8; 32],
    name: &str,
    role: &str,
    place: &str,
    date: &str,
    chosen_line: &str,
) -> [u8; 32] {
    let cap = 4 + 32 + name.len() + role.len() + place.len() + date.len() + chosen_line.len();
    let mut buf = Vec::with_capacity(cap);
    buf.extend_from_slice(&index.to_le_bytes());
    buf.extend_from_slice(prev_hash);
    buf.extend_from_slice(name.as_bytes());
    buf.extend_from_slice(role.as_bytes());
    buf.extend_from_slice(place.as_bytes());
    buf.extend_from_slice(date.as_bytes());
    buf.extend_from_slice(chosen_line.as_bytes());
    hash_state(&buf)
}
