//! Save/Load functionality for POWDERBURN.
//!
//! Wraps the `pb_save` crate with game-state-aware save/load functions
//! that serialise the current SimState into a `SaveFileData` structure
//! and write it to / read it from `$PB_CONFIG_DIR/saves`.

#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};

use pb_sim::hash::compute_state_hash;
use pb_sim::state::SimState;

use crate::state::GameState;

/// File extension for save files.
const SAVE_EXT: &str = ".pbsv";
#[cfg(test)]
const ZERO_HASH_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn configured_save_dir() -> Result<PathBuf, String> {
    let root = std::env::var_os("PB_CONFIG_DIR")
        .ok_or_else(|| "E-SAVE-PATH: PB_CONFIG_DIR is not set".to_string())?;
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("E-SAVE-PATH: PB_CONFIG_DIR must be absolute".to_string());
    }
    Ok(root.join("saves"))
}

fn validate_slot(slot: &str) -> Result<&str, String> {
    let stem = slot.strip_suffix(SAVE_EXT).unwrap_or(slot);
    let is_simple_name = !stem.is_empty()
        && stem.len() <= 64
        && Path::new(stem)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if !is_simple_name {
        return Err(
            "E-SAVE-PATH: slot must contain only ASCII letters, digits, '-' or '_'".to_string(),
        );
    }
    Ok(stem)
}

fn slot_path_in(save_dir: &Path, slot: &str) -> Result<PathBuf, String> {
    let stem = validate_slot(slot)?;
    Ok(save_dir.join(format!("{stem}{SAVE_EXT}")))
}

fn content_root() -> PathBuf {
    if let Some(home) = std::env::var_os("PB_HOME") {
        let candidate = PathBuf::from(home).join("content");
        if candidate.is_dir() {
            return candidate;
        }
    }

    let working_tree = PathBuf::from("content");
    if working_tree.is_dir() {
        return working_tree;
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("content")
}

fn compatibility_hashes() -> Result<([u8; 32], [u8; 32]), String> {
    let root = content_root();
    let rules = pb_content::hash::ruleset_hash(&root)
        .map_err(|error| format!("E-SAVE-HASH: cannot hash ruleset: {error}"))?;
    let content = pb_content::hash::content_hash(&root)
        .map_err(|error| format!("E-SAVE-HASH: cannot hash content: {error}"))?;
    Ok((rules, content))
}

fn hash_hex(state: &SimState) -> String {
    compute_state_hash(state)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write;
            let _ = write!(output, "{byte:02x}");
            output
        })
}

/// Save the current campaign to a named slot.
///
/// Creates a `SaveFileData` from the current `SimState` and writes it
/// atomically to `$PB_CONFIG_DIR/saves/<slot>.pbsv`.
pub fn save_game(state: &GameState, slot: &str) -> Result<(), String> {
    let save_dir = configured_save_dir()?;
    save_game_in(state, slot, &save_dir)
}

fn save_game_in(state: &GameState, slot: &str, save_dir: &Path) -> Result<(), String> {
    use pb_content::schema::SimSnapshotData;
    use pb_save::write::write;

    let sim = state
        .sim
        .as_ref()
        .ok_or_else(|| "E-SAVE-STATE: no active simulation to save".to_string())?;
    let path = slot_path_in(save_dir, slot)?;
    std::fs::create_dir_all(save_dir)
        .map_err(|e| format!("E-SAVE-IO: failed to create saves directory: {e}"))?;
    let state_ron =
        ron::to_string(sim).map_err(|e| format!("E-SAVE-FORMAT: snapshot serialization: {e}"))?;

    let (ruleset_hash, content_hash) = compatibility_hashes()?;
    let mut save = state
        .campaign
        .clone()
        .ok_or_else(|| "E-SAVE-STATE: no active campaign to save".to_string())?;
    save.ruleset_hash = pb_content::hash::hex(&ruleset_hash);
    save.content_hash = pb_content::hash::hex(&content_hash);
    save.campaign_seed = sim.seed;
    save.sim_snapshot = Some(SimSnapshotData {
        state_ron,
        state_hash: hash_hex(sim),
    });
    save.written_at_tick = sim.tick.0;

    write(&path, &save).map_err(|e| format!("E-SAVE-IO: write failed: {e}"))?;

    println!("save: wrote slot '{slot}' at tick {}", sim.tick.0);
    Ok(())
}

/// Load a campaign from a save file in the named slot.
///
/// The container, ledger chain, and complete simulation snapshot are verified
/// before the live game state is changed.
pub fn load_game(state: &mut GameState, slot: &str) -> Result<(), String> {
    let save_dir = configured_save_dir()?;
    load_game_in_with_policy(
        state,
        slot,
        &save_dir,
        pb_save::load::LedgerPolicy::RequireVerified,
    )
}

/// Explicitly load a save whose Ledger chain failed verification.
///
/// This never bypasses format, size, content, ruleset, or simulation snapshot
/// verification. The runtime remains visibly unverified and cannot select a
/// Ledger-based ending.
pub fn load_game_unverified(state: &mut GameState, slot: &str) -> Result<(), String> {
    let save_dir = configured_save_dir()?;
    load_game_in_with_policy(
        state,
        slot,
        &save_dir,
        pb_save::load::LedgerPolicy::AllowUnverified,
    )
}

#[cfg(test)]
fn load_game_in(state: &mut GameState, slot: &str, save_dir: &Path) -> Result<(), String> {
    load_game_in_with_policy(
        state,
        slot,
        save_dir,
        pb_save::load::LedgerPolicy::RequireVerified,
    )
}

fn load_game_in_with_policy(
    state: &mut GameState,
    slot: &str,
    save_dir: &Path,
    policy: pb_save::load::LedgerPolicy,
) -> Result<(), String> {
    let path = slot_path_in(save_dir, slot)?;
    let (ruleset_hash, content_hash) = compatibility_hashes()?;
    let loaded = pb_save::load::read_with_policy(&path, &ruleset_hash, &content_hash, policy)
        .map_err(|e| format!("E-SAVE-VERIFY: {e}"))?;
    let save = loaded.save;
    let snapshot = save
        .sim_snapshot
        .clone()
        .ok_or_else(|| "E-SAVE-FORMAT: save has no simulation snapshot".to_string())?;
    let sim: SimState = ron::from_str(&snapshot.state_ron)
        .map_err(|e| format!("E-SAVE-FORMAT: invalid simulation snapshot: {e}"))?;
    if hash_hex(&sim) != snapshot.state_hash {
        return Err("E-SAVE-TAMPERED: simulation snapshot hash mismatch".to_string());
    }
    if sim.seed != save.campaign_seed || sim.tick.0 != save.written_at_tick {
        return Err("E-SAVE-TAMPERED: snapshot metadata mismatch".to_string());
    }

    let restored_tick = sim.tick.0;
    state.sim = Some(sim);
    state.campaign = Some(save);
    state.ledger_verification = loaded.ledger_verification;
    state.pending_unverified_slot = None;
    state.current_mission = restore_mission_id(
        state
            .sim
            .as_ref()
            .map_or(0, |simulation| simulation.scenario_id),
    );
    state.tick = restored_tick;
    state.message = match state.ledger_verification {
        pb_save::load::LedgerVerification::Verified => {
            format!("Loaded from slot '{slot}' (tick {restored_tick}) — Ledger verified")
        }
        pb_save::load::LedgerVerification::Unverified => format!(
            "UNVERIFIED SAVE '{slot}' loaded (tick {restored_tick}) — Ledger ending disabled"
        ),
    };
    println!("{}", state.message);
    Ok(())
}

fn restore_mission_id(scenario_hash: u32) -> Option<String> {
    let content = pb_content::load::load_all(&content_root()).ok()?;
    content.campaign_nodes.values().find_map(|node| {
        let scenario_id = node.scenario_id.as_deref()?;
        let hash = pb_core::hash::hash_state(scenario_id.as_bytes());
        let candidate = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
        (candidate == scenario_hash).then(|| node.id.clone())
    })
}

/// List available save slots.
///
/// Returns the stem of each `.pbsv` file found in the saves directory,
/// sorted alphabetically.
pub fn list_saves() -> Vec<String> {
    configured_save_dir().map_or_else(|_| Vec::new(), |save_dir| list_saves_in(&save_dir))
}

fn list_saves_in(save_dir: &Path) -> Vec<String> {
    let dir = match std::fs::read_dir(save_dir) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut slots: Vec<String> = dir
        .filter_map(|entry| entry.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "pbsv")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .collect();

    slots.sort();
    slots
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use pb_core::ids::{ActorId, Tick};

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    fn test_save_dir() -> PathBuf {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("powderburn-save-test-{}-{id}", std::process::id()))
    }

    fn populated_state() -> GameState {
        let mut game = GameState::new();
        let scenario_hash = pb_core::hash::hash_state(b"scn_m01_elk_creek");
        let scenario_id = u32::from_le_bytes([
            scenario_hash[0],
            scenario_hash[1],
            scenario_hash[2],
            scenario_hash[3],
        ]);
        let mut sim = SimState::new(0x5eed, scenario_id);
        sim.tick = Tick(432);
        sim.overwatch.insert(ActorId(19));
        game.tick = sim.tick.0;
        game.sim = Some(sim);
        game.campaign = Some(pb_content::schema::SaveFileData {
            format_version: 1,
            ruleset_hash: String::new(),
            content_hash: String::new(),
            campaign_seed: 0x5eed,
            ledger_head_hash: ZERO_HASH_HEX.to_string(),
            ledger_weight: 0,
            ledger_entries: Vec::new(),
            campaign_flags: vec!["test_flag".to_string()],
            completed_nodes: vec!["m01_elk_creek".to_string()],
            company: Vec::new(),
            sim_snapshot: None,
            written_at_tick: 0,
        });
        game
    }

    #[test]
    fn full_simulation_state_roundtrips() {
        let save_dir = test_save_dir();
        let original = populated_state();
        let expected_hash = original.sim.as_ref().map(hash_hex);
        assert!(save_game_in(&original, "slot_1", &save_dir).is_ok());

        let mut loaded = GameState::new();
        assert!(load_game_in(&mut loaded, "slot_1", &save_dir).is_ok());

        assert_eq!(loaded.tick, 432);
        assert_eq!(
            loaded
                .campaign
                .as_ref()
                .map(|campaign| campaign.campaign_flags.clone()),
            Some(vec!["test_flag".to_string()])
        );
        assert_eq!(
            loaded
                .campaign
                .as_ref()
                .map(|campaign| campaign.completed_nodes.clone()),
            Some(vec!["m01_elk_creek".to_string()])
        );
        assert_eq!(loaded.current_mission.as_deref(), Some("m01_elk_creek"));
        assert_eq!(loaded.sim.as_ref().map(hash_hex), expected_hash);
        assert_eq!(list_saves_in(&save_dir), vec!["slot_1".to_string()]);
        assert!(std::fs::remove_dir_all(save_dir).is_ok());
    }

    #[test]
    fn traversal_and_ambiguous_slot_names_are_rejected() {
        let save_dir = test_save_dir();
        let state = populated_state();
        for invalid in ["", ".", "..", "../escape", "nested/slot", "slot.pbsv.pbsv"] {
            assert!(matches!(
                save_game_in(&state, invalid, &save_dir),
                Err(error) if error.starts_with("E-SAVE-PATH:")
            ));
        }
        assert!(!save_dir.exists());
    }

    fn rewrite_snapshot_hash(path: &Path, replacement: &str) -> Result<(), String> {
        use pb_save::format::{deserialize_save, serialize_save};

        let raw = std::fs::read(path).map_err(|error| error.to_string())?;
        let mut save = deserialize_save(&raw).map_err(|error| error.to_string())?;
        let snapshot = save
            .sim_snapshot
            .as_mut()
            .ok_or_else(|| "test save has no snapshot".to_string())?;
        snapshot.state_hash = replacement.to_string();
        let tampered = serialize_save(&save).map_err(|error| error.to_string())?;
        std::fs::write(path, tampered).map_err(|error| error.to_string())
    }

    fn break_ledger_chain(path: &Path) -> Result<(), String> {
        use pb_save::format::{deserialize_save, serialize_save};

        let raw = std::fs::read(path).map_err(|error| error.to_string())?;
        let mut save = deserialize_save(&raw).map_err(|error| error.to_string())?;
        save.ledger_entries
            .push(pb_content::schema::LedgerEntryData {
                index: 0,
                prev_hash: ZERO_HASH_HEX.to_string(),
                name: "Ada".to_string(),
                role: "Scout".to_string(),
                place: "Elk Creek".to_string(),
                date: "1867-08-12".to_string(),
                chosen_line: "This line no longer matches its hash.".to_string(),
                written_by: "System".to_string(),
                hash: ZERO_HASH_HEX.to_string(),
            });
        let tampered = serialize_save(&save).map_err(|error| error.to_string())?;
        std::fs::write(path, tampered).map_err(|error| error.to_string())
    }

    #[test]
    fn tampered_snapshot_is_refused_without_mutating_live_state() {
        let save_dir = test_save_dir();
        let original = populated_state();
        assert!(save_game_in(&original, "tamper", &save_dir).is_ok());
        let path = save_dir.join("tamper.pbsv");
        assert!(rewrite_snapshot_hash(&path, ZERO_HASH_HEX).is_ok());

        let mut live = populated_state();
        live.tick = 99;
        let before = live.sim.as_ref().map(hash_hex);
        assert!(matches!(
            load_game_in(&mut live, "tamper", &save_dir),
            Err(error) if error == "E-SAVE-TAMPERED: simulation snapshot hash mismatch"
        ));
        assert_eq!(live.tick, 99);
        assert_eq!(live.sim.as_ref().map(hash_hex), before);
        assert!(std::fs::remove_dir_all(save_dir).is_ok());
    }

    #[test]
    fn broken_ledger_requires_explicit_unverified_load_and_remains_labeled() {
        let save_dir = test_save_dir();
        let original = populated_state();
        assert!(save_game_in(&original, "damaged", &save_dir).is_ok());
        let path = save_dir.join("damaged.pbsv");
        assert!(break_ledger_chain(&path).is_ok());

        let mut live = GameState::new();
        assert!(matches!(
            load_game_in(&mut live, "damaged", &save_dir),
            Err(error) if error.contains("E-SAVE-TAMPERED")
        ));
        assert!(
            live.sim.is_none(),
            "strict refusal must not mutate live state"
        );

        assert!(load_game_in_with_policy(
            &mut live,
            "damaged",
            &save_dir,
            pb_save::load::LedgerPolicy::AllowUnverified
        )
        .is_ok());
        assert_eq!(
            live.ledger_verification,
            pb_save::load::LedgerVerification::Unverified
        );
        assert!(live.message.contains("UNVERIFIED SAVE"));
        assert!(live.message.contains("Ledger ending disabled"));
        assert!(std::fs::remove_dir_all(save_dir).is_ok());
    }
}
