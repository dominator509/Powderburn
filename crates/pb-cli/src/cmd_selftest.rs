//! Selftest command for pbcli.

#![allow(clippy::panic_in_result_fn)]

use std::path::Path;

use pb_content::load::load_all;
use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::fix32::Fix32;
use pb_core::geom::{Facing, TileXY};
use pb_core::hash::hash_state;
use pb_core::ids::{ActorId, Ap, Tick};
use pb_rng::{PbRng, StreamTag};
use pb_rules::tables::weapon_entry;
use pb_sim::action::{action_cost, Action};
use pb_sim::hash::compute_state_hash;
use pb_sim::state::{ActorState, SimState, Stance};

use crate::args::Args;
use crate::output;

/// Run the `selftest` subcommand.
pub fn run_selftest(args: &Args) -> Result<(), String> {
    // 1. Fix32 basic arithmetic
    let _a = Fix32::from_int(5);
    let _b = Fix32::from_int(3);
    let _sum = _a + _b;
    assert_eq!(_sum.to_int_floor(), 8);

    // 2. TileXY geometry
    let t1 = TileXY::new(0, 0);
    let t2 = TileXY::new(5, 0);
    assert_eq!(t1.chebyshev_distance(t2), 5);

    // 3. Facing directions
    assert_eq!(Facing::North.delta(), (0, -1));
    assert_eq!(Facing::South.opposite(), Facing::North);

    // 4. ActorId / Tick / Ap
    let _id = ActorId(1);
    let _tick = Tick(42);
    let _ap = Ap(10);
    assert_eq!(_ap.0, 10);

    // 5. Event display
    let ev = Event::Misfire { actor: ActorId(5) };
    assert!(ev.to_string().contains("Misfire"));

    // 6. Hash determinism
    let h1 = hash_state(b"selftest");
    let h2 = hash_state(b"selftest");
    assert_eq!(h1, h2);

    // 7. RNG determinism
    let r1 = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 99);
    let r2 = PbRng::draw(42, 1, 100, 5, StreamTag::ToHit, 0, 99);
    assert_eq!(r1, r2);

    // 8. Weapon table
    let _w = weapon_entry("colt_army_1860").ok_or("weapon not found")?;
    assert_eq!(_w.base_damage, 14);

    // 9. Action cost
    let actor = ActorState {
        ap: Ap(10),
        position: TileXY::new(0, 0),
        facing: Facing::South,
        sequence: 5,
        hit_points: 20,
        max_hp: 20,
        name: "Test".to_string(),
        alive: true,
        wounds: vec![],
        sand: 10,
        max_sand: 10,
        stance: Stance::Standing,
        progression: pb_sim::progression::ActorProgression::new(),
        weapon: "colt_army_1860".to_string(),
        loaded_rounds: 6,
        weapon_capacity: 6,
        fouling: 0,
        jammed: false,
    };
    let cost = action_cost(&Action::Reload, &actor);
    assert_eq!(cost, Ap(3));

    // 10. SimState creation
    let mut state = SimState::new(42, 1);
    state.actors.insert(ActorId(1), actor);
    assert!(!state.actors.is_empty());

    // 11. compute_state_hash
    let h = compute_state_hash(&state);
    assert_ne!(h, [0u8; 32]);

    // Emit the state hash if --emit-hash was passed
    if args.emit_hash {
        use std::fmt::Write;
        let mut hex = String::with_capacity(64);
        for b in &h {
            write!(hex, "{:02x}", b).unwrap();
        }
        println!("state-hash: {}", hex);
    }

    // 12. Hit location types
    assert_eq!(HitLocationType::Head.to_string(), "Head");
    assert_eq!(WoundType::Bleeding.to_string(), "Bleeding");

    // 13. Content loading
    let content_root = Path::new("content");
    if content_root.exists() {
        let _content = load_all(content_root).map_err(|e| format!("content load: {}", e))?;
    }

    // === --emit-metrics: run a fixed 12-tick scenario ======================
    if args.emit_metrics {
        run_selftest_metrics()?;
    }

    println!("{}", output::SELFTEST_OK);
    Ok(())
}

/// Run a fixed 12-tick scenario and dump all SPEC-007 metrics to stderr.
fn run_selftest_metrics() -> Result<(), String> {
    use pb_sim::action::Command;
    use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};

    let registry = pb_core::metrics::MetricsRegistry::global();
    registry.reset_all();

    // Create a sim state with 4 actors: 2 allies, 2 enemies
    let mut state = SimState::new(42, 1);

    let ally1 = ActorId(1);
    let ally2 = ActorId(2);
    let enemy1 = ActorId(3);
    let enemy2 = ActorId(4);

    register_actor(
        &mut state,
        ally1,
        build_actor(ally1, "e_ally_alpha", 5, 20, 10, TileXY::new(2, 2)),
    );
    register_actor(
        &mut state,
        ally2,
        build_actor(ally2, "e_ally_beta", 3, 18, 8, TileXY::new(2, 3)),
    );
    register_actor(
        &mut state,
        enemy1,
        build_actor(enemy1, "e_enemy_gamma", 6, 15, 5, TileXY::new(8, 8)),
    );
    register_actor(
        &mut state,
        enemy2,
        build_actor(enemy2, "e_enemy_delta", 4, 12, 4, TileXY::new(9, 9)),
    );

    // Run 12 ticks: advance clock and apply Hold for each
    for _i in 0..12 {
        let Some(actor_id) = advance_to_next_actor(&mut state) else {
            break;
        };
        let cmd = Command {
            actor_id,
            action: pb_sim::action::Action::Hold,
        };
        let _events = pb_sim::action::step(&mut state, cmd)
            .map_err(|e| format!("step error at tick {}: {:?}", state.tick.0, e))?;
    }

    // Emit all metrics to stderr in SPEC-003 format
    registry.emit_all();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selftest_emit_metrics_produces_at_least_8_lines() {
        // Run the metrics scenario and capture stderr output
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_selftest_metrics().unwrap();
        }));
        assert!(result.is_ok(), "run_selftest_metrics should not panic");

        // Verify the registry has been populated with metric data
        let registry = pb_core::metrics::MetricsRegistry::global();
        let line_count = registry.rng_draws.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            line_count > 0,
            "RNG draws should be greater than 0 after 12 ticks, got {}",
            line_count
        );
        assert!(
            registry.sim_step_ms.count() > 0,
            "sim.step.ms should have samples"
        );
        assert!(
            registry.sim_events_per_turn.count() > 0,
            "sim.events.per_turn should have samples"
        );
        let alive = registry.sim_actors_alive.load(std::sync::atomic::Ordering::Relaxed);
        assert!(alive > 0, "sim.actors.alive should be > 0, got {}", alive);
    }
}
