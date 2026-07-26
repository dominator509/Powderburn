NODE-META-BEGIN
ID: EP-006
DEPS: EP-004
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/security-check.sh && sh scripts/test-integration.sh
VERIFY_SENTINEL: security-check: ok
GREEN_TAG: green/EP-006
NODE-META-END

# EP-006 Trust Boundaries, Untrusted Input, and the No-Network Guarantee

## 1. Purpose and Big Picture

A single-player offline game has no accounts, so its security work is entirely about untrusted input
and about a promise: this program does not talk to the network. This node hardens every place a file
crosses a trust boundary, proves the no-network claim at the symbol level rather than by assertion,
and makes the mod sandbox adversarial rather than merely careful.

This node runs in parallel with EP-005. They touch disjoint files. Whichever finishes second rebases
onto the first and reruns its own VERIFY before tagging.

## 2. Scope

The trust boundary inventory from SPEC-005 turned into enforced limits and fuzz targets. Symbol-level
proof that no socket, DNS, or HTTP call is reachable from `powderburn`. The mod sandbox hardened
against path escape, symlink escape, zip-slip in any archive path, executables, oversize declarations,
and decompression bombs. Save file adversarial testing. Crash artifact redaction.

## 3. Non-goals

- No authentication, authorization, sessions, tokens, or accounts. The game has no server and no
  users in the security sense. This is a deliberate scope decision recorded in ADR-0004.
- No anti-cheat and no save encryption. Saves are the player's; the Ledger chain detects corruption,
  it does not prevent editing, and SECURITY.md says so plainly.
- No telemetry to harden, because there is none.

## 4. Context and Orientation

Entry is `green/EP-004`. The five untrusted inputs are: save files, content RON, mod directories,
journal files, and the settings file. Every one of them can be hostile because a player can be handed
one by a stranger. Binding invariants: LBI-09, LBI-11, and the file limits from SPEC-005.

## 5. Files to Read First

    .agent/specs/SPEC-005-auth-and-permissions.md
    SECURITY.md
    scripts/security-check.sh
    crates/pb-content/src/mods.rs
    crates/pb-save/src/load.rs

## 6. Expected Changed Files

    crates/pb-content/src/{mods,load}.rs
    crates/pb-save/src/load.rs
    crates/pb-app/src/settings.rs
    crates/pb-cli/src/journal.rs
    crates/pb-core/src/redact.rs
    crates/pb-content/tests/{adversarial_mods,adversarial_content}.rs
    crates/pb-save/tests/adversarial_saves.rs
    crates/pb-cli/tests/adversarial_journal.rs
    crates/pb-tools/src/fuzz.rs
    tests/fixtures/adversarial/{zip_slip.mod,symlink_escape.mod,bomb.ron,huge_declared_len.pbsave,deep_nest.ron,nan_field.ron}
    SECURITY.md
    scripts/security-check.sh

## 7. Interfaces and Contracts

Every loader in the workspace obeys the same three-step shape, and the adversarial tests assert all
three: check the declared size against a `const` limit before allocating; canonicalize and confine
every path before opening; return a named SPEC-006 error rather than panicking on any malformed
input. No loader may call `unwrap` or `expect`; the workspace clippy config already denies both, and
this node proves the denial is real by attempting a violation and observing the build fail.

## 8. Milestones

### M1: The trust boundary inventory becomes limits
GOAL: Every untrusted input has a declared, enforced, and tested ceiling.
READ: SPEC-005 section 1
CHANGE: crates/pb-content/src/load.rs, crates/pb-save/src/load.rs, crates/pb-app/src/settings.rs,
crates/pb-cli/src/journal.rs
CONTENT: declare, as `const` items adjacent to the code that reads them:
`MAX_CONTENT_FILE_BYTES` 4 MiB, `MAX_RECORDS_PER_FILE` 8192, `MAX_SAVE_BYTES` 32 MiB,
`MAX_LEDGER_ENTRIES` 4096, `MAX_JOURNAL_LINES` 262144, `MAX_SETTINGS_BYTES` 64 KiB,
`MAX_MOD_FILES` 2048, `MAX_NEST_DEPTH` 64. Every read checks the file's actual length against the
limit before reading, and every parser tracks nesting depth against `MAX_NEST_DEPTH`.
RUN:
    grep -rn "MAX_CONTENT_FILE_BYTES\|MAX_SAVE_BYTES\|MAX_JOURNAL_LINES\|MAX_NEST_DEPTH" crates/ | wc -l
    cargo test --offline --workspace --locked
EXPECT: the grep count is at least 8 and `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M1 limits declared and enforced"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-006][M1] declared limits on every untrusted input"

### M2: Adversarial fixtures and the tests that consume them
GOAL: Six hostile files exist in the repository and each produces a named error, not a panic.
READ: SPEC-006 section 2
CHANGE: tests/fixtures/adversarial/*, crates/*/tests/adversarial_*.rs
CONTENT: `zip_slip.mod` references `../../etc/passwd` and must yield `E-MOD-PATH`.
`symlink_escape.mod` contains a symlink pointing outside the mod root and must yield `E-MOD-PATH`
after canonicalization. `bomb.ron` declares eight thousand records but is small; the loader must
refuse before allocating and yield `E-CONTENT-OVERSIZE`. `huge_declared_len.pbsave` declares a
section longer than the file and must yield `E-SAVE-OVERSIZE`. `deep_nest.ron` nests sixty-five
levels and must yield `E-CONTENT-DEPTH`. `nan_field.ron` puts a float where a Fix32 belongs and must
yield a parse error naming the field. Every test asserts the process exit is orderly: no panic, no
abort, no allocation failure.
RUN: cargo test --offline --workspace --locked --test adversarial_mods --test adversarial_content --test adversarial_saves --test adversarial_journal
EXPECT: `test result: ok` for all four
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M2 adversarial fixtures rejected cleanly"
FALLBACK: if a symlink fixture cannot be committed portably, create it in the test's setup function
at runtime and skip with a printed reason on platforms without symlink support, recording the skip in
the Decision Log.
COMMIT: git add -A && git commit -m "[EP-006][M2] adversarial fixtures for every untrusted input"

### M3: Prove the no-network guarantee at the symbol level
GOAL: LBI-09 is proven by inspecting the shipped binary, not by asserting good intentions.
READ: SECURITY.md, scripts/security-check.sh, ARCHITECTURE.md LBI-09
CHANGE: scripts/security-check.sh, SECURITY.md
CONTENT: extend the check to inspect the built `powderburn` binary's dynamic symbol table for
`socket`, `connect`, `bind`, `listen`, `getaddrinfo`, `gethostbyname`, `SSL_connect`, and
`curl_easy_init`, and fail naming any that appear. Also fail on any workspace dependency, transitive
included, whose name matches a network crate list held in the script. Record in SECURITY.md that the
`replay-server` feature is the sole exception, is off by default, is not compiled into `pb-app` at
all, and binds only to loopback when it is compiled at all.
RUN:
    sh scripts/build.sh
    sh scripts/security-check.sh
EXPECT: `build: ok` then `security-check: ok` including the line `symbols: no network symbols found`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M3 symbols: no network symbols found"
FALLBACK: if the platform's symbol tooling is unavailable, fall back to scanning the binary for those
byte strings, which is weaker but still real, and record the weakening in the Decision Log with a
follow-up. Do not delete the check.
COMMIT: git add -A && git commit -m "[EP-006][M3] symbol-level proof of the no-network guarantee"

### M4: Prove the check fires
GOAL: The security gate is observed failing on a real violation before it is trusted.
READ: scripts/security-check.sh
CHANGE: none permanently
CONTENT: temporarily add a `std::net::TcpStream::connect` call behind a never-taken branch in
`pb-app`, rebuild, run the check, observe the failure, then revert with `git checkout --`.
RUN:
    sh scripts/build.sh
    if sh scripts/security-check.sh; then echo "GATE DID NOT FIRE"; exit 1; else echo "gate: fires"; fi
    git checkout -- crates/pb-app/
    sh scripts/build.sh && sh scripts/security-check.sh
EXPECT: `gate: fires` then `security-check: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M4 security gate fires"
FALLBACK: none. A gate never observed to fail is decoration.
COMMIT: git add -A && git commit -m "[EP-006][M4] prove the security gate fires" || true

### M5: Redaction and crash artifacts
GOAL: A crash report contains what a maintainer needs and nothing that identifies the player.
READ: SPEC-007 section 4, SECURITY.md
CHANGE: crates/pb-core/src/redact.rs, crates/pb-app/src/main.rs
CONTENT: the panic hook writes a report containing the version, the commit, the scenario id, the
tick, the seed, the state hash, and the backtrace. It writes the home directory as `<PB_HOME>`, the
user name as `<USER>`, and any absolute path outside the install root as `<PATH>`. It never writes
save contents, companion names, or the Ledger. A test induces a panic in a child process and asserts
the report contains the state hash and contains neither the literal home path nor the user name.
RUN:
    cargo test --offline -p pb-core -p pb-app --locked
    sh scripts/security-check.sh
EXPECT: `test result: ok` then `security-check: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M5 redaction ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-006][M5] crash artifact redaction"

### M6: A short fuzz run in the gate
GOAL: The parsers survive random input for a bounded time on every run of the gate.
READ: TESTING.md
CHANGE: crates/pb-tools/src/fuzz.rs, scripts/security-check.sh
CONTENT: `pbtool fuzz --target <content|save|journal> --iters <n> --seed <n>` mutates a valid input
with a seeded generator and asserts every result is either a successful parse or a named error, never
a panic. The gate runs sixty seconds per target, seeded from the commit hash so a failure is
reproducible. A crash writes the offending input to `tests/fixtures/adversarial/` so it becomes a
permanent regression fixture.
RUN:
    cargo run --offline -q -p pb-tools --bin pbtool -- fuzz --target content --iters 20000 --seed 1
    cargo run --offline -q -p pb-tools --bin pbtool -- fuzz --target save --iters 20000 --seed 1
    cargo run --offline -q -p pb-tools --bin pbtool -- fuzz --target journal --iters 20000 --seed 1
    sh scripts/security-check.sh
    sh scripts/test-integration.sh
EXPECT: `fuzz: ok` three times, then `security-check: ok`, then `test-integration: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-006 MILESTONE_PASS "M6 security-check: ok"
FALLBACK: if sixty seconds per target makes the gate too slow for comfortable iteration, reduce the
gate run to fifteen seconds per target and keep the full run in the release gate, recording the split
in the Decision Log.
COMMIT: git add -A && git commit -m "[EP-006][M6] seeded fuzzing in the security gate"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Limits declared and enforced | M1 grep and tests | count at least 8, `test result: ok` |
| Hostile inputs rejected cleanly | four adversarial tests | `test result: ok` |
| No network symbols in the binary | `sh scripts/security-check.sh` | `symbols: no network symbols found` |
| The security gate fires on a violation | M4 | `gate: fires` |
| Crash reports are redacted | M5 tests | `test result: ok` |
| Parsers survive fuzzing | `pbtool fuzz` three targets | `fuzz: ok` |
| Whole node | `sh scripts/security-check.sh` | `security-check: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-004`. Fixtures added under `tests/fixtures/adversarial/` are removed by
that reset, which is correct: a fixture that survives a reset is a fixture nobody can explain. If
EP-005 tagged first, rebase onto `green/EP-005` and rerun this node's VERIFY before tagging.

## 11. Progress
- [ ] M1 The trust boundary inventory becomes limits
- [ ] M2 Adversarial fixtures and the tests that consume them
- [ ] M3 Prove the no-network guarantee at the symbol level
- [ ] M4 Prove the check fires
- [ ] M5 Redaction and crash artifacts
- [ ] M6 A short fuzz run in the gate

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
