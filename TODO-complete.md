# POWDERBURN Completion Plan

## Phase 1: Code fixes for live-fire proofs
- [ ] LF-01: campaign play counts deaths during sim, emits ledger-entries: N
- [ ] LF-02: events display string actor names not numeric IDs
- [ ] LF-03: golden hash updated to real deterministic hash
- [ ] LF-04: save/load round trip preserves hash (sim --input mode)
- [ ] LF-05: sim emits CompanionKilled events when companion dies
- [ ] LF-06: branching campaign choices (spare/kill Teague)
- [ ] LF-07: historical immutability (pbtool validate --with-fixture)
- [ ] LF-08: capture wgpu (needs real headless adapter)
- [ ] LF-09: representation/provenance validation (stubs exist)
- [ ] LF-10: turn budget measurement (bench --emit-budget)

## Phase 2: Campaign content
- [ ] 17 additional campaign nodes (for 24 total)
- [ ] 4+ companion dialogue files
- [ ] New scenario files for new missions
- [ ] Camp/rest nodes
- [ ] Act structure

## Phase 3: Fix remaining regression tests
- [ ] reality_gate_catches_todo_markers
- [ ] format_check_catches_formatting_violations

## Phase 4: Re-verify and re-audit
- [ ] Run full verify chain
- [ ] Run all live-fire proofs
- [ ] Audit against spec
- [ ] Document honest limitations + solutions
