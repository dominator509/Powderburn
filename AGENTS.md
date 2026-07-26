# AGENTS.md - POWDERBURN control plane

## 1. Mission

POWDERBURN is a squad tactical role-playing game: a deterministic, offline, single-player campaign
set on the American frontier between 1867 and 1878, built in Rust with a continuous-turn-based
combat kernel modeled on the Fallout Tactics lineage (action points, called shots to hit locations,
critical tables, stances, real line of sight, squad of up to six) and extended with period-truthful
systems that no game in that lineage has: black powder smoke that persistently degrades line of
sight, cap-and-ball reload economies, misfire and fouling, and a morale pool called Sand. Its story
is fictional and its history is not: the campaign threads a company of eight recruitable outcasts
through real, unalterable events from the Medicine Lodge Treaty to the Lincoln County War, and every
named person who dies is written into an append-only, hash-chained in-game document called the
Ledger, which is also the save-integrity root and the closing credits. Your job in this repository
is to take it from greenfield to a signed, shipped, live-fire-proven release with no human in the
loop after preflight.

## 2. THE BOOT SEQUENCE

PRIME-BLOCK-BEGIN
This repository is governed by a 6LAYER blueprint pack. AGENTS.md is the authoritative control plane; if anything here conflicts with AGENTS.md, AGENTS.md wins.
On every session start, execute THE BOOT SEQUENCE:
1. Read AGENTS.md fully. 2. Read COMMANDS.md. 3. Read .agent/GRAPH.md and .agent/LOOPS.md. 4. Run: sh scripts/ledger.sh tail 30. 5. Run: sh scripts/preflight.sh -- it MUST print "preflight: ok"; if it fails, report the exact missing items from PREFLIGHT.md and stop (this is the only legitimate pre-run stop). 6. Run: sh scripts/graph-next.sh and dispatch on its one-line output exactly as .agent/GRAPH.md specifies. 7. Repeat step 6 after every completed node until ALL_DONE, then run the ship gate in AGENTS.md.
Hard rules: do not ask the user questions; choose the smallest reversible option, record it, continue. Use only commands from COMMANDS.md. Never invent an API, route, table, flag, or env var -- verify in-repo or transcribe from the pack. One node at a time; milestones in order; commit after every milestone; append ledger events as .agent/LOOPS.md requires. Bounded retries per .agent/LOOPS.md -- never repeat a failed fix. No mocks, stubs, demo modes, or placeholder code in production paths; scripts/reality-gate.sh and scripts/live-fire.sh must genuinely pass. Never weaken a gate, skip a test, or claim an unrun result. Stop only at NODE_BLOCKED (with the full evidence report) or ALL_DONE.
PRIME-BLOCK-END

## 3. Source-of-truth hierarchy

current explicit user instruction > L1 CONTROL > L2 SPECIFICATION > L3 GRAPH > L4 EXECUTION >
repository code and tests > L5 gate output as fact > L6 ledger as history.

- L1 CONTROL (laws, never edited during a run): AGENTS.md, every adapter file,
  .agent/EXECUTION_RULES.md, .agent/LOOPS.md, the rules half of .agent/GRAPH.md.
- L2 SPECIFICATION (changed only by the spec-update rule, with repository evidence and a ledger
  entry): PROJECT_BRIEF.md, ARCHITECTURE.md, .agent/specs/*, SECURITY.md, PREFLIGHT.md,
  ENVIRONMENT.md.
- L3 GRAPH (fixed at generation; a run never rewires its own graph): the GRAPH-TABLE in
  .agent/GRAPH.md, ROADMAP.md, the node inventory.
- L4 EXECUTION: .agent/execplans/*, .agent/prompts/*, .agent/templates/*, COMMANDS.md,
  CONTRIBUTING.md. Only ExecPlan Progress, Surprises, Decision Log, and Outcomes sections are
  mutable.
- L5 VERIFICATION: TESTING.md, scripts/*, .agent/checklists/*, .agent/reality-patterns,
  .agent/reality-allow, and the test suites the plans create. Gates never weaken mid-run. You may
  fix code to satisfy a gate. You may never edit a gate to satisfy code. The single narrow exception
  is adding one justified line to .agent/reality-allow WITH a Decision Log entry naming the file,
  the pattern, and why the match is legitimate.
- L6 STATE (the only always-writable layer): .agent/state/LEDGER.md (append-only), git history,
  green tags, and evidence captured in ExecPlan Progress.

When code contradicts a spec, the spec wins and the code changes. When a plan contradicts a spec,
the plan is corrected through the spec-update rule with a ledger entry.

## 4. The graph protocol

One node equals one ExecPlan equals one bounded unit of work with entry evidence, exit evidence, and
a green tag. Node IDs are EP-000 through EP-010. Dependencies are declared in the GRAPH-TABLE.

SINGLE WRITER: at most one node is IN_PROGRESS repository-wide, ever. The holder is recorded by a
LEASE event in the ledger.

A node is DONE only when all five of these are true: every milestone passed with observed evidence;
the node VERIFY command printed its VERIFY_SENTINEL in this session; the Expected Changed Files
audit passed; a NODE_DONE ledger event was appended; and the tag green/EP-XXX was created. A DONE
claim missing any one of the five is a fabrication.

Dispatch on the single line printed by `sh scripts/graph-next.sh`:
- `NEXT <id>`: append LEASE, then execute that ExecPlan from milestone one.
- `RESUME <id>`: a lease is open. If it is yours, re-verify the last checked milestone sentinel, then
  continue at the first unchecked milestone. If it belongs to another agent and its most recent
  ledger event is older than 90 minutes, append LEASE_TAKEOVER and continue from ledger and ExecPlan
  state. Otherwise do nothing; another agent is live.
- `BLOCKED <id>`: the run is terminally halted. Read the NODE_BLOCKED report. Do not restart, do not
  work around, do not re-lease.
- `STALL <id>`: graph defect. Append NODE_BLOCKED for that id with detail GRAPH_STALL and treat as
  BLOCKED.
- `ALL_DONE`: run the ship gate in section 15, then append RUN_COMPLETE.

Checkpoint and rollback: commit after every milestone with message `[EP-XXX][M<k>] <summary>`.
Nothing is left uncommitted between milestones. Tag `green/EP-XXX` at every NODE_DONE. Rollback is
invoked only by rung 4 of the ladder in .agent/LOOPS.md: `git reset --hard <last green tag or last
[EP-XXX][M<k-1>] commit>`, append ROLLBACK with the target ref, then re-enter the milestone on its
declared FALLBACK path. Rollback never crosses a green tag of a completed node.

Multi-agent cohesion: git plus the ledger are the entire coordination bus. Run graph-next.sh fresh
before every lease and never cache a dispatch. While holding a lease, append HEARTBEAT at least
every fifteen minutes of activity and after every milestone. Append LEASE_RELEASE if you stop for
any reason other than NODE_DONE or NODE_BLOCKED. Solo operation is the degenerate case of the same
protocol.

## 5. STOP conditions

These are the only conditions under which you stop. There are no others.

a. Preflight failure before the run. Report the exact missing items by name from PREFLIGHT.md and
   stop.
b. An action would destroy user data or production data, or cause an irreversible external side
   effect that the specs do not explicitly authorize. Publishing to itch.io is in this class and is
   always MANUAL.
c. A legal, financial, or security judgment the specs do not answer. Concretely for this project:
   any question about the license or provenance of an authored asset that content/PROVENANCE.toml
   does not answer, and any question about whether a depiction of a named historical nation or
   community complies with SPEC-000 section 7 that the spec does not answer.
d. NODE_BLOCKED after the full ladder in .agent/LOOPS.md, with the structured report.
e. Production deploy is authorized for PB_RELEASE_DIR only. External publication is emitted as a
   MANUAL command and the run ends clean.

Everything else: choose the smallest reversible option, record it in the ExecPlan Decision Log,
continue. Do not ask the user for next steps, preferences, or confirmation. Proceed.

## 6. Anti-drift rules

After every milestone run `git status --porcelain` and `git diff --name-only HEAD~1`. Any path
outside that milestone CHANGE list is reverted immediately with `git checkout -- <path>` or
`git clean -fd <path>` unless a Decision Log entry justifying it was written BEFORE keeping it.
Run the Expected Changed Files audit at node end: the set of files changed across the node must
equal the node section 6 list. No broad refactors. No renaming outside a milestone CHANGE list. No
unrelated cleanup, no opportunistic dependency upgrades, no reformatting files you did not otherwise
touch. If you notice a defect outside the current node, write it in Surprises and Discoveries and
keep going.

## 7. Anti-hallucination rules

Never invent a crate API, a function signature, a command, an environment variable, a content key,
a component name, a system name, or a flag. Every name comes from one of exactly three places: a
file you have just read in this repository, a vocabulary table in .agent/specs/SPEC-002 or SPEC-003,
or verbatim content embedded in the ExecPlan you are executing. Commands come only from COMMANDS.md.
Before you call any third-party crate item, read the vendored source at
`vendor/<crate>/src/lib.rs` or the item's own module and confirm the signature; the repository is
vendored offline precisely so that this is always possible. Record every assumption in the Decision
Log.

## 8. Anti-fixation rules

Track failures by ERROR SIGNATURE: the first error line of output, normalized by stripping
timestamps, variable path segments, addresses, and counts. Append `SIG <signature>` to the ledger on
every failure. Climb the ladder in .agent/LOOPS.md section 5.3 by same-signature failure count.
MAX_ATTEMPTS per milestone is declared in each node NODE-META header. A new signature resets the rung
but not the attempt total. Absolute rule: the same fix may never be applied twice. If the diff you
are about to write matches a diff already tried for this signature, you are on the wrong rung.
Climb. Identical command with identical output three times in a row is a forced rung climb.

## 9. Reality law

PRODUCTION PATH means any code that runs when a player exercises a core user outcome, plus its
content data, schema, and build configuration. In this repository that is `crates/`, `content/`,
`assets/`, and `build.rs` files.

TEST DOUBLE ZONE means `crates/*/tests/`, `tests/`, and `#[cfg(test)]` modules, and nothing else.
Even inside that zone, integration, replay, and live-fire suites run the real simulation kernel
against real content files. There is no in-memory impostor for the simulation.

FABRICATION means any of: stubbed systems; hardcoded sample battle results presented as simulated
results; a `demo_mode` or `sandbox_mode` branch in production code; functions that return Ok without
performing the effect; a renderer that draws a placeholder rectangle where an asset belongs; sleeping
and pretending; tests asserting on a mock of the thing under test; commenting out or `#[ignore]`-ing
a failing test; weakening a gate to pass it. All of these are forbidden everywhere in production
paths.

Software that appears to work is a failure state. Only software proven by live-fire counts.

Evidence rules: a gate passes only if you actually ran it in this session and the sentinel line
appeared in real output. Claiming a pass from memory, from a previous run, or from reading the
script is fabrication. Every MILESTONE_PASS ledger event carries the sentinel observed in its
detail. Final review re-runs verify.sh from scratch; cached green is not green.

## 10. Dependency rules

The dependency budget is closed and pinned. Before adding any crate: check whether an existing
workspace crate or an already-vendored dependency does the job; prefer the standard library; prefer
writing thirty lines to adding a dependency. If a crate is genuinely required, it must be added with
an exact version, no caret, no wildcard, then `cargo vendor` is re-run, the lockfile and `vendor/`
are committed, an ADR is written in DECISIONS.md, and ENVIRONMENT.md and scripts/install.sh are
updated in the same milestone. Any crate that performs network I/O, reads the system clock inside a
determinism-critical path, or spawns threads with nondeterministic scheduling is forbidden in
pb-core, pb-rng, pb-rules, pb-sim, pb-ai, and pb-content.

## 11. File creation and commit rules

Create files only at paths listed in the active milestone CHANGE list. Commit after every milestone
with `git add -A && git commit -m "[EP-XXX][M<k>] <imperative summary>"`. Never force push, never
rewrite history, never amend a commit that is already tagged. Tag only through the node completion
step.

## 12. Testing rules

See TESTING.md for the pyramid, the validation matrix, and the flaky-test policy. Restated here:
a flaky test is a bug. Fix it or delete it with an ADR. Never retry until green, never add a sleep
to make a test pass, never mark a test ignored to get a gate green.

## 13. Documentation update rules

L1 files are frozen for the run. L2 spec changes require: the repository evidence that forced the
change quoted in the ExecPlan Decision Log, the edit itself, a ledger append, and a corresponding
update to any ExecPlan that transcribed the old value. L3 is frozen. L4 ExecPlan bodies are frozen;
only Progress, Surprises and Discoveries, Decision Log, and Outcomes and Retrospective may be
written. L5 gates may be extended, never weakened. L6 is append-only.

## 14. Security rules

See SECURITY.md. Restated here: secrets live only in `.env`, are never committed, never printed,
never written to a save file or a crash dump. The shipped binary makes zero network calls; this is
enforced by a workspace deny lint and by a symbol audit in scripts/security-check.sh. Save files and
mod data are untrusted input and are parsed with bounds checks and explicit size limits before any
allocation. Never run a downloaded or user-supplied file as code; data mods are data only.

## 15. Definition of done

For a node: all five conditions in section 4.

For the run, THE SHIP GATE, executed inside EP-010 when graph-next.sh prints ALL_DONE:
1. Working tree clean and equivalent to a fresh clone: `git status --porcelain` prints nothing.
2. `sh scripts/verify.sh` prints `verify: ok`.
3. `sh scripts/production-readiness-check.sh` prints `production-readiness: ok`.
4. Both sentinels observed in this session in real output.
5. `git tag -a v<version> -m "POWDERBURN v<version>"` created from RELEASE.md versioning rules.
6. AUTO_DEPLOY is authorized for PB_RELEASE_DIR: execute the deploy steps in DEPLOYMENT.md hands-off,
   then run `sh scripts/smoke-test.sh --released` and observe `smoke: ok`.
7. External publication is NOT authorized: print the exact butler command from DEPLOYMENT.md marked
   MANUAL and stop clean.
8. Append RUN_COMPLETE to the ledger with the release tag in the detail.

Shippable means this gate passed with real observed output. Nothing less, nothing simulated.

## 16. Final response requirements

Every final response you produce must contain, in this order: nodes completed this session; files
changed versus the Expected Changed Files list; every command run with the sentinel actually
observed; acceptance status per criterion; decisions made and where they are recorded; assumptions
confirmed or changed; remaining risks; ship-gate status.
