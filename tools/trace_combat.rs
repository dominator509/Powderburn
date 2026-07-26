//! Trace combat events for the prov_full_battle scenario with SHA-256 actor IDs.
//! Run: cargo run --release --offline --bin trace_combat

use std::path::Path;

fn hash_scenario_id(id: &str) -> u32 {
    let h = pb_core::hash::hash_state(id.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(id: &str) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

fn main() {
    // Print all actor IDs
    let names = [
        "e_ally_01",
        "e_ally_02",
        "e_enemy_01",
        "e_enemy_02",
        "e_shooter",
        "e_target",
        "c_whitehorse",
    ];
    for name in &names {
        let id = actor_data_id(name);
        println!("ActorId({:20}) = {}", name, id.0);
    }

    // Load content
    let content = pb_content::load::load_all(Path::new("content")).expect("content load failed");
    let scenario_id = "prov_full_battle";
    let scenario = content
        .scenarios
        .get(scenario_id)
        .expect("scenario not found");

    let seed = 1867u64;
    let mut state = pb_sim::state::SimState::new(seed, hash_scenario_id(scenario_id));

    use pb_sim::clock::{build_actor, register_actor};

    // Register actors
    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(&actor_data.id);
        let actor = build_actor(
            actor_id,
            &actor_data.id,
            actor_data.sequence,
            actor_data.hp,
            actor_data.sand,
            pb_core::geom::TileXY::new(actor_data.pos.x, actor_data.pos.y),
        );
        register_actor(&mut state, actor_id, actor);
    }

    let ally_02 = actor_data_id("e_ally_02");
    let enemy_02 = actor_data_id("e_enemy_02");
    let ally_01 = actor_data_id("e_ally_01");
    let enemy_01 = actor_data_id("e_enemy_01");

    use pb_sim::action::{step, Action, Command};
    use pb_sim::clock::advance_to_next_actor;

    let mut round = 0u64;
    loop {
        round += 1;
        let actor_order = [ally_02, enemy_02, ally_01, enemy_01];

        for &actor_id in &actor_order {
            let actor_state = state.actors.get(&actor_id);
            if actor_state.is_none_or(|a| !a.alive) {
                // Dead actor - still need to advance clock for next actor
                // Actually, advance_to_next_actor only picks alive actors, so we skip dead ones
                continue;
            }

            // Determine action based on actor
            let action = if actor_id == ally_02 || actor_id == ally_01 {
                let target = if actor_id == ally_02 {
                    enemy_02
                } else {
                    enemy_01
                };
                // Check if target is alive
                let target_alive = state.actors.get(&target).is_some_and(|a| a.alive);
                if target_alive {
                    // Check if we have enough AP for SnapShot (cost 4)
                    if actor_state.unwrap().ap.0 >= 4 {
                        Action::SnapShot(target)
                    } else if actor_state.unwrap().ap.0 >= 2 {
                        // Move toward enemy
                        let current = actor_state.unwrap().position;
                        Action::Move(pb_core::geom::TileXY::new(current.x + 4, current.y))
                    } else {
                        Action::Hold
                    }
                } else {
                    Action::Hold
                }
            } else {
                Action::Hold
            };

            advance_to_next_actor(&mut state);

            let cmd = Command {
                actor_id,
                action: action.clone(),
            };

            match step(&mut state, cmd) {
                Ok(events) => {
                    for ev in &events {
                        println!("[tick={:5}] {}", state.tick.0, ev);
                    }
                }
                Err(e) => {
                    println!("[tick={:5}] ERROR: {} -> {:?}", state.tick.0, actor_id.0, e);
                }
            }
        }

        // Check if all enemies are dead
        let enemies_alive = state
            .actors
            .iter()
            .any(|(id, a)| a.alive && (*id == enemy_01 || *id == enemy_02));
        if !enemies_alive {
            println!("\n=== All enemies dead at round {} ===", round);
            break;
        }

        if round >= 30 {
            println!("\n=== Reached max rounds ===");
            break;
        }
    }

    // Print final state
    for (id, actor) in &state.actors {
        println!(
            "Final: {} -> hp={}, alive={}",
            id.0, actor.hit_points, actor.alive
        );
    }
}
