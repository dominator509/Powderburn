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
            .map_or("unknown", |actor| actor.name.as_str())
    };

    match event {
        Event::TurnBegin { actor, tick } => format!("event: TurnBegin actor={} tick={tick}", name(actor)),
        Event::TurnEnd { actor, tick } => format!("event: TurnEnd actor={} tick={tick}", name(actor)),
        Event::Moved { actor, from, to } => {
            format!("event: Moved actor={} from={from} to={to}", name(actor))
        }
        Event::StanceChanged { actor, stance } => {
            format!("event: StanceChanged actor={} stance={stance}", name(actor))
        }
        Event::FacingChanged { actor, facing } => {
            format!("event: FacingChanged actor={} facing={facing}", name(actor))
        }
        Event::Fired { actor, target } => {
            format!("event: Fired actor={} target={}", name(actor), name(target))
        }
        Event::Misfire { actor } => format!("event: Misfire actor={}", name(actor)),
        Event::Jammed { actor } => format!("event: Jammed actor={}", name(actor)),
        Event::Missed { actor, target } => {
            format!("event: Missed actor={} target={}", name(actor), name(target))
        }
        Event::ShotHit { actor, target, hit } => format!(
            "event: ShotHit actor={} target={} hit={hit}",
            name(actor),
            name(target)
        ),
        Event::HitLocation { actor, location } => {
            format!("event: HitLocation actor={} location={location}", name(actor))
        }
        Event::DamageApplied { actor, damage } => {
            format!("event: DamageApplied actor={} damage={damage}", name(actor))
        }
        Event::WoundApplied { actor, wound } => {
            format!("event: WoundApplied actor={} wound={wound}", name(actor))
        }
        Event::Critical { actor, effect } => {
            format!("event: Critical actor={} effect={effect}", name(actor))
        }
        Event::WeaponDropped { actor, item } => {
            format!("event: WeaponDropped actor={} item={item}", name(actor))
        }
        Event::SmokeDeposited { tile, density } => {
            format!("event: SmokeDeposited tile={tile} density={density}")
        }
        Event::SmokeDecayed { tile, density } => {
            format!("event: SmokeDecayed tile={tile} density={density}")
        }
        Event::SmokeDrifted { from, to, density } => {
            format!("event: SmokeDrifted from={from} to={to} density={density}")
        }
        Event::OverwatchSet {
            actor,
            reaction_points,
        } => format!(
            "event: OverwatchSet actor={} reaction_points={reaction_points}",
            name(actor)
        ),
        Event::ReactionShot { actor, target } => format!(
            "event: ReactionShot actor={} target={}",
            name(actor),
            name(target)
        ),
        Event::DynamiteLit {
            actor,
            tile,
            detonate_at,
        } => format!(
            "event: DynamiteLit actor={} tile={tile} detonate_at={detonate_at}",
            name(actor)
        ),
        Event::DynamiteCaught { actor, tile } => {
            format!("event: DynamiteCaught actor={} tile={tile}", name(actor))
        }
        Event::DynamiteRethrown {
            actor,
            tile,
            detonate_at,
        } => format!(
            "event: DynamiteRethrown actor={} tile={tile} detonate_at={detonate_at}",
            name(actor)
        ),
        Event::DynamiteExploded { actor, tile } => {
            format!("event: DynamiteExploded actor={} tile={tile}", name(actor))
        }
        Event::CoverDamaged {
            tile,
            facing,
            level,
        } => format!("event: CoverDamaged tile={tile} facing={facing} level={level}"),
        Event::Revealed { actor, until_tick } => format!(
            "event: Revealed actor={} until_tick={until_tick}",
            name(actor)
        ),
        Event::TrackLeft { actor, tile } => {
            format!("event: TrackLeft actor={} tile={tile}", name(actor))
        }
        Event::SandLost { actor, amount } => {
            format!("event: SandLost actor={} amount={amount}", name(actor))
        }
        Event::SandGained { actor, amount } => {
            format!("event: SandGained actor={} amount={amount}", name(actor))
        }
        Event::MoraleStateChanged { actor, state } => {
            format!("event: MoraleStateChanged actor={} state={state}", name(actor))
        }
        Event::Routed { actor } => format!("event: Routed actor={}", name(actor)),
        Event::ActorKilled { actor } => format!("event: ActorKilled actor={}", name(actor)),
        Event::CompanionKilled { id } => format!("event: CompanionKilled id={id}"),
        Event::ObjectiveComplete { id } => format!("event: ObjectiveComplete id={id}"),
        Event::LedgerEntryWritten { index } => format!("event: LedgerEntryWritten index={index}"),
        Event::ScenarioEnded { outcome } => format!("event: ScenarioEnded outcome={outcome}"),
        Event::XpGained {
            actor,
            xp,
            total_xp,
            new_level,
        } => format!(
            "event: XpGained actor={} xp={xp} total_xp={total_xp} new_level={new_level:?}",
            name(actor)
        ),
        Event::LevelUp {
            actor,
            new_level,
            skill_points_granted,
            marks_granted,
        } => format!(
            "event: LevelUp actor={} new_level={new_level} sp={skill_points_granted} marks={marks_granted}",
            name(actor)
        ),
        Event::MarkGained {
            actor,
            mark_id,
            level,
        } => format!("event: MarkGained actor={} mark={mark_id} level={level}", name(actor)),
        Event::SkillPointSpent {
            actor,
            skill,
            new_level,
        } => format!(
            "event: SkillPointSpent actor={} skill={skill} new_level={new_level}",
            name(actor)
        ),
    }
}
