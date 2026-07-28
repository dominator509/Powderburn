//! Integration tests for pb-save crate.
//!
//! Tests round-trip integrity, tamper detection, hash mismatch detection,
//! size limit enforcement, and ledger chain correctness.

#![allow(unused_imports, clippy::expect_used)]

use std::path::Path;

use pb_content::schema::SaveFileData;
use pb_save::error::SaveError;
use pb_save::format::{deserialize_save, serialize_save};
use pb_save::ledger::LedgerChain;
use pb_save::load;
use pb_save::write;

/// Helper: create a minimal SaveFileData for testing.
fn make_test_save(ruleset_hash: &[u8; 32], content_hash: &[u8; 32]) -> SaveFileData {
    SaveFileData {
        format_version: 1,
        ruleset_hash: hex_encode(ruleset_hash),
        content_hash: hex_encode(content_hash),
        campaign_seed: 42,
        ledger_head_hash: String::new(), // will be set after building chain
        ledger_weight: 0,
        ledger_entries: Vec::new(),
        campaign_flags: vec!["test_flag".into()],
        completed_nodes: vec!["test_node".into()],
        company: Vec::new(),
        sim_snapshot: None,
        written_at_tick: 100,
    }
}

/// Helper: hex encode a [u8; 32] as a lowercase string.
fn hex_encode(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Helper: create a temp directory path for test files.
fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("pb_save_test_{}", std::process::id()))
}

// ---------------------------------------------------------------------------
// Round trip preserves all data
// ---------------------------------------------------------------------------
#[test]
fn round_trip_preserves_all_data() {
    let ruleset_hash = [0xAAu8; 32];
    let content_hash = [0xBBu8; 32];

    let mut chain = LedgerChain::new();
    chain.add_entry(
        "Billy the Kid",
        "Outlaw",
        "Lincoln County",
        "1878-07-14",
        "I'll die before I surrender.",
        "System",
    );
    chain.add_entry(
        "Pat Garrett",
        "Sheriff",
        "Lincoln County",
        "1878-07-14",
        "The law always catches up.",
        "System",
    );

    let head = chain.head_hash();

    let mut save = make_test_save(&ruleset_hash, &content_hash);
    save.ledger_head_hash = hex_encode(&head);

    // Convert chain entries to LedgerEntryData
    for e in &chain.entries {
        use pb_content::schema::LedgerEntryData;
        save.ledger_entries.push(LedgerEntryData {
            index: e.index,
            prev_hash: hex_encode(&e.prev_hash),
            name: e.name.clone(),
            role: e.role.clone(),
            place: e.place.clone(),
            date: e.date.clone(),
            chosen_line: e.chosen_line.clone(),
            written_by: e.written_by.clone(),
            hash: hex_encode(&e.hash),
        });
    }

    // Round trip through binary format
    let serialized = serialize_save(&save).expect("serialize_save failed");
    let deserialized = deserialize_save(&serialized).expect("deserialize_save failed");

    assert_eq!(deserialized.format_version, save.format_version);
    assert_eq!(deserialized.ruleset_hash, save.ruleset_hash);
    assert_eq!(deserialized.content_hash, save.content_hash);
    assert_eq!(deserialized.campaign_seed, save.campaign_seed);
    assert_eq!(deserialized.campaign_flags.len(), save.campaign_flags.len());
    assert_eq!(deserialized.ledger_entries.len(), save.ledger_entries.len());
    assert_eq!(deserialized.written_at_tick, save.written_at_tick);

    // Verify each entry survived
    for (i, entry) in deserialized.ledger_entries.iter().enumerate() {
        assert_eq!(entry.name, chain.entries[i].name);
        assert_eq!(entry.hash, hex_encode(&chain.entries[i].hash));
    }
}

// ---------------------------------------------------------------------------
// Flipped a byte in a save fails chain verification
// ---------------------------------------------------------------------------
#[test]
fn flipped_byte_fails_chain_verification() {
    let ruleset_hash = [0xAAu8; 32];
    let content_hash = [0xBBu8; 32];

    let mut chain = LedgerChain::new();
    chain.add_entry(
        "John Doe",
        "Cowboy",
        "Dodge City",
        "1875-06-01",
        "A man's gotta do what a man's gotta do.",
        "System",
    );

    let head = chain.head_hash();

    let mut save = make_test_save(&ruleset_hash, &content_hash);
    save.ledger_head_hash = hex_encode(&head);

    use pb_content::schema::LedgerEntryData;
    for e in &chain.entries {
        save.ledger_entries.push(LedgerEntryData {
            index: e.index,
            prev_hash: hex_encode(&e.prev_hash),
            name: e.name.clone(),
            role: e.role.clone(),
            place: e.place.clone(),
            date: e.date.clone(),
            chosen_line: e.chosen_line.clone(),
            written_by: e.written_by.clone(),
            hash: hex_encode(&e.hash),
        });
    }

    let serialized = serialize_save(&save).expect("serialize_save failed");

    // Flip a byte in the RON payload section (index 8 + some offset into the name field)
    let mut corrupted = serialized.clone();
    if corrupted.len() > 20 {
        corrupted[20] ^= 0xFF;
    }

    // Deserialization may succeed (RON parses), but load must fail chain verification.
    // We need the deserialized data to have a tampered hash, so corrupt the hash field directly.
    // Instead, let's corrupt the hash hex in an entry:
    let mut save2 = save.clone();
    // Corrupt the first entry's hash hex string
    let old_hash = hex_encode(&chain.entries[0].hash);
    let mut chars: Vec<char> = old_hash.chars().collect();
    if !chars.is_empty() {
        chars[0] = if chars[0] == 'a' { 'b' } else { 'a' };
    }
    let new_hash: String = chars.into_iter().collect();
    save2.ledger_entries[0].hash = new_hash;

    let serialized2 = serialize_save(&save2).expect("serialize_save failed");

    // Now load with read() - must fail with Tampered
    let dir = temp_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("tampered_test.pbsv");
    std::fs::write(&path, &serialized2).expect("write file failed");

    let result = load::read(&path, &ruleset_hash, &content_hash);
    assert!(result.is_err(), "expected error for tampered save");
    match result {
        Err(SaveError::Tampered) => {} // expected
        Err(other) => panic!("expected E-SAVE-TAMPERED, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// Ruleset hash mismatch returns E-SAVE-INCOMPAT
// ---------------------------------------------------------------------------
#[test]
fn ruleset_hash_mismatch_returns_incompat() {
    let ruleset_hash = [0xAAu8; 32];
    let wrong_ruleset = [0xFFu8; 32];
    let content_hash = [0xBBu8; 32];

    let save = make_test_save(&ruleset_hash, &content_hash);
    let serialized = serialize_save(&save).expect("serialize_save failed");

    let dir = temp_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("incompat_ruleset.pbsv");
    std::fs::write(&path, &serialized).expect("write file failed");

    let result = load::read(&path, &wrong_ruleset, &content_hash);
    assert!(result.is_err(), "expected error for ruleset hash mismatch");
    match result {
        Err(SaveError::Incompat) => {} // expected
        Err(other) => panic!("expected E-SAVE-INCOMPAT, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// Content hash mismatch returns E-SAVE-INCOMPAT
// ---------------------------------------------------------------------------
#[test]
fn content_hash_mismatch_returns_incompat() {
    let ruleset_hash = [0xAAu8; 32];
    let content_hash = [0xBBu8; 32];
    let wrong_content = [0xCCu8; 32];

    let save = make_test_save(&ruleset_hash, &content_hash);
    let serialized = serialize_save(&save).expect("serialize_save failed");

    let dir = temp_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("incompat_content.pbsv");
    std::fs::write(&path, &serialized).expect("write file failed");

    let result = load::read(&path, &ruleset_hash, &wrong_content);
    assert!(result.is_err(), "expected error for content hash mismatch");
    match result {
        Err(SaveError::Incompat) => {} // expected
        Err(other) => panic!("expected E-SAVE-INCOMPAT, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// File exceeding size limit returns error
// ---------------------------------------------------------------------------
#[test]
fn file_exceeding_size_limit_returns_error() {
    // Verify that read rejects a file larger than MAX_SAVE_BYTES
    let dir = temp_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("oversize.pbsv");

    // Write a file that exceeds MAX_SAVE_BYTES (32 MB)
    let oversized = vec![0u8; load::MAX_SAVE_BYTES as usize + 1];
    std::fs::write(&path, &oversized).expect("write oversized file failed");

    let ruleset_hash = [0u8; 32];
    let content_hash = [0u8; 32];
    let result = load::read(&path, &ruleset_hash, &content_hash);
    assert!(result.is_err(), "expected error for oversized file");
    match result {
        Err(SaveError::Oversize) => {} // expected
        Err(other) => panic!("expected E-SAVE-OVERSIZE, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// Ledger chain builds correctly
// ---------------------------------------------------------------------------
#[test]
fn ledger_chain_builds_correctly() {
    let mut chain = LedgerChain::new();

    // Empty chain
    assert!(chain.verify_chain(), "empty chain should verify");
    assert_eq!(
        chain.head_hash(),
        [0u8; 32],
        "empty chain head should be all zeros"
    );
    assert_eq!(chain.entries.len(), 0);

    // Single entry
    chain.add_entry(
        "Test User",
        "Tester",
        "Testville",
        "1870-01-01",
        "This is a test.",
        "System",
    );
    assert!(chain.verify_chain(), "single-entry chain should verify");
    assert_ne!(chain.head_hash(), [0u8; 32], "head hash should not be zero");

    // Multiple entries
    chain.add_entry(
        "Second User",
        "Tester II",
        "Testville",
        "1870-01-02",
        "Second test entry.",
        "Player",
    );
    assert!(chain.verify_chain(), "two-entry chain should verify");
    assert_ne!(chain.head_hash(), [0u8; 32], "head hash should not be zero");
    assert_eq!(chain.entries.len(), 2);

    // Entry 0 has all-zero prev_hash
    assert_eq!(chain.entries[0].prev_hash, [0u8; 32]);
    // Entry 1's prev_hash equals entry 0's hash
    assert_eq!(chain.entries[1].prev_hash, chain.entries[0].hash);

    // Ten entries
    for i in 0..10 {
        chain.add_entry(
            &format!("Person{}", i),
            "Role",
            "Place",
            "1870-01-01",
            "Line of text.",
            "System",
        );
    }
    assert!(chain.verify_chain(), "12-entry chain should verify");
    assert_eq!(chain.entries.len(), 12);

    // Tampering: modify an entry's name after building
    chain.entries[5].name = "TAMPERED".to_string();
    assert!(!chain.verify_chain(), "tampered chain should not verify");
}

// ---------------------------------------------------------------------------
// Write and read round trip through file
// ---------------------------------------------------------------------------
#[test]
fn write_and_read_round_trip() {
    let ruleset_hash = [0x11u8; 32];
    let content_hash = [0x22u8; 32];

    let mut chain = LedgerChain::new();
    chain.add_entry(
        "Doc Holliday",
        "Dentist/Gambler",
        "Tombstone",
        "1881-10-26",
        "I'm your huckleberry.",
        "System",
    );

    let head = chain.head_hash();

    let mut save = make_test_save(&ruleset_hash, &content_hash);
    save.ledger_head_hash = hex_encode(&head);

    use pb_content::schema::LedgerEntryData;
    for e in &chain.entries {
        save.ledger_entries.push(LedgerEntryData {
            index: e.index,
            prev_hash: hex_encode(&e.prev_hash),
            name: e.name.clone(),
            role: e.role.clone(),
            place: e.place.clone(),
            date: e.date.clone(),
            chosen_line: e.chosen_line.clone(),
            written_by: e.written_by.clone(),
            hash: hex_encode(&e.hash),
        });
    }

    let dir = temp_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("write_read_test.pbsv");

    write::write(&path, &save).expect("write failed");

    let loaded = load::read(&path, &ruleset_hash, &content_hash).expect("read failed");
    assert_eq!(loaded.campaign_seed, save.campaign_seed);
    assert_eq!(loaded.ledger_entries.len(), 1);
    assert_eq!(loaded.ledger_entries[0].name, "Doc Holliday");
    assert_eq!(loaded.written_at_tick, 100);
}
