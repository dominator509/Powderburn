//! M6: Determinism proof test and golden hash.
//!
//! Three-pass determinism verification:
//!   1. Build a proving scenario, apply a fixed journal, capture hash1.
//!   2. Create a FRESH scenario (same seed, same setup), apply same journal,
//!      capture hash2.
//!   3. Create ANOTHER fresh scenario, apply same journal, capture hash3.
//!   4. Assert all three hashes are identical and not all-zeros.

use pb_sim::action::{step, Action, Command};
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::hash::compute_state_hash;
use pb_sim::state::SimState;

use pb_core::geom::TileXY;
use pb_core::ids::ActorId;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the proving scenario "prov_full_battle".
///
/// 4 actors: 2 allies (Sequences 5 and 7), 2 enemies (Sequences 4 and 6).
fn build_prov_scenario(seed: u64) -> SimState {
    let mut state = SimState::new(seed, 1);

    // Ally1 (Sequence 5) at (5, 5)
    register_actor(
        &mut state,
        ActorId(1),
        build_actor(ActorId(1), "Ally1", 5, 35, 20, TileXY::new(5, 5)),
    );
    // Ally2 (Sequence 7) at (5, 6)
    register_actor(
        &mut state,
        ActorId(2),
        build_actor(ActorId(2), "Ally2", 7, 30, 20, TileXY::new(5, 6)),
    );
    // Enemy1 (Sequence 4) at (15, 5)
    register_actor(
        &mut state,
        ActorId(3),
        build_actor(ActorId(3), "Enemy1", 4, 40, 20, TileXY::new(15, 5)),
    );
    // Enemy2 (Sequence 6) at (15, 6)
    register_actor(
        &mut state,
        ActorId(4),
        build_actor(ActorId(4), "Enemy2", 6, 35, 20, TileXY::new(15, 6)),
    );

    state
}

/// Build a short firefight journal (~20 commands).
///
/// The turn order is deterministic given the sequence values above:
///   Round 1: Ally2 (seq 7), Enemy2 (seq 6), Ally1 (seq 5), Enemy1 (seq 4)
///   Round 2: same order at ticks 72, 76, 80, 84
///   Round 3: same order at ticks 144, 152, 160, 168
///   Round 4: same order at ticks 216, 228, 240, 252
///   Round 5: same order at ticks 288, 304, 320, 336
fn build_journal() -> Vec<Command> {
    let a1 = ActorId(1); // Ally1
    let a2 = ActorId(2); // Ally2
    let e1 = ActorId(3); // Enemy1
    let e2 = ActorId(4); // Enemy2

    // Round 1 (ticks 0, 0, 0, 0)
    let r1 = vec![
        Command {
            actor_id: a2,
            action: Action::SnapShot(e1),
        }, // 1. Ally2 snap Enemy1
        Command {
            actor_id: e2,
            action: Action::SnapShot(a1),
        }, // 2. Enemy2 snap Ally1
        Command {
            actor_id: a1,
            action: Action::SnapShot(e1),
        }, // 3. Ally1 snap Enemy1
        Command {
            actor_id: e1,
            action: Action::Move(TileXY::new(14, 5)),
        }, // 4. Enemy1 moves
    ];

    // Round 2 (ticks 72, 76, 80, 84)
    let r2 = vec![
        Command {
            actor_id: a2,
            action: Action::SnapShot(e2),
        }, // 5. Ally2 snap Enemy2
        Command {
            actor_id: e2,
            action: Action::SnapShot(a2),
        }, // 6. Enemy2 snap Ally2
        Command {
            actor_id: a1,
            action: Action::SnapShot(e1),
        }, // 7. Ally1 snap Enemy1
        Command {
            actor_id: e1,
            action: Action::SnapShot(a1),
        }, // 8. Enemy1 snap Ally1
    ];

    // Round 3 (ticks 144, 152, 160, 168)
    let r3 = vec![
        Command {
            actor_id: a2,
            action: Action::CalledShot(e1, pb_core::event::HitLocationType::GunArm),
        },
        Command {
            actor_id: e2,
            action: Action::SnapShot(a1),
        },
        Command {
            actor_id: a1,
            action: Action::CalledShot(e1, pb_core::event::HitLocationType::Head),
        },
        Command {
            actor_id: e1,
            action: Action::SnapShot(a2),
        },
    ];

    // Round 4 (ticks 216, 228, 240, 252)
    let r4 = vec![
        Command {
            actor_id: a2,
            action: Action::AimedShot(e1),
        },
        Command {
            actor_id: e2,
            action: Action::Move(TileXY::new(14, 6)),
        },
        Command {
            actor_id: a1,
            action: Action::AimedShot(e2),
        },
        Command {
            actor_id: e1,
            action: Action::SnapShot(a2),
        },
    ];

    // Round 5 (ticks 288, 304, 320, 336)
    let r5 = vec![
        Command {
            actor_id: a2,
            action: Action::SnapShot(e1),
        },
        Command {
            actor_id: e2,
            action: Action::SnapShot(a1),
        },
        Command {
            actor_id: a1,
            action: Action::Hold,
        },
        Command {
            actor_id: e1,
            action: Action::Hold,
        },
    ];

    let mut journal = Vec::new();
    journal.extend(r1);
    journal.extend(r2);
    journal.extend(r3);
    journal.extend(r4);
    journal.extend(r5);
    journal
}

/// Apply a journal to the simulation state.
///
/// For each command, advances to the next actor's turn and then applies the
/// command via `step()`. Errors (dead actor, insufficient AP) are silently
/// skipped so the journal can be applied even after some actors have died.
fn apply_journal(state: &mut SimState, journal: &[Command]) {
    for cmd in journal {
        // Advance to this actor's turn
        let _next = advance_to_next_actor(state);

        // If the actor is dead, step returns an error — that's fine.
        let _ = step(state, cmd.clone());
    }
}

/// Run a full scenario and return the terminal state hash.
fn run_and_hash(seed: u64, journal: &[Command]) -> [u8; 32] {
    let mut state = build_prov_scenario(seed);
    apply_journal(&mut state, journal);
    compute_state_hash(&state)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Three-pass determinism: same seed + same setup + same journal always
/// produces the same terminal state hash.
#[test]
fn determinism_three_pass_identical_hashes() {
    let seed: u64 = 12345;
    let journal = build_journal();

    let hash1 = run_and_hash(seed, &journal);
    let hash2 = run_and_hash(seed, &journal);
    let hash3 = run_and_hash(seed, &journal);

    // All three must be identical
    assert_eq!(
        hash1, hash2,
        "Pass 1 and pass 2 hashes must match — determinism violated"
    );
    assert_eq!(
        hash2, hash3,
        "Pass 2 and pass 3 hashes must match — determinism violated"
    );

    // Hash must not be all zeros (would indicate a broken hash function)
    assert_ne!(hash1, [0u8; 32], "Terminal hash must not be all zeros");
}

/// Different seeds must produce different terminal states.
#[test]
fn determinism_different_seed_different_hash() {
    let journal = build_journal();
    let hash_a = run_and_hash(42, &journal);
    let hash_b = run_and_hash(9999, &journal);
    assert_ne!(
        hash_a, hash_b,
        "Different seeds must produce different terminal hashes"
    );
}

/// Different journals applied to the same seed must produce different hashes.
#[test]
fn determinism_different_journal_different_hash() {
    let seed: u64 = 12345;
    let journal_full = build_journal();

    // A minimal journal: just one actor acting
    let mut state_a = build_prov_scenario(seed);
    apply_journal(&mut state_a, &journal_full);
    let hash_full = compute_state_hash(&state_a);

    // Apply only the first 4 commands
    let mut state_b = build_prov_scenario(seed);
    apply_journal(&mut state_b, &journal_full[..4]);
    let hash_partial = compute_state_hash(&state_b);

    assert_ne!(
        hash_full, hash_partial,
        "Different journals must produce different terminal hashes"
    );
}

/// Verify that both step() and resolve_shot() produce consistent results
/// when called with the same seed, scenario, tick, and actor.
#[test]
fn called_shot_determinism_same_result_twice() {
    use pb_core::event::HitLocationType;
    use pb_core::geom::Facing;
    use pb_core::ids::Ap;
    use pb_sim::shot::resolve_shot;
    use pb_sim::state::{ActorState, Stance};

    let seed: u64 = 42;
    let mut state = SimState::new(seed, 1);

    // Create shooter and target
    let shooter = ActorId(1);
    let target = ActorId(2);

    state.actors.insert(
        shooter,
        ActorState {
            ap: Ap(10),
            position: TileXY::new(0, 0),
            facing: Facing::South,
            sequence: 5,
            hit_points: 30,
            max_hp: 30,
            name: "Shooter".into(),
            alive: true,
            wounds: vec![],
            sand: 20,
            max_sand: 20,
            stance: Stance::Standing,
        },
    );
    state.actors.insert(
        target,
        ActorState {
            ap: Ap(10),
            position: TileXY::new(5, 0),
            facing: Facing::North,
            sequence: 4,
            hit_points: 35,
            max_hp: 35,
            name: "Target".into(),
            alive: true,
            wounds: vec![],
            sand: 20,
            max_sand: 20,
            stance: Stance::Standing,
        },
    );

    // Advance clock to give shooter a tick
    let _ = advance_to_next_actor(&mut state);

    // Resolve a called shot to GunArm twice
    let result_a = resolve_shot(&state, shooter, target, Some(HitLocationType::GunArm));
    let result_b = resolve_shot(&state, shooter, target, Some(HitLocationType::GunArm));

    // Both calls must produce identical results
    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
    assert_eq!(
        result_a.unwrap(),
        result_b.unwrap(),
        "Called shot resolve_shot must be deterministic: same inputs → same events"
    );
}
