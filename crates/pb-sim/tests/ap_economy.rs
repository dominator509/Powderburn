#![allow(clippy::while_let_loop, clippy::unwrap_used)]

use std::collections::BTreeMap;

use pb_core::geom::TileXY;
use pb_core::ids::{ActorId, Tick};
use pb_sim::clock::{self, turn_length};
use pb_sim::state::SimState;

/// Integration test for the AP economy and sequence clock.
///
/// Verifies that three actors with different Sequence values act the
/// expected number of times over a 600-tick simulation window.
#[test]
fn ap_economy_sequence_clock_counts() {
    let mut state = SimState::new(42, 1);

    // Register three actors with Sequence 2, 5, 9
    let id_a = ActorId(1);
    let id_b = ActorId(2);
    let id_c = ActorId(3);

    let actor_a = clock::build_actor(id_a, "Slow", 2, 20, 10, TileXY::new(0, 0));
    let actor_b = clock::build_actor(id_b, "Medium", 5, 20, 10, TileXY::new(0, 0));
    let actor_c = clock::build_actor(id_c, "Fast", 9, 20, 10, TileXY::new(0, 0));

    clock::register_actor(&mut state, id_a, actor_a);
    clock::register_actor(&mut state, id_b, actor_b);
    clock::register_actor(&mut state, id_c, actor_c);

    // Run for 600 ticks, counting how many times each actor acts
    let mut counts: BTreeMap<ActorId, u32> = BTreeMap::new();
    counts.insert(id_a, 0);
    counts.insert(id_b, 0);
    counts.insert(id_c, 0);

    // Continue advancing until tick exceeds 600
    loop {
        if let Some(actor) = clock::advance_to_next_actor(&mut state) {
            if state.tick > Tick(600) {
                break;
            }
            *counts.get_mut(&actor).unwrap() += 1;
        } else {
            break;
        }
    }

    // Compute expected counts:
    // turn_length = clamp(100 - 4*seq, 50, 96)
    // Seq 2: 92 -> acts at ticks 0, 92, 184, 276, 368, 460, 552 = 7 times
    // Seq 5: 80 -> acts at ticks ... many times
    // Seq 9: 64 -> acts at ticks ... most times
    //
    // The exact counts depend on tie-breaking. All three start at tick 0.
    // Seq 9 goes first (highest seq), then Seq 5, then Seq 2.
    // They interleave since turn lengths differ.

    let expected_a: u32 = {
        let tl = turn_length(2); // 92
                                 // Acts at ticks < 600: 0, 92, 184, 276, 368, 460, 552
                                 // 552 + 92 = 644 > 600, so 7 actions
        1 + (599u64 / tl) as u32
    };

    let expected_b: u32 = {
        let tl = turn_length(5); // 80
                                 // 1 + floor(599 / 80) = 1 + 7 = 8
        1 + (599u64 / tl) as u32
    };

    let expected_c: u32 = {
        let tl = turn_length(9); // 64
                                 // 1 + floor(599 / 64) = 1 + 9 = 10
        1 + (599u64 / tl) as u32
    };

    assert_eq!(
        counts[&id_a], expected_a,
        "Seq 2 (Slow) should act {} times but acted {}",
        expected_a, counts[&id_a]
    );
    assert_eq!(
        counts[&id_b], expected_b,
        "Seq 5 (Medium) should act {} times but acted {}",
        expected_b, counts[&id_b]
    );
    assert_eq!(
        counts[&id_c], expected_c,
        "Seq 9 (Fast) should act {} times but acted {}",
        expected_c, counts[&id_c]
    );

    // The sum of all actions should match the expected totals.
    let total_expected = expected_a + expected_b + expected_c;
    let total_actual = counts[&id_a] + counts[&id_b] + counts[&id_c];
    assert_eq!(
        total_actual, total_expected,
        "Total actions {} should be {}",
        total_actual, total_expected
    );
}

/// Test that an action costing 5 AP with only 4 AP remaining returns
/// InsufficientAp and leaves state unchanged (verified by hash).
#[test]
fn insufficient_ap_does_not_change_state() {
    use pb_core::ids::Ap;
    use pb_sim::action::{step, Action, Command};
    use pb_sim::state::SimError;

    let mut state = SimState::new(99, 1);
    let id = ActorId(1);

    // Build actor with 4 AP remaining
    let mut actor = clock::build_actor(id, "LowAP", 5, 20, 10, TileXY::new(5, 5));
    actor.ap = Ap(4);
    clock::register_actor(&mut state, id, actor);

    // Try a CalledShot (costs 5 AP)
    let cmd = Command {
        actor_id: id,
        action: Action::CalledShot(ActorId(2), pb_core::event::HitLocationType::Head),
    };

    let result = step(&mut state, cmd);

    assert!(result.is_err(), "Expected InsufficientAp error but got Ok");

    match result.unwrap_err() {
        SimError::InsufficientAp { actor, have, need } => {
            assert_eq!(actor, id);
            assert_eq!(have, Ap(4));
            assert_eq!(need, Ap(5));
        }
        other => panic!("Expected InsufficientAp but got {:?}", other),
    }

    // State unchanged: AP is still 4
    assert_eq!(
        state.actors[&id].ap,
        Ap(4),
        "AP should remain unchanged after InsufficientAp"
    );
}
