//! Property tests for save/load round-trip and ledger chain integrity.
//!
//! Verifies:
//! - LBI-08: save round-trip preserves state hash (via SaveFileData)
//! - LBI-13: Ledger chain verifies successfully and fails on mutation
//!
//! Tests construct synthetic `SaveFileData` instances and `LedgerChain`
//! entries, round-trip them through serialization/deserialization, and
//! verify chain integrity.

#![allow(clippy::expect_used)]

use pb_content::schema::{LedgerEntryData, SaveFileData};
use pb_save::format::{deserialize_save, serialize_save};
use pb_save::ledger::LedgerChain;

// ---------------------------------------------------------------------------
// Simple LCG for generating test data.
// ---------------------------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    #[allow(dead_code)]
    fn next_str(&mut self, base: &str, _n: u32) -> String {
        format!("{}_{}", base, self.next_u64() % 10000)
    }
}

/// Generate a hex string from a 32-byte hash for use in SaveFileData.
fn bytes_to_hex(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Parse a 64-char hex string into a 32-byte array.
fn hex_to_bytes(s: &str) -> [u8; 32] {
    assert_eq!(s.len(), 64, "hex string must be 64 chars");
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("valid hex");
    }
    out
}

/// Build a SaveFileData with a valid ledger chain for testing.
fn make_save(chain: &LedgerChain) -> SaveFileData {
    let ledger_entries: Vec<LedgerEntryData> = chain
        .entries
        .iter()
        .map(|e| LedgerEntryData {
            index: e.index,
            prev_hash: bytes_to_hex(&e.prev_hash),
            name: e.name.clone(),
            role: e.role.clone(),
            place: e.place.clone(),
            date: e.date.clone(),
            chosen_line: e.chosen_line.clone(),
            written_by: e.written_by.clone(),
            hash: bytes_to_hex(&e.hash),
        })
        .collect();

    let head_hash = bytes_to_hex(&chain.head_hash());

    SaveFileData {
        format_version: 1,
        ruleset_hash: "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".into(),
        content_hash: "f0e1d2c3b4a5968778695a4b3c2d1e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6".into(),
        campaign_seed: 42,
        ledger_head_hash: head_hash,
        ledger_weight: 0,
        ledger_entries,
        campaign_flags: vec!["a1_treaty_witnessed".into()],
        completed_nodes: vec!["m01_elk_creek".into()],
        company: vec![],
        sim_snapshot: None,
        written_at_tick: 12345,
    }
}

/// Create a ledger chain with `count` entries.
fn build_chain(_rng: &mut Lcg, count: u32) -> LedgerChain {
    let mut chain = LedgerChain::new();

    let names = [
        "Elias Ward",
        "Naomi Freed",
        "Tsayd-tainte",
        "Sgt. Absalom Doyle",
        "Ignacio Ruelas",
        "Wen Li-hsiang",
        "Cordelia Ames",
        "Hollis Mercer",
        "Tomas Alcantara",
    ];

    let roles = [
        "Scout",
        "Freedmen",
        "Kiowa warrior",
        "Buffalo Soldier",
        "Tejano drover",
        "Railway laborer",
        "Pinkerton agent",
        "Confederate veteran",
        "Field surgeon",
    ];

    let places = [
        "Medicine Lodge, Kansas",
        "Adobe Walls, Texas",
        "Pawnee Fork, Kansas",
        "Promontory Summit, Utah",
        "Nicodemus, Kansas",
        "El Paso, Texas",
        "Los Angeles, California",
        "Fort Marion, Florida",
        "Memphis, Tennessee",
    ];

    let dates = [
        "1867-10-21",
        "1874-06-27",
        "1867-04-19",
        "1869-05-10",
        "1877-08-01",
        "1877-09-15",
        "1871-10-24",
        "1875-05-21",
        "1878-08-01",
    ];

    for i in 0..count {
        let idx = (i as usize) % names.len();
        chain.add_entry(
            names[idx],
            roles[idx],
            places[idx],
            dates[idx],
            &format!("Entry {} — the ledger records the dead", i),
            if i % 2 == 0 { "System" } else { "elias" },
        );
    }

    chain
}

// ===========================================================================
// Tests
// ===========================================================================

/// LBI-08: Save round-trip preserves state hash.
///
/// We serialize a SaveFileData to bytes, deserialize it back, and verify
/// that the ledger_head_hash (which encodes the state) is identical.
#[test]
fn save_round_trip_preserves_ledger_head_hash() {
    for seed in 0..200 {
        let mut rng = Lcg::new(seed);

        // Build a chain with 1-10 entries
        let entry_count = (rng.next_u64() % 10) as u32 + 1;
        let chain = build_chain(&mut rng, entry_count);

        // Verify the chain is valid before round-tripping
        assert!(
            chain.verify_chain(),
            "seed {}: chain should be valid before round-trip",
            seed
        );

        let head_before = chain.head_hash();
        let save = make_save(&chain);

        // Serialize
        let serialized = serialize_save(&save).expect("serialize_save should succeed");

        // Deserialize
        let deserialized = deserialize_save(&serialized)
            .expect("deserialize_save should succeed on round-tripped data");

        // Compare head hashes
        let head_after = hex_to_bytes(&deserialized.ledger_head_hash);
        assert_eq!(
            head_before, head_after,
            "seed {}: ledger head hash changed after round-trip ({} entries)",
            seed, entry_count
        );
    }
}

/// LBI-13: Ledger chain verifies successfully and fails on mutation.
///
/// We construct a valid chain, verify it, then mutate various fields and
/// confirm that verification fails.
#[test]
fn ledger_chain_verifies_and_fails_on_mutation() {
    for seed in 0..200 {
        let mut rng = Lcg::new(seed);
        let entry_count = (rng.next_u64() % 8) as u32 + 2; // At least 2 entries
        let mut chain = build_chain(&mut rng, entry_count);

        // 1. Chain must verify as-is.
        assert!(
            chain.verify_chain(),
            "seed {}: unmodified chain should verify",
            seed
        );

        // 2. Mutate a name in the last entry — verification must fail.
        if let Some(last) = chain.entries.last_mut() {
            last.name = "MUTATED".to_string();
            assert!(
                !chain.verify_chain(),
                "seed {}: chain should fail after name mutation",
                seed
            );
        }

        // 3. Rebuild and mutate prev_hash of a non-first entry.
        let mut chain2 = build_chain(&mut Lcg::new(seed), entry_count);
        if chain2.entries.len() > 1 {
            chain2.entries[1].prev_hash = [0xFFu8; 32];
            assert!(
                !chain2.verify_chain(),
                "seed {}: chain should fail after prev_hash mutation",
                seed
            );
        }

        // 4. Rebuild and mutate the hash field directly.
        let mut chain3 = build_chain(&mut Lcg::new(seed), entry_count);
        if let Some(entry) = chain3.entries.first_mut() {
            entry.hash = [0xAAu8; 32];
            assert!(
                !chain3.verify_chain(),
                "seed {}: chain should fail after hash mutation on entry 0",
                seed
            );
        }

        // 5. Rebuild and set entry 0's prev_hash to non-zero (must fail).
        let mut chain4 = build_chain(&mut Lcg::new(seed), entry_count);
        chain4.entries[0].prev_hash = [0x01u8; 32];
        assert!(
            !chain4.verify_chain(),
            "seed {}: chain should fail when entry 0 has non-zero prev_hash",
            seed
        );
    }
}

/// Serialization of an empty chain is valid.
#[test]
fn empty_chain_round_trip() {
    let chain = LedgerChain::new();
    assert!(chain.verify_chain(), "empty chain verifies");
    assert_eq!(
        chain.head_hash(),
        [0u8; 32],
        "empty chain head is all zeros"
    );

    let save = make_save(&chain);
    let serialized = serialize_save(&save).expect("serialize empty save");
    let deserialized = deserialize_save(&serialized).expect("deserialize empty save");
    assert_eq!(
        deserialized.ledger_entries.len(),
        0,
        "empty round-trip has zero entries"
    );
}

/// Serialization round-trip preserves every field of every ledger entry.
#[test]
fn ledger_entry_fields_preserved_round_trip() {
    let mut rng = Lcg::new(42);
    let chain = build_chain(&mut rng, 5);
    let save = make_save(&chain);

    let serialized = serialize_save(&save).expect("serialize");
    let deserialized = deserialize_save(&serialized).expect("deserialize");

    assert_eq!(
        save.ledger_entries.len(),
        deserialized.ledger_entries.len(),
        "entry count preserved"
    );

    for (i, (orig, round)) in save
        .ledger_entries
        .iter()
        .zip(deserialized.ledger_entries.iter())
        .enumerate()
    {
        assert_eq!(orig.index, round.index, "entry {}: index", i);
        assert_eq!(orig.prev_hash, round.prev_hash, "entry {}: prev_hash", i);
        assert_eq!(orig.name, round.name, "entry {}: name", i);
        assert_eq!(orig.role, round.role, "entry {}: role", i);
        assert_eq!(orig.place, round.place, "entry {}: place", i);
        assert_eq!(orig.date, round.date, "entry {}: date", i);
        assert_eq!(
            orig.chosen_line, round.chosen_line,
            "entry {}: chosen_line",
            i
        );
        assert_eq!(orig.written_by, round.written_by, "entry {}: written_by", i);
        assert_eq!(orig.hash, round.hash, "entry {}: hash", i);
    }
}

/// Fields outside the hash (written_by) can change without breaking chain.
#[test]
fn ledger_chain_ignores_written_by_in_hash() {
    let mut chain = LedgerChain::new();
    chain.add_entry("Test", "role", "place", "date", "line", "author1");

    // Change written_by (not in hash input)
    if let Some(entry) = chain.entries.last_mut() {
        entry.written_by = "author2".to_string();
    }

    // Chain should still verify
    assert!(
        chain.verify_chain(),
        "changing written_by should not break chain integrity"
    );
}
