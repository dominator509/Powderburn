//! Save/Load functionality for POWDERBURN.
//!
//! Wraps the `pb_save` crate with game-state-aware save/load functions
//! that serialise the current SimState into a `SaveFileData` structure
//! and write it to / read it from the saves directory.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use pb_sim::state::SimState;

use crate::state::GameState;

/// Directory where save files are stored.
const SAVE_DIR: &str = "/root/powderburn/saves";
/// File extension for save files.
const SAVE_EXT: &str = ".pbsv";

/// Ensure the save directory exists.
fn ensure_save_dir() -> Result<(), String> {
    let dir = Path::new(SAVE_DIR);
    std::fs::create_dir_all(dir).map_err(|e| format!("failed to create saves dir: {e}"))
}

/// Build the full path for a save slot.
fn slot_path(slot: &str) -> PathBuf {
    let name = if slot.ends_with(SAVE_EXT) {
        slot.to_string()
    } else {
        format!("{slot}{SAVE_EXT}")
    };
    Path::new(SAVE_DIR).join(name)
}

/// Save the current campaign to a named slot.
///
/// Creates a `SaveFileData` from the current `SimState` and writes it
/// atomically to `<SAVE_DIR>/<slot>.pbsv`.
pub fn save_game(state: &GameState, slot: &str) -> Result<(), String> {
    use pb_content::schema::SaveFileData;
    use pb_save::format::serialize_save;
    use pb_save::write::write;

    ensure_save_dir()?;

    let seed = state.sim.as_ref().map(|s| s.seed).unwrap_or(42);

    let save = SaveFileData {
        format_version: 1,
        ruleset_hash: String::new(),
        content_hash: String::new(),
        campaign_seed: seed,
        ledger_head_hash: String::new(),
        campaign_flags: vec![],
        company: vec![],
        ledger_entries: vec![],
        sim_snapshot: None,
        written_at_tick: state.tick,
    };

    // Serialize first to validate
    let data = serialize_save(&save).map_err(|e| format!("serialize failed: {e}"))?;

    let path = slot_path(slot);
    write(&path, &save).map_err(|e| format!("write failed: {e}"))?;

    println!("save: wrote {} bytes to {}", data.len(), path.display());
    Ok(())
}

/// Load a campaign from a save file in the named slot.
///
/// Reads and deserialises the save file, then creates a minimal `SimState`
/// from the stored data.  Full state reconstruction (actors, sequence clock)
/// is left for when `sim_snapshot` is populated; currently resets to an
/// empty SimState so the player can continue.
pub fn load_game(state: &mut GameState, slot: &str) -> Result<(), String> {
    use pb_save::format::deserialize_save;

    let path = slot_path(slot);
    let raw = std::fs::read(&path).map_err(|e| format!("failed to read save file: {e}"))?;

    let save = deserialize_save(&raw).map_err(|e| format!("deserialize failed: {e}"))?;

    // Build a fresh SimState from the save metadata
    let mut sim = SimState::new(save.campaign_seed, 1);
    sim.tick.0 = save.written_at_tick;

    state.sim = Some(sim);
    state.tick = save.written_at_tick;
    state.message = format!(
        "Loaded from slot '{}' (tick {})",
        slot, save.written_at_tick
    );
    println!(
        "load: restored from {} (seed={}, tick={})",
        path.display(),
        save.campaign_seed,
        save.written_at_tick
    );
    Ok(())
}

/// List available save slots.
///
/// Returns the stem of each `.pbsv` file found in the saves directory,
/// sorted alphabetically.
pub fn list_saves() -> Vec<String> {
    let dir = match std::fs::read_dir(Path::new(SAVE_DIR)) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut slots: Vec<String> = dir
        .filter_map(|entry| entry.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "pbsv").unwrap_or(false))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .collect();

    slots.sort();
    slots
}
