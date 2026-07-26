# SPEC-003 API Contracts

Layer L2. The command surface and the crate boundaries. Names here are locked.

## 1. pbcli command surface

Every subcommand is non-interactive, reads no stdin unless told to, writes machine-greppable lines to
stdout, and writes diagnostics to stderr. Exit code 0 means the sentinel was printed.

| Command | Required flags | Optional flags | Output sentinels |
| --- | --- | --- | --- |
| `sim` | `--scenario <path>` `--seed <u64>` | `--journal <path>` `--emit-hash` `--emit-events` `--suspend-at-tick <u64>` `--save <path>` `--resume <path>` `--trace-actor <id>` | `state-hash: <64hex>`, `resumed-hash: <64hex>`, `chain: intact`, `event: ...` |
| `replay` | `--journal <path>` | `--expect <64hex>` `--diff <path>` | `replay: match` or `replay: differ at tick <n>` |
| `capture` | `--scenario <path>` `--seed <u64>` `--out <path>` | `--adapter <gl or vulkan>` `--tick <u64>` | `capture: ok <64hex>` |
| `campaign new` | `--save <path>` | `--company-seed <u64>` | `campaign: created` |
| `campaign play` | `--save <path>` | `--mission <id>` `--journal <path>` `--script <path>` `--from-new` `--emit-outcome` `--emit-manifest` | `outcome: VICTORY or DEFEAT or WITHDRAWN`, `ledger-entries: <n>`, `available: <mission_id>` |
| `campaign audit` | `--save <path>` | `--dangling-refs` | `dangling-refs: <n>`, `ledger-entry: <id> present` |
| `bench turn` | `--scenario <path>` `--seed <u64>` | `--iterations <n>` `--emit-budget` | `worst-ai-turn-ms: <n>`, `worst-sim-step-ms: <n>` |
| `selftest` | none | `--emit-hash` `--emit-metrics` | `selftest: ok`, `state-hash: <64hex>`, `metric: <name> <value>` |
| `a11y-report` | none | none | `a11y: ok` or `a11y: FAIL <check>` |
| `serve-replay` | `--port <n>` | none | binds 127.0.0.1 only, feature `replay-server`, never in a release build |

## 2. pbtool command surface

| Command | Output sentinel |
| --- | --- |
| `validate content` | `pbtool validate: ok` |
| `validate content --with-fixture <path>` | non-zero exit and an `E-` error code on a violating fixture |
| `validate representation` | `representation: ok` |
| `validate provenance --asset-root <path>` | `provenance: ok` |
| `golden refresh` | `pbtool golden: ok` |
| `image stats <path>` | `unique-colors: <n>`, `dimensions: <w>x<h>` |
| `atlas pack <dir>` | `atlas: ok <count>` |

`golden refresh` is the only command that writes into `$PB_GOLDEN_DIR`. It refuses to run unless the
working tree is clean, so a golden can never be silently regenerated to hide a determinism break.

## 3. Journal format (the replay contract)

A journal is a UTF-8 text file, one command per line, no trailing whitespace:

    <tick> <actor_id> <Command> <arg>=<value> ...

Commands: `Move`, `Face`, `Stance`, `Snap`, `Aimed`, `Called`, `Fan`, `Reload`, `ClearJam`,
`DrawBead`, `Bandage`, `Rally`, `Throw`, `Melee`, `Loot`, `UseItem`, `Hold`, `EndTurn`.

The journal is the complete input to the simulation. LBI-04: every state mutation originates from a
journal command or from a deterministic consequence of one. Replaying a journal against the same
ruleset, content, and seed reproduces the terminal state hash exactly. A journal line that is illegal
at its tick is a hard error, `E-JOURNAL-ILLEGAL`, not a skipped line.

## 4. Crate API contracts

    // pb-sim
    pub fn step(state: &mut SimState, cmd: Command) -> Result<Vec<Event>, SimError>;
    pub fn advance_to_next_actor(state: &mut SimState) -> Option<ActorId>;
    pub fn state_hash(state: &SimState) -> [u8; 32];

    // pb-rng
    pub fn draw(seed: u64, scenario: u32, tick: u64, actor: u32, stream: StreamTag, lo: i32, hi: i32) -> i32;

    // pb-content
    pub fn load_all(root: &Path) -> Result<Content, ContentError>;
    pub fn validate(content: &Content) -> Vec<Diagnostic>;

    // pb-save
    pub fn write(path: &Path, save: &SaveFile) -> Result<(), SaveError>;
    pub fn read(path: &Path, ruleset_hash: Hash32, content_hash: Hash32) -> Result<SaveFile, SaveError>;

    // pb-render
    pub fn capture_frame(state: &SimState, content: &Content, adapter: Backend, tick: u64) -> Result<Rgba8Image, RenderError>;

`step` is total: it either applies the command completely and returns its events, or applies nothing
and returns an error. There is no partial application. This is what makes save-resume exact.

## 5. Stability rules

These signatures are frozen from EP-004 onward. Changing one requires a spec update with repository
evidence, an ADR, and an update to every ExecPlan that transcribes it. Adding a new subcommand
requires adding its sentinel to this table and to COMMANDS.md in the same milestone.
