//! Utility AI for enemy/squad behavior.
//!
//! M5: Bounded candidate generation, integer scoring, deterministic tie-breaking.
//! All scoring uses only integer arithmetic.  No floats, no unordered maps.

use std::collections::BTreeSet;

use pb_core::event::HitLocationType;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_rules::tables::weapon_entry;
use pb_sim::action::{Action, Command};
use pb_sim::state::ActorState;

// ---------------------------------------------------------------------------
// AiCandidate
// ---------------------------------------------------------------------------

/// A single candidate action that the AI is evaluating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCandidate {
    /// The action to perform.
    pub action: Action,
    /// The computed utility score (populated by `score_candidate`).
    pub score: i32,
    /// Optional actor target (for shots, melee, bandage).
    pub target: Option<ActorId>,
    /// Optional destination tile (for movement).
    pub destination: Option<TileXY>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Half-width of the map boundary in tiles (±MAP_BOUND).
const MAP_BOUND: i16 = 100;

/// Maximum movement candidates to generate.
const MAX_MOVEMENT_CANDIDATES: usize = 24;

/// Maximum target candidates to generate.
const MAX_TARGET_CANDIDATES: usize = 6;

/// Scoring weights.
const W_DAMAGE: i32 = 10;
const W_COVER: i32 = 8;
const W_FLANK: i32 = 12;
const W_SAND_RISK: i32 = -15;
const W_DISTANCE: i32 = -3;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a bounded set of candidate actions for `actor`.
///
/// - At most 24 movement positions are sampled from Chebyshev-distance rings
///   1, 2, and 3 around the actor.
/// - At most 6 enemy targets are selected (nearest first, then lowest HP).
/// - For each target: SnapShot, AimedShot, CalledShot (Torso), Melee – up to
///   24 target-action candidates.
/// - Generic actions: Reload, UseItem, Hold.
pub fn generate_candidates(
    actor: &ActorState,
    allies: &[ActorState],
    enemies: &[ActorState],
) -> Vec<AiCandidate> {
    let mut candidates: Vec<AiCandidate> = Vec::new();

    // -- Movement candidates (at most 24) --
    let positions = generate_movement_positions(actor, allies);
    for pos in positions.iter().take(MAX_MOVEMENT_CANDIDATES) {
        candidates.push(AiCandidate {
            action: Action::Move(*pos),
            score: 0,
            target: None,
            destination: Some(*pos),
        });
    }

    // -- Target-based candidates (at most 6 targets × up to 4 actions each) --
    let targets = select_targets(actor, enemies, MAX_TARGET_CANDIDATES);
    for &(enemy_id, _position) in &targets {
        // SnapShot
        candidates.push(AiCandidate {
            action: Action::SnapShot(enemy_id),
            score: 0,
            target: Some(enemy_id),
            destination: None,
        });
        // AimedShot
        candidates.push(AiCandidate {
            action: Action::AimedShot(enemy_id),
            score: 0,
            target: Some(enemy_id),
            destination: None,
        });
        // CalledShot (default to Torso – highest base chance)
        candidates.push(AiCandidate {
            action: Action::CalledShot(enemy_id, HitLocationType::Torso),
            score: 0,
            target: Some(enemy_id),
            destination: None,
        });
        // Melee
        candidates.push(AiCandidate {
            action: Action::Melee(enemy_id),
            score: 0,
            target: Some(enemy_id),
            destination: None,
        });
    }

    // -- Generic action candidates --
    candidates.push(AiCandidate {
        action: Action::Reload,
        score: 0,
        target: None,
        destination: None,
    });
    candidates.push(AiCandidate {
        action: Action::UseItem,
        score: 0,
        target: None,
        destination: None,
    });
    candidates.push(AiCandidate {
        action: Action::Hold,
        score: 0,
        target: None,
        destination: None,
    });

    candidates
}

/// Score a single candidate using a weighted integer sum.
///
/// Weights:
/// - expected_damage × 10
/// - cover_gained × 8
/// - flanking_gained × 12
/// - sand_risk × −15
/// - distance_to_objective × −3
pub fn score_candidate(
    candidate: &AiCandidate,
    actor: &ActorState,
    _allies: &[ActorState],
    enemies: &[ActorState],
) -> i32 {
    let dmg = compute_expected_damage(candidate, actor, enemies);
    let cover = compute_cover_gained(candidate, actor, enemies);
    let flank = compute_flanking_gained(candidate, actor, enemies);
    let sand = compute_sand_risk(candidate, actor, enemies);
    let dist = compute_distance_to_objective(candidate, actor, enemies);

    dmg * W_DAMAGE + cover * W_COVER + flank * W_FLANK + sand * W_SAND_RISK + dist * W_DISTANCE
}

/// Select the best-scoring candidate.
///
/// Ties are broken by *index in the input vector* (lower index = higher
/// priority).  This is deterministic because iteration order is insertion
/// order.
pub fn select_best(candidates: Vec<AiCandidate>) -> Option<AiCandidate> {
    candidates
        .into_iter()
        .enumerate()
        .max_by_key(|&(idx, ref c)| (c.score, -(idx as i32)))
        .map(|(_idx, c)| c)
}

/// Convenience: generate, score, select, return a `Command`.
///
/// If no candidates are available, returns `Command { action: Action::Hold,
/// actor_id: ??? }` – see `decide_action` below.  **Note**: the caller must
/// supply an `ActorId` for the final `Command`.  The signature below uses a
/// **default ActorId(0)** as a placeholder because the actor is identified by
/// `ActorState` not by ID.  Production callers should replace it.
///
/// If `candidates` is empty, returns `Hold`.
pub fn decide_action(actor: &ActorState, allies: &[ActorState], enemies: &[ActorState]) -> Command {
    // We need an ActorId for the Command.  Since ActorState doesn't carry one,
    // we use ActorId(0) as a formal placeholder.  Callers with a SimState
    // should look up the real ID.
    let actor_id = ActorId(0);

    let candidates = generate_candidates(actor, allies, enemies);
    if candidates.is_empty() {
        return Command {
            actor_id,
            action: Action::Hold,
        };
    }

    let scored: Vec<AiCandidate> = candidates
        .into_iter()
        .map(|c| {
            let s = score_candidate(&c, actor, allies, enemies);
            AiCandidate { score: s, ..c }
        })
        .collect();

    select_best(scored).map_or(
        Command {
            actor_id,
            action: Action::Hold,
        },
        |best| Command {
            actor_id,
            action: best.action,
        },
    )
}

// ---------------------------------------------------------------------------
// Movement candidate generation
// ---------------------------------------------------------------------------

/// Generate tile positions at Chebyshev distances 1, 2, 3 from the actor.
///
/// Positions are emitted in a deterministic order (rings, then dx, then dy).
/// Occupied (ally) tiles are filtered out.  The caller caps at
/// `MAX_MOVEMENT_CANDIDATES`.
fn generate_movement_positions(actor: &ActorState, allies: &[ActorState]) -> Vec<TileXY> {
    let pos = actor.position;

    // Collect ally positions for occupancy check
    let ally_positions: BTreeSet<TileXY> = allies.iter().map(|a| a.position).collect();

    let mut result = Vec::new();

    // Rings at Chebyshev distances 1, 2, 3
    for d in 1i16..=3i16 {
        // All tiles where max(|dx|, |dy|) == d
        for dx in -d..=d {
            for dy in -d..=d {
                if dx.abs() > d || dy.abs() > d {
                    continue;
                }
                if dx.abs().max(dy.abs()) != d {
                    continue;
                }
                let tile = TileXY::new(pos.x.wrapping_add(dx), pos.y.wrapping_add(dy));

                // Bounds check
                if tile.x.abs() > MAP_BOUND || tile.y.abs() > MAP_BOUND {
                    continue;
                }
                // Not occupied by an ally
                if ally_positions.contains(&tile) {
                    continue;
                }
                // Not the actor's current tile
                if tile == pos {
                    continue;
                }
                result.push(tile);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Target selection
// ---------------------------------------------------------------------------

/// Select up to `max` enemy targets, sorted by Chebyshev distance then HP.
///
/// Returns `(ActorId, TileXY)` so the caller has both the ID and position.
/// The `ActorId` is derived from the enemy's index in the slice (1-based).
fn select_targets(
    actor: &ActorState,
    enemies: &[ActorState],
    max: usize,
) -> Vec<(ActorId, TileXY)> {
    let mut indexed: Vec<(ActorId, &ActorState)> = enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive)
        .map(|(i, e)| (ActorId((i + 1) as u32), e))
        .collect();

    // Sort: nearest first, then lowest HP.
    indexed.sort_by_key(|&(_, e)| {
        let dist = actor.position.chebyshev_distance(e.position);
        (dist, e.hit_points)
    });

    indexed
        .into_iter()
        .take(max)
        .map(|(id, e)| (id, e.position))
        .collect()
}

// ---------------------------------------------------------------------------
// Scoring components
// ---------------------------------------------------------------------------

/// Look up a default weapon's base damage for range calculations.
///
/// Uses the first weapon entry (Colt Army 1860) as the default sidearm.
fn default_base_damage() -> i32 {
    weapon_entry("colt_army_1860").map_or(10, |w| w.base_damage)
}

/// Look up a default weapon's max range.
fn default_max_range() -> i32 {
    weapon_entry("colt_army_1860").map_or(24, |w| w.max_range)
}

/// Compute expected damage for a candidate.
///
/// - Shooting actions: (base_damage * 100) / max(1, distance)
/// - Melee: flat 8 damage
/// - Others: 0
fn compute_expected_damage(
    candidate: &AiCandidate,
    actor: &ActorState,
    enemies: &[ActorState],
) -> i32 {
    match candidate.action {
        Action::SnapShot(_) | Action::AimedShot(_) | Action::CalledShot(_, _) => {
            let target_id = match candidate.target {
                Some(id) => id,
                None => return 0,
            };
            let dist = distance_to_enemy_by_id(target_id, actor, enemies);
            let base = default_base_damage();
            let range_penalty = core::cmp::max(1_i16, dist);
            // Returns base_damage adjusted for range (integer, no floats)
            (base * 100) / (range_penalty as i32)
        }
        Action::Melee(_) => {
            // Melee does moderate damage at close range
            let target_id = match candidate.target {
                Some(id) => id,
                None => return 0,
            };
            let dist = distance_to_enemy_by_id(target_id, actor, enemies);
            if dist <= 2 {
                8
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Compute cover gained: simple heuristic based on distance to nearest enemy.
/// Movement to a position farther from enemies gives better cover.
/// Returns 0..5.
fn compute_cover_gained(
    candidate: &AiCandidate,
    actor: &ActorState,
    enemies: &[ActorState],
) -> i32 {
    match candidate.action {
        Action::Move(dest) => {
            let current_min_dist = min_enemy_distance(actor.position, enemies);
            let dest_min_dist = min_enemy_distance(dest, enemies);
            let gain = dest_min_dist - current_min_dist;
            // Clamp to [0, 5]
            if gain > 0 {
                core::cmp::min(gain, 5_i16) as i32
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Compute flanking gained: does the candidate position give a flanking angle?
/// Simple heuristic: if the position is not on the same axis as the nearest
/// enemy, it's considered flanking (returns 1 if flanking, 0 otherwise).
fn compute_flanking_gained(
    candidate: &AiCandidate,
    actor: &ActorState,
    enemies: &[ActorState],
) -> i32 {
    match candidate.action {
        Action::Move(dest) => {
            let nearest = nearest_alive_enemy(dest, enemies);
            match nearest {
                Some((_, enemy_pos)) => {
                    let current_to_enemy = (
                        actor.position.x - enemy_pos.x,
                        actor.position.y - enemy_pos.y,
                    );
                    let dest_to_enemy = (dest.x - enemy_pos.x, dest.y - enemy_pos.y);
                    // Flanking if the sign of at least one axis flips
                    let flanking = (current_to_enemy.0.signum() != dest_to_enemy.0.signum()
                        && dest_to_enemy.0 != 0)
                        || (current_to_enemy.1.signum() != dest_to_enemy.1.signum()
                            && dest_to_enemy.1 != 0);
                    if flanking {
                        1
                    } else {
                        0
                    }
                }
                None => 0,
            }
        }
        _ => 0,
    }
}

/// Compute sand risk: how exposed is the actor at the candidate state.
/// Counts enemies within max range of the actor at the candidate position.
/// Returns 0..MAX_TARGET_CANDIDATES.
fn compute_sand_risk(candidate: &AiCandidate, actor: &ActorState, enemies: &[ActorState]) -> i32 {
    let pos = match candidate.action {
        Action::Move(dest) => dest,
        _ => actor.position,
    };
    let max_range = default_max_range();
    let count: i32 = enemies
        .iter()
        .filter(|e| e.alive)
        .filter(|e| pos.chebyshev_distance(e.position) as i32 <= max_range)
        .count() as i32;
    count
}

/// Compute distance to objective (nearest enemy).
/// Returns Chebyshev distance to nearest alive enemy, clamped to [0, 20].
fn compute_distance_to_objective(
    candidate: &AiCandidate,
    actor: &ActorState,
    enemies: &[ActorState],
) -> i32 {
    let pos = match candidate.action {
        Action::Move(dest) => dest,
        _ => actor.position,
    };
    let dist = min_enemy_distance(pos, enemies);
    core::cmp::min(dist as i32, 20)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Minimum Chebyshev distance from `pos` to any alive enemy.
fn min_enemy_distance(pos: TileXY, enemies: &[ActorState]) -> i16 {
    enemies
        .iter()
        .filter(|e| e.alive)
        .map(|e| pos.chebyshev_distance(e.position))
        .min()
        .unwrap_or(i16::MAX)
}

/// Find the nearest alive enemy and return their (ActorId, position).
///
/// Uses the same 1-based index convention as `select_targets`.
fn nearest_alive_enemy(pos: TileXY, enemies: &[ActorState]) -> Option<(ActorId, TileXY)> {
    enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive)
        .min_by_key(|(_, e)| pos.chebyshev_distance(e.position))
        .map(|(i, e)| (ActorId((i + 1) as u32), e.position))
}

/// Distance from actor to an enemy identified by ActorId.
///
/// The `ActorId` is assumed to be (index_in_slice + 1) as assigned by
/// `select_targets`.
fn distance_to_enemy_by_id(target_id: ActorId, actor: &ActorState, enemies: &[ActorState]) -> i16 {
    let idx = (target_id.0.saturating_sub(1)) as usize;
    enemies
        .get(idx)
        .map(|e| actor.position.chebyshev_distance(e.position))
        .unwrap_or(99)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unnecessary_cast,
    clippy::manual_range_contains
)]
mod tests {
    use super::*;
    use pb_core::geom::Facing;
    use pb_core::ids::Ap;
    use pb_sim::state::Stance;

    /// Helper to create an actor at a given position with the given id label
    /// in its name.
    fn make_actor(x: i16, y: i16, hp: i32, ap_val: i16, alive: bool, name: &str) -> ActorState {
        ActorState {
            ap: Ap(ap_val),
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
            progression: pb_sim::progression::ActorProgression::new(),
            weapon: String::new(),
            loaded_rounds: 0,
            weapon_capacity: 0,
            fouling: 0,
            jammed: false,
        }
    }

    #[test]
    fn bounded_movement_candidates() {
        let actor = make_actor(0, 0, 20, 10, true, "Hero");
        let allies = vec![make_actor(1, 0, 20, 10, true, "Ally1")];
        let enemies = vec![make_actor(10, 10, 20, 10, true, "Enemy1")];

        let candidates = generate_candidates(&actor, &allies, &enemies);

        // Count movement candidates
        let move_count = candidates
            .iter()
            .filter(|c| matches!(c.action, Action::Move(_)))
            .count();
        assert!(
            move_count <= MAX_MOVEMENT_CANDIDATES,
            "move count {} > max {}",
            move_count,
            MAX_MOVEMENT_CANDIDATES
        );
    }

    #[test]
    fn bounded_target_candidates() {
        let actor = make_actor(0, 0, 20, 10, true, "Hero");
        let allies = vec![];
        // Create more than MAX_TARGET_CANDIDATES enemies
        let enemies: Vec<ActorState> = (0..10)
            .map(|i| {
                make_actor(
                    5 + i as i16,
                    5 + i as i16,
                    10 + i as i32,
                    10,
                    true,
                    &format!("Enemy{}", i),
                )
            })
            .collect();

        let candidates = generate_candidates(&actor, &allies, &enemies);

        // Count target-based action candidates (SnapShot, AimedShot, CalledShot, Melee)
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

        // At most 6 targets × 4 actions = 24 target-based candidates
        assert!(
            target_action_count <= MAX_TARGET_CANDIDATES * 4,
            "target action count {} > max {}",
            target_action_count,
            MAX_TARGET_CANDIDATES * 4
        );
    }

    #[test]
    fn ai_picks_shooting_over_holding_when_enemies_exist() {
        let actor = make_actor(0, 0, 20, 10, true, "Hero");
        let allies = vec![];
        let enemies = vec![make_actor(3, 0, 10, 10, true, "Enemy1")];

        let cmd = decide_action(&actor, &allies, &enemies);
        // The AI should pick a shooting action (SnapShot, AimedShot, CalledShot) over Hold
        assert!(
            matches!(
                cmd.action,
                Action::SnapShot(_)
                    | Action::AimedShot(_)
                    | Action::CalledShot(_, _)
                    | Action::Melee(_)
            ),
            "expected a combat action but got {:?}",
            cmd.action
        );
    }

    #[test]
    fn ties_break_by_index() {
        let candidates = vec![
            AiCandidate {
                action: Action::Hold,
                score: 50,
                target: None,
                destination: None,
            },
            AiCandidate {
                action: Action::Reload,
                score: 50,
                target: None,
                destination: None,
            },
            AiCandidate {
                action: Action::UseItem,
                score: 50,
                target: None,
                destination: None,
            },
        ];

        let best = select_best(candidates).expect("should have a best candidate");
        // All have score 50, so tie-break by index → first (Hold) should win
        assert_eq!(best.action, Action::Hold);
    }

    #[test]
    fn select_best_returns_highest_score() {
        let candidates = vec![
            AiCandidate {
                action: Action::Hold,
                score: 10,
                target: None,
                destination: None,
            },
            AiCandidate {
                action: Action::Reload,
                score: 50,
                target: None,
                destination: None,
            },
            AiCandidate {
                action: Action::UseItem,
                score: 25,
                target: None,
                destination: None,
            },
        ];

        let best = select_best(candidates).expect("should have a best candidate");
        assert_eq!(best.action, Action::Reload);
    }

    #[test]
    fn select_best_returns_none_for_empty() {
        assert!(select_best(vec![]).is_none());
    }

    #[test]
    fn generate_candidates_includes_generic_actions() {
        let actor = make_actor(0, 0, 20, 10, true, "Hero");
        let allies = vec![];
        let enemies = vec![];

        let candidates = generate_candidates(&actor, &allies, &enemies);

        let has_reload = candidates.iter().any(|c| c.action == Action::Reload);
        let has_use_item = candidates.iter().any(|c| c.action == Action::UseItem);
        let has_hold = candidates.iter().any(|c| c.action == Action::Hold);

        assert!(has_reload, "should include Reload");
        assert!(has_use_item, "should include UseItem");
        assert!(has_hold, "should include Hold");
    }

    #[test]
    fn generate_candidates_movement_not_occupied() {
        let actor = make_actor(0, 0, 20, 10, true, "Hero");
        // Ally sitting at (1, 0) — that tile should not appear as a movement candidate
        let allies = vec![make_actor(1, 0, 20, 10, true, "Ally1")];
        let enemies = vec![make_actor(10, 10, 20, 10, true, "Enemy1")];

        let candidates = generate_candidates(&actor, &allies, &enemies);

        for c in &candidates {
            if let Action::Move(pos) = c.action {
                assert_ne!(
                    pos,
                    TileXY::new(1, 0),
                    "movement candidate should not land on ally tile"
                );
            }
        }
    }
}
