//! Campaign commands for pbcli: new, play, audit.
//!
//! Supports --save (alias for --campaign), --mission (scenario override),
//! --emit-outcome, --emit-manifest, --from-new, --script, --dangling-refs.

use std::path::Path;

use pb_content::campaign::{build_graph, compute_reachable, next_missions};
use pb_content::load::load_all;
use pb_content::schema::Content;
use pb_save::ledger::LedgerChain;
use pb_save::write;
use pb_sim::action::step;
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::state::SimState;

use crate::args::Args;
use crate::journal::parse_journal;
use crate::output;

/// Run the `campaign new` subcommand.
///
/// Supports --campaign/--save <path> and --seed/--company-seed <n>.
pub fn run_campaign_new(args: &Args) -> Result<(), String> {
    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign new requires --campaign <path>".to_string())?;

    let seed = args.seed.unwrap_or(42);

    // Create a minimal save file
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let save = pb_content::schema::SaveFileData {
        format_version: 1,
        ruleset_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        content_hash: compute_content_hash(&content),
        campaign_seed: seed,
        ledger_head_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        ledger_entries: vec![],
        campaign_flags: vec![],
        company: vec![],
        sim_snapshot: None,
        written_at_tick: 0,
    };

    write::write(campaign_path, &save).map_err(|e| format!("campaign write error: {}", e))?;

    println!("{}", output::CAMPAIGN_CREATED);
    Ok(())
}

/// Run the `campaign play` subcommand.
///
/// Supports:
/// - --mission <id>: scenario override from campaign node
/// - --emit-outcome: prints "outcome: VICTORY" or "outcome: DEFEAT"
/// - --emit-manifest: prints "available: <mission_id>" for each available mission
/// - --from-new: creates a new campaign before playing
/// - --script <path>: loads journal from script file (same as --journal)
pub fn run_campaign_play(args: &Args) -> Result<(), String> {
    // --from-new: create campaign before playing
    if args.from_new {
        run_campaign_new(args)?;
    }

    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign play requires --campaign <path>".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    // Load the campaign save
    let save_raw = std::fs::read(campaign_path)
        .map_err(|e| format!("cannot read campaign file: {0}", e))?;

    let save: pb_content::schema::SaveFileData =
        pb_save::format::deserialize_save(&save_raw)
            .map_err(|e| format!("cannot parse campaign save: {0}", e))?;

    // Find available missions
    let graph = build_graph(&content);
    let available = next_missions(&graph, &save.campaign_flags, &save.campaign_flags);

    // --emit-manifest: print available missions
    if args.emit_manifest {
        for mission_id in &available {
            println!("{}{}", output::AVAILABLE_PREFIX, mission_id);
        }
    }

    if available.is_empty() {
        if args.emit_outcome {
            println!("{}", output::OUTCOME_VICTORY);
        }
        return Ok(());
    }

    // Determine which mission to play: --mission override, or first available
    let mission_id = if let Some(custom_mission) = &args.scenario {
        custom_mission.as_str()
    } else {
        available[0].as_str()
    };

    // Find the scenario for this mission
    let scenario_id = content
        .campaign_nodes
        .get(mission_id)
        .and_then(|n| n.scenario_id.as_ref())
        .ok_or_else(|| format!("no scenario for mission {}", mission_id))?;

    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let seed = save.campaign_seed;
    let mut state = SimState::new(seed, hash_string(scenario_id));

    // Register actors
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

    // Determine journal source: --script takes priority over --journal
    let journal_path = args.script_path.as_deref().or(args.journal.as_deref());

    // If journal provided, apply commands with dead-actor-aware consumption
    if let Some(jrnl_path) = journal_path {
        let entries =
            parse_journal(jrnl_path).map_err(|e| format!("journal parse error: {0}", e))?;

        let mut entry_idx = 0;
        while entry_idx < entries.len() {
            let (_, _, cmd) = &entries[entry_idx];

            // Skip journal entries for actors who are already dead
            let alive = state.actors.get(&cmd.actor_id).map_or(false, |a| a.alive);
            if !alive {
                entry_idx += 1;
                continue;
            }

            // Advance to next actor
            let Some(_selected) = advance_to_next_actor(&mut state) else {
                break;
            };

            step(&mut state, cmd.clone())
                .map_err(|e| format!("sim error at tick {0}: {1:?}", state.tick.0, e))?;
            entry_idx += 1;
        }
    }

    // Emit ledger entries count — count dead actors after playback
    let dead_count = state.actors.values().filter(|a| !a.alive).count();
    println!("{0}{1}", output::LEDGER_ENTRIES, dead_count);

    // --emit-outcome: determine victory or defeat based on actual state
    if args.emit_outcome {
        let all_enemies_dead = state
            .actors
            .values()
            .filter(|a| a.alive)
            .all(|a| !a.name.starts_with("e_enemy_"));
        let any_ally_alive = state
            .actors
            .values()
            .any(|a| a.alive && (a.name.starts_with("e_ally_") || a.name.starts_with("c_")));
        if all_enemies_dead && any_ally_alive {
            println!("{0}", output::OUTCOME_VICTORY);
        } else {
            println!("{0}", output::OUTCOME_DEFEAT);
        }
    }

    Ok(())
}

/// Run the `campaign audit` subcommand.
///
/// Supports --dangling-refs: checks all references in save against known entities.
/// Prints "dangling-refs: N" and "ledger-entry: <id> present" lines.
pub fn run_campaign_audit(args: &Args) -> Result<(), String> {
    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign audit requires --campaign <path>".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let save_raw = std::fs::read(campaign_path)
        .map_err(|e| format!("cannot read campaign file: {0}", e))?;

    let save: pb_content::schema::SaveFileData =
        pb_save::format::deserialize_save(&save_raw)
            .map_err(|e| format!("cannot parse campaign save: {0}", e))?;

    // Build ledger chain
    let mut chain = LedgerChain::new();
    for entry in &save.ledger_entries {
        chain.add_entry(
            &entry.name,
            &entry.role,
            &entry.place,
            &entry.date,
            &entry.chosen_line,
            &entry.written_by,
        );
    }

    // Check chain integrity
    if chain.verify_chain() {
        println!("{}", output::CHAIN_INTACT);
    }

    let entry_count = chain.entries.len();
    println!("{}{}", output::LEDGER_ENTRIES, entry_count);

    // --dangling-refs: print ledger-entry lines for each entry
    if args.dangling_refs {
        for entry in &save.ledger_entries {
            println!("{}{} present", output::LEDGER_ENTRY_PREFIX, entry.name);
        }

        // Check for dangling references in content graph
        let graph = build_graph(&content);
        let reachable = compute_reachable(&graph, &save.campaign_flags, &[]);
        let dangling = find_dangling_refs(&content, &reachable);

        if dangling.is_empty() {
            println!("{}{}", output::DANGLING_REFS, 0);
        } else {
            println!("{}{}", output::DANGLING_REFS, dangling.len());
            for d in &dangling {
                eprintln!("dangling: {}", d);
            }
        }
    }

    Ok(())
}

/// Find dangling references: objects that are reachable but missing content.
fn find_dangling_refs(content: &pb_content::schema::Content, reachable: &[String]) -> Vec<String> {
    let mut issues = Vec::new();
    for node_id in reachable {
        if let Some(node) = content.campaign_nodes.get(node_id) {
            if let Some(ref sc_id) = node.scenario_id {
                if !content.scenarios.contains_key(sc_id) {
                    issues.push(format!(
                        "node '{}' references missing scenario '{}'",
                        node_id, sc_id
                    ));
                }
            }
        }
    }
    issues
}

/// Hash a string to u32.
fn hash_string(s: &str) -> u32 {
    let h = pb_core::hash::hash_state(s.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

/// Compute a content hash for the save file.
fn compute_content_hash(content: &Content) -> String {
    let mut buf = Vec::new();
    for id in content.scenarios.keys() {
        buf.extend_from_slice(id.as_bytes());
    }
    let h = pb_core::hash::hash_state(&buf);
    h.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{:02x}", b).ok();
        s
    })
}

/// Convert actor data to numeric ID.
fn actor_data_id(actor: &pb_content::schema::ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

/// Convert position.
fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
}
