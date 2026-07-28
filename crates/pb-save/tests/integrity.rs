//! SPEC-005 section 2: append-only Ledger integrity and refusal codes.

#![allow(clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use pb_content::schema::{LedgerEntryData, SaveFileData};
use pb_save::{error::SaveError, ledger::LedgerChain, load, write};

fn hex(bytes: &[u8; 32]) -> String {
    pb_content::hash::hex(bytes)
}

fn test_root(name: &str) -> PathBuf {
    let cache = std::env::var_os("PB_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(".pbcache")
        });
    cache
        .join("test")
        .join(format!("{name}-{}", std::process::id()))
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn entry_data(chain: &LedgerChain) -> Vec<LedgerEntryData> {
    chain
        .entries
        .iter()
        .map(|entry| LedgerEntryData {
            index: entry.index,
            prev_hash: hex(&entry.prev_hash),
            name: entry.name.clone(),
            role: entry.role.clone(),
            place: entry.place.clone(),
            date: entry.date.clone(),
            chosen_line: entry.chosen_line.clone(),
            written_by: entry.written_by.clone(),
            hash: hex(&entry.hash),
        })
        .collect()
}

fn save_data(
    chain: &LedgerChain,
    ruleset_hash: &[u8; 32],
    content_hash: &[u8; 32],
) -> SaveFileData {
    SaveFileData {
        format_version: 1,
        ruleset_hash: hex(ruleset_hash),
        content_hash: hex(content_hash),
        campaign_seed: 1867,
        ledger_head_hash: hex(&chain.head_hash()),
        ledger_weight: 0,
        ledger_entries: entry_data(chain),
        campaign_flags: Vec::new(),
        completed_nodes: Vec::new(),
        company: Vec::new(),
        sim_snapshot: None,
        written_at_tick: 0,
    }
}

#[test]
fn appending_preserves_prior_entries_and_links_new_head() {
    let mut chain = LedgerChain::new();
    chain.add_entry(
        "Ada",
        "Scout",
        "Elk Creek",
        "1867-10-21",
        "We held.",
        "System",
    );
    let first = chain.entries[0].clone();

    chain.add_entry(
        "Ben",
        "Drover",
        "Elk Creek",
        "1867-10-22",
        "Keep moving.",
        "c_ada",
    );

    assert_eq!(chain.entries[0].index, first.index);
    assert_eq!(chain.entries[0].prev_hash, first.prev_hash);
    assert_eq!(chain.entries[0].hash, first.hash);
    assert_eq!(chain.entries[1].index, 1);
    assert_eq!(chain.entries[1].prev_hash, first.hash);
    assert!(chain.verify_chain());
}

#[test]
fn hash_mismatch_and_ledger_tamper_are_refused() {
    let root = test_root("save-integrity");
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(&root).expect("test directory must be created");
    let path = root.join("campaign.pbsv");
    let ruleset_hash = [0x11; 32];
    let content_hash = [0x22; 32];

    let mut chain = LedgerChain::new();
    chain.add_entry(
        "Ada",
        "Scout",
        "Elk Creek",
        "1867-10-21",
        "We held.",
        "System",
    );
    let save = save_data(&chain, &ruleset_hash, &content_hash);
    write::write(&path, &save).expect("valid save must be written");

    assert!(matches!(
        load::read(&path, &[0x33; 32], &content_hash),
        Err(SaveError::Incompat)
    ));

    let mut tampered = save;
    tampered.ledger_entries[0].chosen_line = "History rewritten.".to_string();
    write::write(&path, &tampered).expect("tampered fixture must be written");
    assert!(matches!(
        load::read(&path, &ruleset_hash, &content_hash),
        Err(SaveError::Tampered)
    ));
}

#[test]
fn tampered_ledger_can_only_open_under_explicit_unverified_policy() {
    let root = test_root("save-unverified");
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(&root).expect("test directory must be created");
    let path = root.join("campaign.pbsv");
    let ruleset_hash = [0x11; 32];
    let content_hash = [0x22; 32];

    let mut chain = LedgerChain::new();
    chain.add_entry(
        "Ada",
        "Scout",
        "Elk Creek",
        "1867-10-21",
        "We held.",
        "System",
    );
    let mut save = save_data(&chain, &ruleset_hash, &content_hash);
    save.ledger_entries[0].chosen_line = "Disk-damaged line.".to_string();
    write::write(&path, &save).expect("tampered fixture must be written");

    assert!(matches!(
        load::read_with_policy(
            &path,
            &ruleset_hash,
            &content_hash,
            load::LedgerPolicy::RequireVerified
        ),
        Err(SaveError::Tampered)
    ));
    let loaded = load::read_with_policy(
        &path,
        &ruleset_hash,
        &content_hash,
        load::LedgerPolicy::AllowUnverified,
    )
    .expect("explicit unverified mode should open a well-formed broken chain");
    assert_eq!(
        loaded.ledger_verification,
        load::LedgerVerification::Unverified
    );

    assert!(
        matches!(
            load::read_with_policy(
                &path,
                &[0x33; 32],
                &content_hash,
                load::LedgerPolicy::AllowUnverified
            ),
            Err(SaveError::Incompat)
        ),
        "unverified mode must never bypass content compatibility"
    );
}

#[test]
fn payload_format_version_is_enforced_in_addition_to_container_version() {
    let root = test_root("save-payload-version");
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(&root).expect("test directory must be created");
    let path = root.join("campaign.pbsv");
    let ruleset_hash = [0x11; 32];
    let content_hash = [0x22; 32];
    let mut save = save_data(&LedgerChain::new(), &ruleset_hash, &content_hash);
    save.format_version = 99;
    write::write(&path, &save).expect("future-version fixture must be written");

    assert!(matches!(
        load::read(&path, &ruleset_hash, &content_hash),
        Err(SaveError::Version)
    ));
}
