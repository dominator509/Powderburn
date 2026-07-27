//! Sentinel constants for pbcli output formatting.
//! Each constant provides a canonical prefix string.

use pb_core::event::Event;
use pb_sim::state::SimState;

/// Prefix for state hash output.
pub const STATE_HASH_FORMAT: &str = "state-hash: ";

/// Replay match sentinel.
pub const REPLAY_MATCH: &str = "replay: match";

/// Replay differ sentinel (followed by tick number).
pub const REPLAY_DIFFER: &str = "replay: differ at tick ";

/// Campaign created sentinel.
pub const CAMPAIGN_CREATED: &str = "campaign: created";

/// Outcome victory sentinel.
pub const OUTCOME_VICTORY: &str = "outcome: VICTORY";

/// Outcome defeat sentinel.
pub const OUTCOME_DEFEAT: &str = "outcome: DEFEAT";

/// Ledger entries count prefix.
pub const LEDGER_ENTRIES: &str = "ledger-entries: ";

/// Chain intact sentinel.
pub const CHAIN_INTACT: &str = "chain: intact";

/// Dangling references prefix.
pub const DANGLING_REFS: &str = "dangling-refs: ";

/// Ledger entry prefix.
pub const LEDGER_ENTRY_PREFIX: &str = "ledger-entry: ";

/// Available prefix.
pub const AVAILABLE_PREFIX: &str = "available: ";

/// Capture ok sentinel.
pub const CAPTURE_OK: &str = "capture: ok ";

/// Selftest ok sentinel.
pub const SELFTEST_OK: &str = "selftest: ok";

/// Validate ok sentinel for pbtool.
pub const PBM_VALIDATE_OK: &str = "pbtool validate: ok";

/// Representation ok sentinel.
pub const REPRESENTATION_OK: &str = "representation: ok";

/// Provenance ok sentinel.
pub const PROVENANCE_OK: &str = "provenance: ok";

/// Golden ok sentinel.
pub const GOLDEN_OK: &str = "pbtool golden: ok";

/// Unique colors prefix.
pub const UNIQUE_COLORS: &str = "unique-colors: ";

/// Dimensions prefix.
pub const DIMENSIONS: &str = "dimensions: ";

/// Atlas ok sentinel.
pub const ATLAS_OK: &str = "atlas: ok ";

/// Resumed hash prefix.
pub const RESUMED_HASH_FORMAT: &str = "resumed-hash: ";

/// Worst AI turn time prefix.
pub const WORST_AI_TURN: &str = "worst-ai-turn-ms: ";

/// Worst sim step time prefix.
pub const WORST_SIM_STEP: &str = "worst-sim-step-ms: ";

/// P95 frame render time prefix.
pub const P95_FRAME_MS: &str = "p95-frame-ms: ";

/// Format an event using string names from simulation state.
///
/// Looks up actor IDs in `state.actors` to produce human-readable names
/// (e.g. `actor=e_bandit_02` instead of `actor=ActorId(42)`).
pub fn format_event(event: &Event, state: &SimState) -> String {
    let name = |actor_id| {
        state
            .actors
            .get(actor_id)
            .map(|a| a.name.as_str())
            .unwrap_or("unknown")
    };

    match event {
        Event::HitLocation { actor, location } => {
            format!(
                "event: HitLocation actor={} location={}",
                name(actor),
                location
            )
        }
        Event::DamageApplied { actor, damage } => {
            format!(
                "event: DamageApplied actor={} damage={}",
                name(actor),
                damage
            )
        }
        Event::WoundApplied { actor, wound } => {
            format!("event: WoundApplied actor={} wound={}", name(actor), wound)
        }
        Event::WeaponDropped { actor, item } => {
            format!("event: WeaponDropped actor={} item={}", name(actor), item)
        }
        Event::Misfire { actor } => {
            format!("event: Misfire actor={}", name(actor))
        }
        Event::ShotHit { actor, target, hit } => {
            format!(
                "event: ShotHit actor={} target={} hit={}",
                name(actor),
                name(target),
                hit
            )
        }
        Event::ActorKilled { actor } => {
            format!("event: ActorKilled actor={}", name(actor))
        }
        Event::SmokeDeposited { tile, density } => {
            format!("event: SmokeDeposited tile={} density={}", tile, density)
        }
        Event::CompanionKilled { id } => {
            format!("event: CompanionKilled id={}", id)
        }
        Event::XpGained {
            actor,
            xp,
            total_xp,
            new_level,
        } => {
            format!(
                "event: XpGained actor={} xp={} total_xp={} new_level={:?}",
                name(actor),
                xp,
                total_xp,
                new_level
            )
        }
        Event::LevelUp {
            actor,
            new_level,
            skill_points_granted,
            marks_granted,
        } => {
            format!(
                "event: LevelUp actor={} new_level={} sp={} marks={}",
                name(actor),
                new_level,
                skill_points_granted,
                marks_granted
            )
        }
        Event::MarkGained {
            actor,
            mark_id,
            level,
        } => {
            format!(
                "event: MarkGained actor={} mark={} level={}",
                name(actor),
                mark_id,
                level
            )
        }
        Event::SkillPointSpent {
            actor,
            skill,
            new_level,
        } => {
            format!(
                "event: SkillPointSpent actor={} skill={} new_level={}",
                name(actor),
                skill,
                new_level
            )
        }
    }
}
