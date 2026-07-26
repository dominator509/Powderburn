NODE-META-BEGIN
ID: EP-003
DEPS: EP-002
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/test-integration.sh
VERIFY_SENTINEL: test-integration: ok
GREEN_TAG: green/EP-003
NODE-META-END

# EP-003 Data and Persistence

## 1. Purpose and Big Picture

Turn the kernel into a game by making content real. This node builds the RON schema and loader, the
validator that enforces historical immutability, representation law, anachronism, and permadeath
propagation, the campaign graph, the append-only hash-chained Ledger, and the save format with its
refusal codes. When it is done, the game's rules and story live in files a designer can diff, and the
validator refuses to let those files say something the project has decided they may not say.

## 2. Scope

pb-content schema, loader, validator, campaign graph, mod loading with path confinement. pb-save
format, Ledger chain, integrity refusal. The real `content/` tree: rule tables, the proving
scenarios, the twenty-four campaign nodes, the nine companion records, dialogue skeletons,
`PROVENANCE.toml`, `BIBLIOGRAPHY.md`, `lint/forbidden_tokens.txt`.

## 3. Non-goals

- No CLI surface. EP-004.
- No rendering of any content. EP-005.
- No full dialogue authoring; this node lands the schema, the nine companion records, and the Act I
  dialogue. Acts II through IV land in EP-007 as content, gated by the same validator.
- No balance passes.

## 4. Context and Orientation

Entry is `green/EP-002`, a kernel with a golden hash produced from test-constructed scenarios. The
central risk of this node is that loading content from files changes the state hash; M6 exists to
prove it does not. Binding invariants: LBI-05, LBI-06, LBI-07, LBI-08, LBI-11, LBI-13.

## 5. Files to Read First

    .agent/specs/SPEC-002-data-model.md
    .agent/specs/SPEC-000-product-scope.md sections 5 and 7
    .agent/specs/SPEC-005-auth-and-permissions.md
    .agent/specs/SPEC-006-error-handling.md
    crates/pb-sim/tests/determinism.rs

## 6. Expected Changed Files

    crates/pb-content/src/{lib,schema,load,validate,campaign,mods}.rs
    crates/pb-content/tests/{schema,history,representation,anachronism,permadeath,hashing,mod_sandbox}.rs
    crates/pb-save/src/{lib,format,load,write,ledger}.rs
    crates/pb-save/tests/integrity.rs
    content/rules/{weapons,items,marks,ways,factions,tables}.ron
    content/scenarios/{prov_called_shot,prov_full_battle,prov_sixty_actors,m01_elk_creek,m04_medicine_lodge}.ron
    content/campaign/{nodes,acts,flags}.ron
    content/dialogue/{c_elias,c_naomi,c_whitehorse,c_doyle,c_ruelas,c_wen,c_ames,c_mercer,c_alcantara,act1}.ron
    content/lint/forbidden_tokens.txt
    content/PROVENANCE.toml
    content/BIBLIOGRAPHY.md
    tests/fixtures/violation_alters_history.ron
    tests/fixtures/violation_missing_nation.ron
    tests/fixtures/violation_anachronism.ron
    tests/fixtures/mod_path_escape/mod.ron

## 7. Interfaces and Contracts

    pub fn load_all(root: &Path) -> Result<Content, ContentError>;
    pub fn validate(content: &Content) -> Vec<Diagnostic>;
    pub fn write(path: &Path, save: &SaveFile) -> Result<(), SaveError>;
    pub fn read(path: &Path, ruleset_hash: Hash32, content_hash: Hash32) -> Result<SaveFile, SaveError>;

Entity field lists are transcribed from SPEC-002 section 3 exactly. Error codes come from SPEC-006
section 2 exactly. `LedgerEntry.hash = blake3(index || prev_hash || name || role || place || date ||
chosen_line)` with entry zero carrying thirty-two zero bytes as `prev_hash`.

## 8. Milestones

### M1: Schema and loader
GOAL: Every entity in SPEC-002 section 3 round trips through RON with declared limits.
READ: SPEC-002 section 3, SPEC-005 section 1
CHANGE: crates/pb-content/src/{lib,schema,load}.rs, crates/pb-content/tests/schema.rs
CONTENT: transcribe every field list from SPEC-002 section 3. `load.rs` declares
`const MAX_CONTENT_FILE_BYTES: usize = 4 * 1024 * 1024;` and
`const MAX_RECORDS_PER_FILE: usize = 8192;` and checks both before allocating. Unknown fields are a
hard error, not a warning, so a typo in a mod is caught rather than silently ignored.
RUN: cargo test --offline -p pb-content --locked --test schema
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M1 schema ok"
FALLBACK: if RON derive proves awkward for the tile grid, store the grid as a run-length encoded
string field with a documented codec; keep every other record in plain RON.
COMMIT: git add -A && git commit -m "[EP-003][M1] content schema and loader"

### M2: The rule tables as data
GOAL: Every number in SPEC-001 lives in `content/rules/`, not in code.
READ: SPEC-001 sections 5, 7, 11, 12
CHANGE: content/rules/*.ron, crates/pb-rules/src/tables.rs
CONTENT: move the const tables written in EP-002 M3 into RON, keeping the same values. `weapons.ron`
carries the full roster from SPEC-001 section 11, every record with `first_year_available`,
`historical_note`, and `sources`. `tables.ron` carries the AP costs, hit locations, multipliers,
critical effects, called shot modifiers, range band modifiers, Sand costs, and morale thresholds.
RUN:
    cargo test --offline -p pb-rules -p pb-sim --locked
    cargo test --offline -p pb-sim --locked --test determinism
EXPECT: `test result: ok` and the determinism test still matching the EP-002 golden
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M2 rules as data, hash stable"
FALLBACK: if a value cannot be expressed as data without contorting the type, leave that single value
in code with a comment citing the spec line and record it in the Decision Log; do not move the rest
back into code.
COMMIT: git add -A && git commit -m "[EP-003][M2] rule tables as content data"

### M3: The validator: history, representation, anachronism
GOAL: Content that violates the project's laws is rejected with the exact SPEC-006 code.
READ: SPEC-000 sections 5.5 and 7, SPEC-006 section 2
CHANGE: crates/pb-content/src/validate.rs, crates/pb-content/tests/{history,representation,anachronism}.rs,
content/lint/forbidden_tokens.txt, tests/fixtures/violation_*.ron
CONTENT: E-HIST-001 fires when a campaign node tagged `HISTORICAL_FIXED` has any outgoing edge whose
`grants` alter its declared outcome, or when a mod overrides such a node. E-HIST-002 fires when a
character record lacks `nation` or `community`, or has fewer than two `sources`. E-HIST-003 fires on
any forbidden token anywhere under `content/`. E-ANACHRONISM-001 fires when a dated scenario places an
item whose `first_year_available` is later than the scenario date. Each fixture in
`tests/fixtures/` triggers exactly one code and the test asserts the code, not the message.
RUN: cargo test --offline -p pb-content --locked --test history --test representation --test anachronism
EXPECT: `test result: ok` for all three
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M3 validator laws ok"
FALLBACK: none needed. These gates are the reason the content tree is trustworthy; a validator that
warns instead of failing is not one.
COMMIT: git add -A && git commit -m "[EP-003][M3] historical, representation, anachronism validation"

### M4: The campaign graph and permadeath propagation
GOAL: Twenty-four missions and twelve camps exist as data, and no reachable content references a
dead companion.
READ: SPEC-000 section 5.4, SPEC-002 section 5
CHANGE: content/campaign/*.ron, content/dialogue/*.ron, crates/pb-content/src/campaign.rs,
crates/pb-content/tests/permadeath.rs
CONTENT: every node carries id, kind, date, requires, grants, unlocks, historical_tag, citations, and
companion_gates. Every HISTORICAL_FIXED node carries at least two citations that also appear in
`content/BIBLIOGRAPHY.md`. Permadeath propagation is computed structurally: for every companion, the
set of nodes and dialogue reachable when that companion is dead must contain zero references to them
outside the Ledger. The test asserts zero for all nine, one companion at a time and then all
combinations of any three.
RUN: cargo test --offline -p pb-content --locked --test permadeath
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M4 permadeath ok"
FALLBACK: if the all-combinations-of-three check is too slow, reduce to all singles plus a fixed
pseudo-random sample of two hundred triples chosen by a seeded generator, which is still a real
proof; record the reduction in the Decision Log.
COMMIT: git add -A && git commit -m "[EP-003][M4] campaign graph and permadeath propagation"

### M5: Saves and the Ledger chain
GOAL: A save round trips exactly; a tampered chain is detected; a hash mismatch is refused.
READ: SPEC-005 section 2, SPEC-002 section 3, SPEC-006 section 2
CHANGE: crates/pb-save/src/{lib,format,load,write,ledger}.rs, crates/pb-save/tests/integrity.rs
CONTENT: `load.rs` declares `const MAX_SAVE_BYTES: usize = 32 * 1024 * 1024;` and
`const MAX_LEDGER_ENTRIES: usize = 4096;` and checks both before allocation. The chain is recomputed
from entry zero on every load. Tests: round trip preserves `state_hash`; a flipped byte in an entry
yields `E-SAVE-TAMPERED`; a save written under a different ruleset hash yields `E-SAVE-INCOMPAT`; a
truncated file yields `E-SAVE-OVERSIZE` or a parse error but never a panic; a save with a declared
section length larger than the file is rejected before allocating.
RUN: cargo test --offline -p pb-save --locked
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M5 save integrity ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-003][M5] save format and hash-chained Ledger"

### M6: Prove the loader is faithful
GOAL: The scenarios that were constructed in test code in EP-002 now load from RON and produce the
identical golden hash.
READ: crates/pb-sim/tests/determinism.rs, tests/golden/prov_full_battle.hash
CHANGE: content/scenarios/prov_*.ron, crates/pb-content/tests/hashing.rs,
crates/pb-sim/tests/determinism.rs
CONTENT: replace the in-test scenario construction with `pb_content::load_all` plus scenario lookup.
The golden hash file is NOT regenerated. If the hash moves, the loader is not faithful and that is a
defect in this node, not a reason to refresh the golden.
RUN:
    cargo test --offline -p pb-sim --locked --test determinism
    cargo test --offline -p pb-content --locked --test hashing
    sh scripts/test-integration.sh
EXPECT: `test result: ok` twice, then `test-integration: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M6 test-integration: ok"
FALLBACK: none. Regenerating the golden here is explicitly forbidden; it would erase the only proof
that the file format and the test constructor agree.
COMMIT: git add -A && git commit -m "[EP-003][M6] prove the content loader is faithful"

### M7: Mod sandbox and provenance
GOAL: A mod cannot escape its directory, ship an executable, or override a fixed historical node, and
every asset has a license entry.
READ: SPEC-005 section 3, ARCHITECTURE.md LBI-11
CHANGE: crates/pb-content/src/mods.rs, crates/pb-content/tests/mod_sandbox.rs,
content/PROVENANCE.toml, tests/fixtures/mod_path_escape/mod.ron
CONTENT: path confinement canonicalizes every referenced path and rejects anything outside the mod
root with `E-MOD-PATH`. Any file with an execute bit or an ELF, PE, or shebang magic number is
rejected with `E-MOD-EXEC`. `PROVENANCE.toml` has one entry per asset with `path`, `origin`,
`license`, `author`, and `url_or_note`.
RUN:
    cargo test --offline -p pb-content --locked --test mod_sandbox
    sh scripts/test-integration.sh
EXPECT: `test result: ok` then `test-integration: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-003 MILESTONE_PASS "M7 mod sandbox ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-003][M7] mod sandbox and asset provenance"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Schema round trip with limits | `--test schema` | `test result: ok` |
| Rules are data and the hash is stable | `--test determinism` after M2 | `test result: ok` |
| Historical immutability, LBI-05 | `--test history` | `test result: ok` |
| Representation law, LBI-06 | `--test representation` | `test result: ok` |
| Anachronism | `--test anachronism` | `test result: ok` |
| Permadeath propagation, LBI-07 | `--test permadeath` | `test result: ok` |
| Save integrity and chain, LBI-08 and LBI-13 | `cargo test -p pb-save` | `test result: ok` |
| Loader faithfulness | `--test hashing` | `test result: ok` |
| Mod sandbox | `--test mod_sandbox` | `test result: ok` |
| Whole node | `sh scripts/test-integration.sh` | `test-integration: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-002` restores the entry state exactly, including deleting the whole
`content/` tree this node creates. No external state exists.

## 11. Progress
- [ ] M1 Schema and loader
- [ ] M2 The rule tables as data
- [ ] M3 The validator: history, representation, anachronism
- [ ] M4 The campaign graph and permadeath propagation
- [ ] M5 Saves and the Ledger chain
- [ ] M6 Prove the loader is faithful
- [ ] M7 Mod sandbox and provenance

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
