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

    println!("{}", output::SELFTEST_OK);
    Ok(())
}
