# debug-validation-failure.md - the ladder, operationalized for one failing command

Inputs: FAILING_COMMAND, its full output, and the node and milestone you are in.

Step 0. Capture the ERROR SIGNATURE: the first error line, normalized by stripping timestamps,
variable path segments, addresses, and counts. Append `SIG <signature>` to the ledger. Count how many
times this signature has already appeared for this milestone.

Rung 1, first occurrence.
- Read the entire output, not the first line only. Rust puts the useful part at the bottom.
- Form exactly ONE hypothesis and write it in Surprises and Discoveries before touching anything.
- Make the smallest fix that tests that hypothesis.
- Re-run the NARROWEST command, for example `cargo test --offline -p pb-sim shot_pipeline::called_shot_breaks_gun_arm -- --exact --nocapture`, not `scripts/verify.sh`.

Rung 2, second occurrence.
- Stop patching. Isolate.
- For a determinism failure: `pbcli sim --trace-actor <id>` on both runs and diff the draw logs. The
  first differing draw names the subsystem.
- For a replay divergence: `pbcli replay --diff` and read the event log around the reported tick.
- For a content failure: `pbtool validate content` and read the error code against SPEC-006.
- Confirm or kill the hypothesis with evidence before editing code again.

Rung 3, third occurrence.
- The approach is wrong. Record every failed hypothesis.
- Switch to the milestone declared FALLBACK. Append FALLBACK_TAKEN.
- A fallback is a simpler real implementation. It is never a mock, never a skipped test, never a
  loosened assertion, never a regenerated golden.

Rung 4, fallback exhausted or MAX_ATTEMPTS reached.
- `git reset --hard <last green tag or last milestone commit>`, append ROLLBACK with the ref, then
  attempt the fallback once from clean state.

Rung 5.
- Append NODE_BLOCKED and write the 5.7 report into the ExecPlan Progress: exact blocker; full
  evidence with commands, outputs, exit codes; every signature and hypothesis; every rung with its
  diff summarized in one line; the smallest human decision needed as a question with a finite answer
  set; and a recommended default.

Absolute rules. Never the same fix twice. Never weaken the gate. Never regenerate a golden to make a
test pass. Never add `#[ignore]`. Never add a sleep.
