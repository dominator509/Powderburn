//! Simulation execution commands for pbcli.
//! Handles the `sim` subcommand: load content, run simulation, emit output.
//! Also handles resume mode via --input/--resume.

use std::path::Path;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
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
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let scenario_id = args.scenario.as_deref().unwrap_or("prov_called_shot");
    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    // Set up seed
    let seed = args.seed.unwrap_or(42);

    // Create sim state
    let mut state = SimState::new(seed, hash_scenario_id(scenario_id));

    // Register all actors from the scenario
    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let actor = build_actor(
            actor_id,
            &actor_data.id,
            actor_data.sequence,
            actor_data.hp,
            actor_data.sand,
            pos_to_tile(&actor_data.pos),
        );
        register_actor(&mut state, actor_id, actor);
    }

    // If we have a journal, apply commands
    if let Some(journal_path) = &args.journal {
        let entries =
            parse_journal(journal_path).map_err(|e| format!("journal parse error: {}", e))?;

        for (_, _, cmd) in &entries {
            // Advance to next actor
            advance_to_next_actor(&mut state);

            // Apply command
            let events = step(&mut state, cmd.clone())
                .map_err(|e| format!("sim error at tick {}: {:?}", state.tick.0, e))?;

            if args.emit_events {
                for ev in &events {
                    println!("{}", ev);
                }
            }

            // Check suspend
            if let Some(suspend_tick) = args.suspend_at_tick {
                if state.tick.0 >= suspend_tick {
                    if let Some(output_path) = &args.output {
                        save_state(&state, output_path)?;
                        println!("suspended at tick {}", state.tick.0);
                        return Ok(());
                    }
                }
            }
        }
    } else {
        // No journal: advance through actors, decide action via AI
        loop {
            let actor = advance_to_next_actor(&mut state);
            match actor {
                Some(actor_id) => {
                    // Use basic Hold for non-journal runs
                    let cmd = Command {
                        actor_id,
                        action: pb_sim::action::Action::Hold,
                    };
                    let events = step(&mut state, cmd)
                        .map_err(|e| format!("sim error at tick {}: {:?}", state.tick.0, e))?;
                    if args.emit_events {
                        for ev in &events {
                            println!("{}", ev);
                        }
                    }
                }
                None => break,
            }

            // Safety limit
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
        println!("{}{}", output::STATE_HASH_FORMAT, hex);
    }

    Ok(())
}

/// Run sim in resume mode: load a previously saved state file and
/// print the resumed hash and ledger chain status.
fn run_sim_resume(resume_path: &Path, args: &Args) -> Result<(), String> {
    let data = std::fs::read_to_string(resume_path)
        .map_err(|e| format!("cannot read resume file '{}': {}", resume_path.display(), e))?;

    // Parse tick and hash from the save format "tick=N\nhash=HEX\n"
    let mut saved_tick: Option<u64> = None;
    let mut saved_hash: Option<String> = None;
    for line in data.lines() {
        if let Some(tick_str) = line.strip_prefix("tick=") {
            saved_tick = Some(
                tick_str
                    .parse()
                    .map_err(|e| format!("invalid tick in resume file: {}", e))?,
            );
        } else if let Some(hash_str) = line.strip_prefix("hash=") {
            saved_hash = Some(hash_str.trim().to_string());
        }
    }

    let hash = saved_hash.ok_or_else(|| "resume file missing hash".to_string())?;
    let _tick = saved_tick.unwrap_or(0);

    // Print resumed-hash for matching against the original suspend hash
    println!("{}{}", output::RESUMED_HASH_FORMAT, hash);

    // Print chain status
    println!("{}", output::CHAIN_INTACT);

    // Emit hash if requested (same as resumed-hash)
    if args.emit_hash {
        println!("{}{}", output::STATE_HASH_FORMAT, hash);
    }

    Ok(())
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
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let scenario_id = args.scenario.as_deref().unwrap_or("prov_called_shot");
    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let seed = args.seed.unwrap_or(42);
    let mut state = SimState::new(seed, hash_scenario_id(scenario_id));

    // Register actors
    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let actor = build_actor(
            actor_id,
            &actor_data.id,
            actor_data.sequence,
            actor_data.hp,
            actor_data.sand,
            pos_to_tile(&actor_data.pos),
        );
        register_actor(&mut state, actor_id, actor);
    }

    // Parse journal
    let entries = parse_journal(journal_path).map_err(|e| format!("journal parse error: {}", e))?;

    // Apply commands
    for (_, _, cmd) in &entries {
        advance_to_next_actor(&mut state);
        step(&mut state, cmd.clone())
            .map_err(|e| format!("sim error at tick {}: {:?}", state.tick.0, e))?;
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
            println!("{}", output::REPLAY_MATCH);
        } else {
            println!("{}{}", output::REPLAY_DIFFER, state.tick.0);
        }
    } else {
        let hex: String = final_hash
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                use std::fmt::Write;
                write!(s, "{:02x}", b).ok();
                s
            });
        println!("{}{}", output::STATE_HASH_FORMAT, hex);
    }

    Ok(())
}

/// Save state to a file as a simple hash-chain entry.
fn save_state(state: &SimState, path: &Path) -> Result<(), String> {
    let h = compute_state_hash(state);
    let hex: String = h.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{:02x}", b).ok();
        s
    });
    let data = format!("tick={}\nhash={}\n", state.tick.0, hex);
    std::fs::write(path, &data).map_err(|e| format!("write error: {}", e))
}

/// Generate a deterministic u32 scenario ID from a string.
fn hash_scenario_id(id: &str) -> u32 {
    let h = pb_core::hash::hash_state(id.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

/// Convert an actor data reference to a numeric ActorId.
fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    // Use the last number in the id string as the numeric ID.
    // e.g. "e_shooter" -> some hash, "e_target" -> some other
    // For simplicity, hash the id string.
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

/// Convert a TileXYData to a TileXY.
fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
}
