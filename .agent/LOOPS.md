# LOOPS - every loop in POWDERBURN is declared, bounded, and terminates

The no-deadlock guarantee: every loop below exits in bounded iterations into exactly one of
{pass, fallback, rollback, NODE_BLOCKED}. A fake pass is forbidden by the evidence rules in
AGENTS.md section 9.

## 5.1 The run loop (outermost)

Repeat: run `sh scripts/graph-next.sh`, dispatch per the table in .agent/GRAPH.md. On ALL_DONE run
the ship gate and exit. Bounded because the node count is eleven, every node terminates by 5.2, and
BLOCKED exits the loop.

## 5.2 The node loop

For the leased node: execute milestones strictly in order, each through 5.3. After the last
milestone: run the node VERIFY command, run the Expected Changed Files audit, append NODE_DONE, tag
`green/EP-XXX`, append LEASE_RELEASE. Bounded because milestones are finite and 5.3 terminates.

## 5.3 The milestone loop (the verify-fix ladder)

Every milestone ends with RUN commands and EXPECT sentinels. On mismatch, climb this ladder.

Track failures by ERROR SIGNATURE: the first error line of output, normalized by stripping
timestamps, variable path segments, memory addresses, and counts. For Rust that usually means the
`error[EXXXX]:` line or the first `thread 'x' panicked at` line with the path trimmed to the crate
and module. Append `SIG <signature>` to the ledger on every failure. The ladder counts SAME-signature
failures. A new signature resets to rung 1, but the milestone attempt total is capped at
MAX_ATTEMPTS from the node NODE-META header (default 6).

- Rung 1, first same-signature failure. Read the full error. Form ONE hypothesis. Make the smallest
  targeted fix. Rerun the NARROWEST failing command, not the whole suite. For Rust that means
  `cargo test --offline -p <crate> <testname> -- --exact --nocapture`, not `scripts/verify.sh`.
- Rung 2, second. Stop patching. Isolate. Write or run a narrower diagnostic: a single test, a single
  module, an added assertion, a `pbcli sim --trace-actor <id>` run. Confirm or kill the hypothesis
  with evidence before touching code again.
- Rung 3, third. The approach is wrong. Record the failed hypotheses in Surprises and Discoveries,
  then switch to the milestone declared FALLBACK path. A fallback is a simpler real implementation
  or an already-vendored alternative. A fallback is never a mock.
- Rung 4, the fallback also exhausts three attempts, or MAX_ATTEMPTS is reached. ROLLBACK per
  AGENTS.md section 4: reset hard to the last green tag or last milestone commit, append ROLLBACK
  with the target ref, then attempt the fallback path once from a clean state.
- Rung 5. Append NODE_BLOCKED with the structured report of 5.7. Terminal. Never loop back, never
  fake a pass, never comment out the failing test, never add `#[ignore]`.

Absolute rule: the same fix may never be applied twice. If the diff you are about to make matches a
diff already tried for this signature, you are on the wrong rung. Climb.

## 5.4 Readiness loops

Any started process is probed, never assumed. Loop up to 30 times with a 2 second sleep against an
exact readiness command. On success continue. On exhaustion treat as a milestone failure with
signature `READINESS_TIMEOUT_<service>` and enter 5.3.

The only backgrounded process in this repository is `pbcli serve-replay`. Its start, probe, and kill
commands are in COMMANDS.md. Every background start records its PID into `$PB_CACHE_DIR/pbcli.pid`
and teardown with `kill` is part of the same milestone. A milestone that starts a process and does
not kill it fails its own scope audit.

## 5.5 Watchdogs

- Repetition watchdog. Identical command with identical output three times in a row means you are
  spinning. Forced rung climb.
- Silence watchdog. Ten consecutive actions without a ledger append means append HEARTBEAT with a
  one line status now.
- Scope watchdog. After every milestone run `git status --porcelain` and
  `git diff --name-only HEAD~1`. Any path outside the milestone CHANGE list is reverted immediately
  unless a Decision Log entry justifying it was written BEFORE keeping it.
- Budget watchdog. If a milestone exceeds its declared step or wall budget, treat it as a failure
  with signature `BUDGET_EXCEEDED` and enter 5.3 at rung 3. Do not grind.
- Determinism watchdog, specific to this project. Any milestone that touches pb-core, pb-rng,
  pb-rules, pb-sim, pb-ai, or pb-content must end by running LF-03, the triple-run determinism proof.
  A drifting state hash is a failure with signature `DETERMINISM_DRIFT` and enters 5.3 at rung 2,
  because the cause is always structural: a float in state, a HashMap iteration, a clock read, or an
  unordered parallel fold.

## 5.6 The re-grounding loop (drift killer)

At the start of EVERY milestone, before any action, re-read in this order: (1) the milestone block
itself, (2) the node Non-goals section, (3) `sh scripts/ledger.sh tail 15`. Long context drift dies
here, because the instructions nearest the work are always the freshest thing in context.

## 5.7 The blocked report (the only legitimate terminal failure)

NODE_BLOCKED detail must reference a report appended to that ExecPlan Progress section containing:
the exact blocker; full evidence with commands, outputs, and exit codes; every signature and every
hypothesis tried; every rung climbed with diffs summarized in one line each; the smallest human
decision needed, phrased as a question with a finite answer set; and a recommended default. A BLOCKED
without this report is itself a defect.

## 5.8 Non-interactive mandate

Every command runs unattended. Export the non-interactive block from COMMANDS.md at session start.
Forbidden outright: bare interactive REPLs, editors, pagers, watch modes, prompt-on-conflict
commands, and any credential prompt. Credentials come from `.env` only, loaded by scripts. Watch and
dev servers are allowed only backgrounded under 5.4.
