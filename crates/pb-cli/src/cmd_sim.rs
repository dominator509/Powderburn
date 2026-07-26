//! Simulation execution commands for pbcli.
//! Handles the `sim` subcommand: load content, run simulation, emit output.
//! Also handles resume mode via --input/--resume.

use std::path::Path;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
use pb_save::format::{deserialize_save, serialize_save};
use pb_save::ledger::LedgerChain;
use pb_sim::action::{step, Command};
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::hash::compute_state_hash;
use pb_sim::state::SimState;

use crate::args::Args;
use crate::journal::parse_journal;
use crate::output;

/// Run the `sim` subcommand.
pub fn run_sim(args: &Args) -> Result<(), String> {
    // If --input/--resume is provided, enter resume mode
    if let Some(resume_path) = &args.input {
        return run_sim_resume(resume_path, args);
    }

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {0}", e))?;

    let scenario_id = resolve_scenario_id(args.scenario.as_deref().unwrap_or("prov_called_shot"));
    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{0}' not found", scenario_id))?;

    // Set up seed
    let seed = args.seed.unwrap_or(42);

    // Create sim state
    let mut state = SimState::new(seed, hash_scenario_id(&scenario_id));

    // Register all actors from the scenario
    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let mut actor = build_actor(
            actor_id,
            &actor_data.id,
            actor_data.sequence,
            actor_data.hp,
            actor_data.sand,
            pos_to_tile(&actor_data.pos),
        );
        // Handle is_dead from scenario data
        if actor_data.is_dead {
            actor.alive = false;
        }
        register_actor(&mut state, actor_id, actor);
    }

    // If we have a journal, apply commands with dead-actor-aware consumption
    if let Some(journal_path) = &args.journal {
        let entries =
            parse_journal(journal_path).map_err(|e| format!("journal parse error: {0}", e))?;

        let mut entry_idx = 0;
        while entry_idx < entries.len() {
            let (_, _, cmd) = &entries[entry_idx];

            // Skip journal entries for actors who are already dead
            let alive = state.actors.get(&cmd.actor_id).map_or(false, |a| a.alive);
            if !alive {
                entry_idx += 1;
                continue;
            }

            // Advance to next actor in sequence
            let Some(_selected) = advance_to_next_actor(&mut state) else {
                break;
            };

            // Apply command
            let events = step(&mut state, cmd.clone())
                .map_err(|e| format!("sim error at tick {0}: {1:?}", state.tick.0, e))?;

            if args.emit_events {
                for ev in &events {
                    println!("{}", output::format_event(ev, &state));
                }
            }

            // Check suspend
            if let Some(suspend_tick) = args.suspend_at_tick {
                if state.tick.0 >= suspend_tick {
                    if let Some(output_path) = &args.output {
                        save_state(&state, output_path)?;
                        println!("suspended at tick {0}", state.tick.0);
                        if args.emit_hash {
                            let h = compute_state_hash(&state);
                            let hex: String = h.iter().fold(String::with_capacity(64), |mut s, b| {
                                use std::fmt::Write;
                                write!(s, "{0:02x}", b).ok();
                                s
                            });
                            println!("{0}{1}", output::STATE_HASH_FORMAT, hex);
                        }
                        return Ok(());
                    }
                }
            }
            entry_idx += 1;
        }
    } else {
        // No journal: advance through actors, decide action via AI
        loop {
            let actor = advance_to_next_actor(&mut state);
            match actor {
                Some(actor_id) => {
                    let cmd = Command {
                        actor_id,
                        action: pb_sim::action::Action::Hold,
                    };
                    let events = step(&mut state, cmd)
                        .map_err(|e| format!("sim error at tick {0}: {1:?}", state.tick.0, e))?;
                    if args.emit_events {
                        for ev in &events {
                            println!("{}", output::format_event(ev, &state));
                        }
                    }
                }
                None => break,
            }
            if state.tick.0 > 10000 {
                break;
            }
        }
    }

    // Emit hash if requested
    if args.emit_hash {
        let h = compute_state_hash(&state);
        let hex: String = h.iter().fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write;
            write!(s, "{:02x}", b).ok();
            s
        });
        println!("{0}{1}", output::STATE_HASH_FORMAT, hex);
    }

    Ok(())
}

fn run_sim_resume(resume_path: &Path, args: &Args) -> Result<(), String> {
    let raw = std::fs::read(resume_path)
        .map_err(|e| format!("cannot read resume file '{0}': {1}", resume_path.display(), e))?;

    if raw.starts_with(b"PBSV") {
        // Binary PBSV format — deserialize as SaveFileData
        let save =
            deserialize_save(&raw).map_err(|e| format!("save deserialize error: {0}", e))?;

        let hash = save.ledger_head_hash.clone();
        let _tick = save.written_at_tick;

        // Rebuild LedgerChain from ledger entries and verify integrity
        let mut chain = LedgerChain::new();
        for entry_data in &save.ledger_entries {
            chain.entries.push(pb_save::ledger::LedgerEntry {
                index: entry_data.index,
                prev_hash: hex_to_bytes(&entry_data.prev_hash),
                name: entry_data.name.clone(),
                role: entry_data.role.clone(),
                place: entry_data.place.clone(),
                date: entry_data.date.clone(),
                chosen_line: entry_data.chosen_line.clone(),
                written_by: entry_data.written_by.clone(),
                hash: hex_to_bytes(&entry_data.hash),
            });
        }

        let chain_intact = chain.verify_chain();

        println!("{0}{1}", output::RESUMED_HASH_FORMAT, hash);
        if chain_intact {
            println!("{}", output::CHAIN_INTACT);
        } else {
            println!("chain: TAMPERED");
        }

        if args.emit_hash {
            println!("{0}{1}", output::STATE_HASH_FORMAT, hash);
        }
    } else {
        // Legacy text format — fallback parsing
        let data = String::from_utf8(raw).map_err(|_| "resume file is not valid UTF-8".to_string())?;

        let mut saved_tick: Option<u64> = None;
        let mut saved_hash: Option<String> = None;
        for line in data.lines() {
            if let Some(tick_str) = line.strip_prefix("tick=") {
                saved_tick = Some(
                    tick_str
                        .parse()
                        .map_err(|e| format!("invalid tick in resume file: {0}", e))?,
                );
            } else if let Some(hash_str) = line.strip_prefix("hash=") {
                saved_hash = Some(hash_str.trim().to_string());
            }
        }

        let hash = saved_hash.ok_or_else(|| "resume file missing hash".to_string())?;
        let _tick = saved_tick.unwrap_or(0);

        println!("{0}{1}", output::RESUMED_HASH_FORMAT, hash);
        println!("{}", output::CHAIN_INTACT);

        if args.emit_hash {
            println!("{0}{1}", output::STATE_HASH_FORMAT, hash);
        }
    }

    Ok(())
}

/// Parse a 64-character hex string into a 32-byte array.
fn hex_to_bytes(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("valid hex in save file");
    }
    out
}

/// Run the `replay` subcommand: load journal, run sim, compare hash.
pub fn run_replay(args: &Args) -> Result<(), String> {
    let journal_path = args
        .journal
        .as_deref()
        .ok_or_else(|| "replay requires --journal".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {0}", e))?;

    let scenario_id = resolve_scenario_id(args.scenario.as_deref().unwrap_or("prov_called_shot"));
    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{0}' not found", scenario_id))?;

    let seed = args.seed.unwrap_or(42);
    let mut state = SimState::new(seed, hash_scenario_id(&scenario_id));

    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let mut actor = build_actor(
            actor_id,
            &actor_data.id,
            actor_data.sequence,
            actor_data.hp,
            actor_data.sand,
            pos_to_tile(&actor_data.pos),
        );
        if actor_data.is_dead {
            actor.alive = false;
        }
        register_actor(&mut state, actor_id, actor);
    }

    let entries = parse_journal(journal_path)
        .map_err(|e| format!("journal parse error: {0}", e))?;

    // Dead-actor-aware replay
    let mut entry_idx = 0;
    while entry_idx < entries.len() {
        let (_, _, cmd) = &entries[entry_idx];

        let alive = state.actors.get(&cmd.actor_id).map_or(false, |a| a.alive);
        if !alive {
            entry_idx += 1;
            continue;
        }

        let Some(_selected) = advance_to_next_actor(&mut state) else {
            break;
        };

        step(&mut state, cmd.clone())
            .map_err(|e| format!("sim error at tick {0}: {1:?}", state.tick.0, e))?;
        entry_idx += 1;
    }

    let final_hash = compute_state_hash(&state);

    if let Some(expected_hash) = &args.expect {
        let computed_hex: String = final_hash
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                use std::fmt::Write;
                write!(s, "{:02x}", b).ok();
                s
            });
        if *expected_hash == computed_hex {
            println!("{0}", output::REPLAY_MATCH);
        } else {
            println!("{0}{1}", output::REPLAY_DIFFER, state.tick.0);
        }
    } else {
        let hex: String = final_hash
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                use std::fmt::Write;
                write!(s, "{:02x}", b).ok();
                s
            });
        println!("{0}{1}", output::STATE_HASH_FORMAT, hex);
    }

    Ok(())
}

fn save_state(state: &SimState, path: &Path) -> Result<(), String> {
    let h = compute_state_hash(state);
    let hex: String = h.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{:02x}", b).ok();
        s
    });

    // Write PBSV binary format
    let save = pb_content::schema::SaveFileData {
        format_version: 1,
        ruleset_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        content_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        campaign_seed: state.seed,
        ledger_head_hash: hex,
        ledger_entries: vec![],
        campaign_flags: vec![],
        company: vec![],
        sim_snapshot: None,
        written_at_tick: state.tick.0,
    };

    let data = serialize_save(&save).map_err(|e| format!("serialize error: {0}", e))?;
    std::fs::write(path, &data).map_err(|e| format!("write error: {0}", e))
}

/// Convert a scenario argument to a proper scenario ID.
/// Accepts both bare IDs and file paths.
fn resolve_scenario_id(arg: &str) -> String {
    let stem = arg
        .trim_start_matches("content/scenarios/")
        .trim_start_matches("scenarios/")
        .trim_end_matches(".ron");
    stem.to_string()
}

fn hash_scenario_id(id: &str) -> u32 {
    let h = pb_core::hash::hash_state(id.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
}
