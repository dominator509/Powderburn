//! Integration tests for pb-ai utility AI.
//!
//! M5: Tests that bounded candidate generation, integer scoring, and
//! deterministic tie-breaking all work correctly.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unnecessary_cast,
    clippy::identity_op,
    unused_imports
)]

use pb_ai::utility::{generate_candidates, score_candidate, select_best, AiCandidate};
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::{ActorId, Ap};
use pb_sim::action::Action;
use pb_sim::state::{ActorState, Stance};

/// Helper to create an actor at a given position.
fn make_actor(x: i16, y: i16, hp: i32, ap: i16, alive: bool, name: &str) -> ActorState {
    ActorState {
        ap: Ap(ap),
        position: TileXY::new(x, y),
        facing: Facing::South,
        sequence: 5,
        hit_points: hp,
        max_hp: 20,
        name: name.to_string(),
        alive,
        wounds: vec![],
        sand: 10,
        max_sand: 10,
        stance: Stance::Standing,
    }
}

/// Test that ties break by candidate index (lower index wins).
#[test]
fn ties_break_by_index() {
    let candidates = vec![
        AiCandidate {
            action: Action::Hold,
            score: 100,
            target: None,
            destination: None,
        },
        AiCandidate {
            action: Action::Reload,
            score: 100,
            target: None,
            destination: None,
        },
        AiCandidate {
            action: Action::UseItem,
            score: 100,
            target: None,
            destination: None,
        },
    ];

    let best = select_best(candidates).expect("should have a best candidate");
    // All three have the same score (100); tie-break by index → first (Hold) wins.
    assert_eq!(
        best.action,
        Action::Hold,
        "tie-break by index should pick the first candidate"
    );
}

/// Test that the AI picks shooting over holding when enemies exist.
#[test]
fn ai_picks_shooting_over_holding() {
    let actor = make_actor(0, 0, 20, 10, true, "Player");
    let allies = vec![];
    let enemies = vec![make_actor(5, 0, 10, 10, true, "Enemy1")];

    // With enemies nearby, a shooting action should score higher than Hold.
    let candidates = generate_candidates(&actor, &allies, &enemies);

    let scored: Vec<AiCandidate> = candidates
        .into_iter()
        .map(|c| {
            let s = score_candidate(&c, &actor, &allies, &enemies);
            AiCandidate { score: s, ..c }
        })
        .collect();

    let best = select_best(scored).expect("should have candidates");
    assert!(
        matches!(
            best.action,
            Action::SnapShot(_)
                | Action::AimedShot(_)
                | Action::CalledShot(_, _)
                | Action::Melee(_)
        ),
        "expected a combat action when enemies are present, but got {:?}",
        best.action
    );
}

/// Test that enemies generate bounded candidates.
///
/// At most 24 movement candidates + at most 6 target enemies (each generating
/// up to 4 action candidates) + 3 generic actions (Reload, UseItem, Hold).
#[test]
fn bounded_candidate_generation() {
    let actor = make_actor(0, 0, 20, 10, true, "Player");
    let allies = vec![make_actor(1, 1, 20, 10, true, "Ally1")];

    // More than MAX_TARGET_CANDIDATES enemies at varying distances
    let enemies: Vec<ActorState> = (0..10)
        .map(|i| {
            make_actor(
                3 + i as i16,
                0 + (i as i16 / 2),
                15 - i as i32,
                10,
                true,
                &format!("Enemy{}", i),
            )
        })
        .collect();

    let candidates = generate_candidates(&actor, &allies, &enemies);

    // Count movement candidates
    let move_count = candidates
        .iter()
        .filter(|c| matches!(c.action, Action::Move(_)))
        .count();
    assert!(move_count <= 24, "movement candidates: {} > 24", move_count);

    // Count target-based action candidates
    let target_action_count = candidates
        .iter()
        .filter(|c| {
            matches!(
                c.action,
                Action::SnapShot(_)
                    | Action::AimedShot(_)
                    | Action::CalledShot(_, _)
                    | Action::Melee(_)
            )
        })
        .count();
    // At most 6 targets × 4 actions = 24
    assert!(
        target_action_count <= 24,
        "target action candidates: {} > 24",
        target_action_count
    );

    // Count generic actions
    let generic_count = candidates
        .iter()
        .filter(|c| matches!(c.action, Action::Reload | Action::UseItem | Action::Hold))
        .count();
    assert_eq!(generic_count, 3, "should have exactly 3 generic actions");
}

/// Test that generate_candidates returns non-empty even with no enemies.
#[test]
fn generate_candidates_empty_enemies() {
    let actor = make_actor(0, 0, 20, 10, true, "Player");
    let allies = vec![];
    let enemies: Vec<ActorState> = vec![];

    let candidates = generate_candidates(&actor, &allies, &enemies);
    assert!(
        !candidates.is_empty(),
        "should still generate movement and generic candidates"
    );

    // Should include movement candidates and generic actions
    let has_move = candidates
        .iter()
        .any(|c| matches!(c.action, Action::Move(_)));
    let has_hold = candidates.iter().any(|c| c.action == Action::Hold);
    assert!(has_move, "should include movement candidates");
    assert!(has_hold, "should include Hold");
}

/// Test that scoring is deterministic (same inputs → same score).
#[test]
fn scoring_is_deterministic() {
    let actor = make_actor(0, 0, 20, 10, true, "Player");
    let allies = vec![];
    let enemies = vec![make_actor(5, 0, 10, 10, true, "Enemy1")];

    let candidate = AiCandidate {
        action: Action::SnapShot(ActorId(1)),
        score: 0,
        target: Some(ActorId(1)),
        destination: None,
    };

    let score1 = score_candidate(&candidate, &actor, &allies, &enemies);
    let score2 = score_candidate(&candidate, &actor, &allies, &enemies);
    assert_eq!(score1, score2, "scoring must be deterministic");
}
