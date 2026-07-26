NODE-META-BEGIN
ID: EP-000
DEPS: -
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/preflight.sh
VERIFY_SENTINEL: preflight: ok
GREEN_TAG: green/EP-000
NODE-META-END

# EP-000 Discovery and Toolchain

## 1. Purpose and Big Picture

Prove the ground before building on it. This node touches no game code. It confirms the pinned
toolchain, initializes the repository, creates the directory skeleton every later node writes into,
and proves that every command named in COMMANDS.md exists and runs to a sentinel against an empty
project. When this node is done, no later node can fail because a tool was missing or a command was
imaginary.

## 2. Scope

- Toolchain verification: rustc exactly 1.85.0, cargo, rustup components, git, zstd, minisign,
  python3, sha256sum, and a software graphics adapter.
- Repository initialization and the first commit.
- The directory skeleton: `crates/`, `content/`, `assets/`, `tests/`, `.cargo/`.
- Loud-fail stub scripts replaced by real ones where the real one is already knowable, and a recorded
  inventory of which gates are not yet meaningful because there is no code.
- Confirming every COMMANDS.md command resolves.

## 3. Non-goals

- No Rust crates. EP-001 owns the workspace.
- No content files. EP-003 owns content.
- No game logic of any kind. EP-002 owns the kernel.
- No vendoring. EP-001 M2 owns `cargo vendor`.

## 4. Context and Orientation

Repository state at entry: the blueprint pack files only, no code, possibly no git history. The
invariants in play are procedural: LBI-01 determinism starts with a pinned compiler, and the preflight
covenant means everything external is resolved here or it is a defect later.

## 5. Files to Read First

    AGENTS.md
    COMMANDS.md
    PREFLIGHT.md
    ENVIRONMENT.md
    .agent/LOOPS.md
    .agent/GRAPH.md

## 6. Expected Changed Files

    .gitignore
    .agent/state/LEDGER.md
    ASSUMPTIONS.md
    tests/.keep
    content/.keep
    assets/.keep
    crates/.keep
    .cargo/.keep

## 7. Interfaces and Contracts

None yet. The only contract this node establishes is that every command in the COMMANDS.md table
exists as a file under `scripts/` and exits with a sentinel or a loud, named failure.

## 8. Milestones

### M1: Confirm the environment
GOAL: Every required tool is present at its required version and the preflight covenant holds.
READ: PREFLIGHT.md, ENVIRONMENT.md, scripts/preflight.sh
CHANGE: none
CONTENT: none; this milestone only observes.
RUN:
    export CI=true GIT_TERMINAL_PROMPT=0 GIT_PAGER=cat PAGER=cat DEBIAN_FRONTEND=noninteractive
    export CARGO_TERM_COLOR=never CARGO_NET_OFFLINE=true RUST_BACKTRACE=1
    set -a; . ./.env; set +a
    sh scripts/preflight.sh
EXPECT: `preflight: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-000 MILESTONE_PASS "M1 preflight: ok"
FALLBACK: none needed; a preflight failure is STOP condition (a) in AGENTS.md and is reported, not
worked around.
COMMIT: git add -A && git commit -m "[EP-000][M1] confirm toolchain and preflight"

### M2: Initialize the repository
GOAL: A git repository exists with the pack committed and a clean tree.
READ: .gitignore
CHANGE: .gitignore
CONTENT: `.gitignore` already exists in the pack. Verify it contains `.env` on its own line; if not,
append that exact line.
RUN:
    git rev-parse --git-dir >/dev/null 2>&1 || git init
    grep -qx '.env' .gitignore
    git add -A && git commit -m "[6LAYER] bootstrap blueprint pack" || true
    git status --porcelain
EXPECT: `git status --porcelain` prints nothing.
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-000 MILESTONE_PASS "M2 clean tree"
FALLBACK: none needed; git init is deterministic and idempotent.
COMMIT: git add -A && git commit -m "[EP-000][M2] initialize repository" || true

### M3: Create the directory skeleton
GOAL: Every directory a later node writes into exists and is tracked.
READ: ARCHITECTURE.md repository map
CHANGE: crates/.keep, content/.keep, assets/.keep, tests/.keep, .cargo/.keep
CONTENT: each `.keep` file contains exactly one line: `placeholder directory marker, removed when the
first real file lands`
RUN:
    for d in crates content assets tests .cargo; do mkdir -p "$d"; printf 'placeholder directory marker, removed when the first real file lands\n' > "$d/.keep"; done
    ls -d crates content assets tests .cargo
EXPECT: all five directory names printed.
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-000 MILESTONE_PASS "M3 skeleton created"
FALLBACK: none needed; mkdir is idempotent.
COMMIT: git add -A && git commit -m "[EP-000][M3] create directory skeleton"

### M4: Prove every command resolves
GOAL: Every script named in the COMMANDS.md table exists, is POSIX sh, and parses.
READ: COMMANDS.md
CHANGE: none
CONTENT: none.
RUN:
    for f in scripts/*.sh scripts/probes/*.sh; do sh -n "$f" || { echo "PARSE FAIL $f"; exit 1; }; done
    for c in install preflight format-check lint lint-determinism typecheck test-unit test-integration test-e2e build reality-gate live-fire security-check dependency-audit smoke-test verify production-readiness-check make-release-index ledger graph-next; do [ -f "scripts/$c.sh" ] || { echo "MISSING scripts/$c.sh"; exit 1; }; done
    echo "commands: resolved"
EXPECT: `commands: resolved`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-000 MILESTONE_PASS "M4 commands: resolved"
FALLBACK: if a script named in COMMANDS.md is absent, the pack is defective; append NODE_BLOCKED with
the missing path rather than authoring a substitute, because an invented gate is worse than no gate.
COMMIT: git add -A && git commit -m "[EP-000][M4] prove every command resolves"

### M5: Record the discovery inventory
GOAL: The gaps between the pack's assumptions and this machine are written down.
READ: ASSUMPTIONS.md
CHANGE: ASSUMPTIONS.md
CONTENT: append a section titled `## Verified at EP-000` with one line per assumption A-01, A-02,
A-03, A-08 stating confirmed or changed, and the exact command output that decided it.
RUN:
    rustc --version; git --version; zstd --version; python3 --version; minisign -v 2>&1 | head -n 1
    sh scripts/probes/pb_headless_adapter.sh && echo "adapter: ok"
    grep -q 'Verified at EP-000' ASSUMPTIONS.md && echo "inventory: recorded"
EXPECT: `adapter: ok` and `inventory: recorded`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-000 MILESTONE_PASS "M5 inventory: recorded"
FALLBACK: if the software adapter is absent, record A-01 as changed, append NODE_BLOCKED naming
PREFLIGHT.md section 3, because EP-005 has no exit evidence without it.
COMMIT: git add -A && git commit -m "[EP-000][M5] record discovery inventory"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Toolchain pinned and complete | `sh scripts/preflight.sh` | `preflight: ok` |
| Clean tree | `git status --porcelain` | empty |
| Skeleton present | `ls -d crates content assets tests .cargo` | all five |
| Every gate script parses | the M4 loop | `commands: resolved` |
| Assumptions recorded | `grep 'Verified at EP-000' ASSUMPTIONS.md` | a match |

## 10. Idempotence and Recovery

This node creates no code. To re-enter cold: `git reset --hard <first commit>` if any exists,
otherwise simply re-run from M1. Every milestone here is idempotent by construction.

## 11. Progress
- [ ] M1 Confirm the environment
- [ ] M2 Initialize the repository
- [ ] M3 Create the directory skeleton
- [ ] M4 Prove every command resolves
- [ ] M5 Record the discovery inventory

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
