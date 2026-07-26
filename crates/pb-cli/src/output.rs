//! Sentinel constants for pbcli output formatting.
//! Each constant provides a canonical prefix string.

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

/// Worst AI turn time prefix.
pub const WORST_AI_TURN: &str = "worst-ai-turn-ms: ";

/// Worst sim step time prefix.
pub const WORST_SIM_STEP: &str = "worst-sim-step-ms: ";
