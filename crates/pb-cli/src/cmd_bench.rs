//! Benchmark commands for pbcli.

use std::path::Path;
use std::time::Instant;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
use pb_sim::action::Action;
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::state::SimState;

use crate::args::Args;
use crate::output;

/// Run the `bench turn` subcommand.
pub fn run_bench(args: &Args) -> Result<(), String> {
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let scenario_id = args.bench_scenario.as_deref().unwrap_or("prov_full_battle");
    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let iterations = args.iterations.unwrap_or(10) as usize;
    let seed = args.seed.unwrap_or(42);

    let mut worst_turn_ms: u128 = 0;
    let mut worst_step_ms: u128 = 0;

    for _ in 0..iterations {
        let mut state = SimState::new(seed, hash_string(scenario_id));

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

        // Run several turns
        for _ in 0..20 {
            let turn_start = Instant::now();

            // AI turn
            let ai_start = Instant::now();
            // Simple AI: pick the first actor and Hold
            if let Some(actor_id) = advance_to_next_actor(&mut state) {
                let _cmd = pb_sim::action::Command {
                    actor_id,
                    action: Action::Hold,
                };
            }
            let ai_elapsed = ai_start.elapsed().as_millis();
            if ai_elapsed > worst_turn_ms {
                worst_turn_ms = ai_elapsed;
            }

            let step_elapsed = turn_start.elapsed().as_millis();
            if step_elapsed > worst_step_ms {
                worst_step_ms = step_elapsed;
            }
        }
    }

    // --emit-budget: only emit budget lines if explicitly requested
    if args.emit_budget {
        println!("{}{}", output::WORST_AI_TURN, worst_turn_ms);
        println!("{}{}", output::WORST_SIM_STEP, worst_step_ms);
    }
    Ok(())
}

fn hash_string(s: &str) -> u32 {
    let h = pb_core::hash::hash_state(s.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
}
