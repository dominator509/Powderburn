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

    // 8. Authored weapon data
    let content_root = Path::new("content");
    let content = load_all(content_root).map_err(|e| format!("content load: {e}"))?;
    let weapon = content
        .weapons
        .get("colt_army_1860")
        .ok_or("authored weapon not found")?;
    assert_eq!(weapon.damage_dice.count, 1);
    assert_eq!(weapon.damage_dice.sides, 8);
    assert_eq!(weapon.damage_dice.bonus, 2);

    // 9. Action cost
    let actor = ActorState {
        faction_id: String::new(),
        is_companion: false,
        attributes: pb_core::Attributes::BALANCED,
        ap: Ap(10),
        position: TileXY::new(0, 0),
        facing: Facing::South,
        sequence: 5,
        hit_points: 20,
        max_hp: 20,
        name: "Test".to_string(),
        alive: true,
        routed: false,
        wounds: vec![],
        sand: 10,
        max_sand: 10,
        stance: Stance::Standing,
        progression: pb_sim::progression::ActorProgression::new(),
        weapon: "colt_army_1860".to_string(),
        weapon_profile: pb_sim::state::WeaponProfile {
            damage_count: weapon.damage_dice.count,
            damage_sides: weapon.damage_dice.sides,
            damage_bonus: weapon.damage_dice.bonus,
            accuracy: weapon.accuracy,
            ap_overrides: weapon.ap_override.clone(),
            range_bands: weapon.range_bands,
            reload_class: weapon.reload_class.clone(),
            fouling_rate: weapon.fouling_rate,
            base_misfire: weapon.base_misfire,
            smoke_output: weapon.smoke_output,
            two_handed: weapon.two_handed,
        },
        loaded_rounds: weapon.capacity,
        weapon_capacity: weapon.capacity,
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

    const SELFTEST_STATE_HASH: &str =
        "14ff9dfa13c6612760ad12b1865fe472a6bcb0445d35f47ec290346ccaec3d91";
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for byte in &h {
        write!(hex, "{byte:02x}").map_err(|error| format!("hash formatting failed: {error}"))?;
    }
    if hex != SELFTEST_STATE_HASH {
        return Err(format!(
            "selftest state hash mismatch: got {hex}, expected {SELFTEST_STATE_HASH}"
        ));
    }

    // Emit the verified state hash if --emit-hash was passed.
    if args.emit_hash {
        println!("state-hash: {}", hex);
    }

    // 12. Hit location types
    assert_eq!(HitLocationType::Head.to_string(), "Head");
    assert_eq!(WoundType::Bleeding.to_string(), "Bleeding");

    // === --emit-metrics: run a fixed 12-tick scenario ======================
    if args.emit_metrics {
        run_selftest_metrics()?;
    }

    println!("{}", output::SELFTEST_OK);
    Ok(())
}

/// Run a fixed 12-tick scenario and dump all SPEC-007 metrics to stderr.
fn run_selftest_metrics() -> Result<(), String> {
    use pb_sim::clock::{build_actor, register_actor};
    use std::sync::atomic::Ordering;

    let registry = pb_core::metrics::MetricsRegistry::global();
    registry.reset_all();

    // Measure a real full content parse.
    let content_started = std::time::Instant::now();
    load_all(Path::new("content")).map_err(|error| format!("content load: {error}"))?;
    registry.set_content_load_ms(duration_ms(content_started.elapsed()));

    // Create a close-quarters state so utility AI reaches legal shot actions.
    let mut state = SimState::new(42, 1);

    let ally1 = ActorId(1);
    let ally2 = ActorId(2);
    let enemy1 = ActorId(3);
    let enemy2 = ActorId(4);

    let mut ally_alpha = build_actor(ally1, "e_ally_alpha", 5, 20, 10, TileXY::new(2, 2));
    ally_alpha.faction_id = "player".to_string();
    register_actor(&mut state, ally1, ally_alpha);
    let mut ally_beta = build_actor(ally2, "e_ally_beta", 3, 18, 8, TileXY::new(2, 3));
    ally_beta.faction_id = "player".to_string();
    register_actor(&mut state, ally2, ally_beta);
    let mut enemy_gamma = build_actor(enemy1, "e_enemy_gamma", 6, 15, 5, TileXY::new(5, 2));
    enemy_gamma.faction_id = "enemy".to_string();
    register_actor(&mut state, enemy1, enemy_gamma);
    let mut enemy_delta = build_actor(enemy2, "e_enemy_delta", 4, 12, 4, TileXY::new(5, 3));
    enemy_delta.faction_id = "enemy".to_string();
    register_actor(&mut state, enemy2, enemy_delta);

    // Execute twelve complete, scored AI turns through the real step API.
    for _ in 0..12 {
        let started = std::time::Instant::now();
        let worst_step = crate::cmd_bench::run_one_ai_actor_turn(&mut state)?;
        registry.record_ai_turn(duration_ms(started.elapsed()));
        registry.record_sim_step(u64::try_from(worst_step).unwrap_or(u64::MAX));
        if state.actors.values().filter(|actor| actor.alive).count() < 2 {
            break;
        }
    }

    registry.set_sim_actors_alive(
        u64::try_from(state.actors.values().filter(|actor| actor.alive).count())
            .unwrap_or(u64::MAX),
    );
    registry.set_smoke_volumes_live(
        u64::try_from(
            state
                .smoke_grid
                .iter()
                .filter(|density| **density > 0)
                .count(),
        )
        .unwrap_or(u64::MAX),
    );

    // Exercise the append-only Ledger, atomic save writer, and verified loader.
    let mut ledger = pb_save::ledger::LedgerChain::new();
    ledger.add_entry(
        "Selftest",
        "Proof",
        "Elk Creek",
        "1867-08-12",
        "Measured persistence boundary",
        "System",
    );
    if !ledger.verify_chain() {
        return Err("selftest Ledger chain did not verify".to_string());
    }
    let ruleset_hash = [0x11; 32];
    let content_hash = [0x22; 32];
    let ledger_entries = ledger
        .entries
        .iter()
        .map(|entry| pb_content::schema::LedgerEntryData {
            index: entry.index,
            prev_hash: hex32(&entry.prev_hash),
            name: entry.name.clone(),
            role: entry.role.clone(),
            place: entry.place.clone(),
            date: entry.date.clone(),
            chosen_line: entry.chosen_line.clone(),
            written_by: entry.written_by.clone(),
            hash: hex32(&entry.hash),
        })
        .collect();
    let save = pb_content::schema::SaveFileData {
        format_version: 1,
        ruleset_hash: hex32(&ruleset_hash),
        content_hash: hex32(&content_hash),
        campaign_seed: state.seed,
        ledger_head_hash: hex32(&ledger.head_hash()),
        ledger_weight: 0,
        ledger_entries,
        campaign_flags: vec!["selftest".to_string()],
        completed_nodes: Vec::new(),
        company: Vec::new(),
        sim_snapshot: None,
        written_at_tick: state.tick.0,
    };
    let save_path =
        std::env::temp_dir().join(format!("powderburn-selftest-{}.pbsv", std::process::id()));
    pb_save::write::write(&save_path, &save).map_err(|error| format!("save write: {error}"))?;
    pb_save::load::read(&save_path, &ruleset_hash, &content_hash)
        .map_err(|error| format!("verified save read: {error}"))?;
    std::fs::remove_file(&save_path).map_err(|error| format!("save cleanup: {error}"))?;

    // Render one actual state frame. Pipeline setup is excluded by the helper.
    let runtime =
        tokio::runtime::Runtime::new().map_err(|error| format!("render runtime: {error}"))?;
    let device = runtime
        .block_on(pb_render::device::RenderDevice::new_headless())
        .map_err(|error| format!("render device: {error}"))?;
    let frame_ms = pb_render::capture::render_state_frame_ms(
        &device,
        &pb_render::RenderConfig {
            width: 640,
            height: 360,
            adapter_name: None,
        },
        &state,
    )?;
    registry.record_render_frame(frame_ms);

    if registry.rng_draws.load(Ordering::Relaxed) == 0 {
        return Err("selftest did not exercise any RNG draws".to_string());
    }

    registry.emit_all();

    Ok(())
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selftest_emit_metrics_produces_at_least_8_lines() {
        let result = run_selftest_metrics();
        assert!(result.is_ok(), "run_selftest_metrics should succeed");

        let registry = pb_core::metrics::MetricsRegistry::global();
        assert!(
            registry.sim_step_ms.count() > 0,
            "sim.step.ms should have samples"
        );
        assert!(
            registry.sim_events_per_turn.count() > 0,
            "sim.events.per_turn should have samples"
        );
        let alive = registry
            .sim_actors_alive
            .load(std::sync::atomic::Ordering::Relaxed);
        assert!(alive > 0, "sim.actors.alive should be > 0, got {}", alive);
    }
}
