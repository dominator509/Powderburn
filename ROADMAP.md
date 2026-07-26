# ROADMAP

Do not implement from this file. Implementation happens only through the graph: run
`sh scripts/graph-next.sh`.

This file is strategic narrative. Phases mirror the GRAPH-TABLE one to one.

## Phase 0 - EP-000 Discovery and Toolchain
Purpose: prove the ground before building on it. Depends on nothing. Exit: rustc 1.85.0 confirmed,
repository initialized, every COMMANDS.md command executes to its sentinel against an empty skeleton.
Specs: none. Plan: EP-000.

## Phase 1 - EP-001 Foundation
Purpose: a workspace that is offline, pinned, formatted, linted, and green on nothing. Depends on
EP-000. Exit: `verify: ok` on a skeleton with one real passing test. Specs: SPEC-002. Plan: EP-001.

## Phase 2 - EP-002 Core Domain
Purpose: the simulation kernel and, above all, determinism. Depends on EP-001. Exit: unit tests green
and the triple-run determinism proof matching a golden hash. Specs: SPEC-001, SPEC-002. Plan: EP-002.

## Phase 3 - EP-003 Data and Persistence
Purpose: content becomes real: the RON schema, the validator with its historical and representation
rules, the campaign graph, the hash-chained Ledger, saves. Depends on EP-002. Exit: integration tests
green, `pbtool validate: ok`. Specs: SPEC-002, SPEC-000 section 7, SPEC-005. Plan: EP-003.

## Phase 4 - EP-004 Service Layer
Purpose: the command surface every proof drives, and the journal contract. Depends on EP-003. Exit:
`replay: match` against goldens. Specs: SPEC-003, SPEC-006. Plan: EP-004.

## Phase 5a - EP-005 Client
Purpose: the game you can see and play, through to a headless frame capture. Depends on EP-004.
Exit: `test-e2e: ok` and `capture: ok`. Specs: SPEC-004. Plan: EP-005.

## Phase 5b - EP-006 Security Baseline
Purpose: the trust boundaries, the mod sandbox, the no-network guarantee. Depends on EP-004, runs in
parallel with EP-005 in a multi-agent run. Exit: `security-check: ok`. Specs: SPEC-005, SPEC-006.
Plan: EP-006.

## Phase 6 - EP-007 Testing Hardening
Purpose: coverage to target, a regression per core outcome, forced-failure tests, no flakes. Depends
on EP-005 and EP-006. Exit: `verify: ok` from a clean tree. Specs: SPEC-008 section 2. Plan: EP-007.

## Phase 7 - EP-008 Observability and Operations
Purpose: logs, redaction, metrics, budgets, crash artifacts, runbooks. Depends on EP-007. Exit:
`smoke: ok` and every SPEC-007 acceptance criterion. Specs: SPEC-007. Plan: EP-008.

## Phase 8 - EP-009 Deployment and Release
Purpose: reproducible signed artifacts, the release index, a real rollback drill. Depends on EP-008.
Exit: `build: ok` plus verified signatures plus the drill recorded in the ledger. Specs: SPEC-008
sections 9 and 10. Plan: EP-009.

## Phase 9 - EP-010 Production Readiness and Ship
Purpose: everything, from a clean tree, then ship. Depends on EP-009. Exit: `production-readiness:
ok`, the release tagged, artifacts published to the self-hosted release directory, released smoke
green, the optional external publish printed as MANUAL, RUN_COMPLETE appended. Specs: SPEC-008.
Plan: EP-010.

## After v1 (not in this graph)
Windows cross-build; controller support; macOS; a scenario editor; the Ledger as an exportable
document; a second campaign in the Southwest, 1879 to 1886. None of these are implementable from this
file. They become graphs of their own.
