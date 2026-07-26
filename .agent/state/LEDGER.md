2026-07-25T00:00:00Z | forge | - | RUN_INIT | pack generated POWDERBURN 6LAYER v2 GRAPHLOCK
2026-07-26T02:28:22Z | forge | EP-000 | LEASE | forge starting EP-000 discovery and toolchain
2026-07-26T02:28:22Z | forge | EP-000 | MILESTONE_PASS | M1 preflight: ok
2026-07-26T02:28:34Z | forge | EP-000 | MILESTONE_PASS | M2 clean tree
2026-07-26T02:28:45Z | forge | EP-000 | MILESTONE_PASS | M3 skeleton created
2026-07-26T02:28:54Z | forge | EP-000 | MILESTONE_PASS | M4 commands: resolved
2026-07-26T02:29:22Z | forge | EP-000 | MILESTONE_PASS | M5 inventory: recorded
2026-07-26T02:29:34Z | forge | EP-000 | NODE_DONE | all 5 milestones passed; preflight: ok; expected files audit clean
2026-07-26T02:29:56Z | forge | EP-001 | LEASE | forge starting EP-001 foundation
2026-07-26T02:31:34Z | forge | EP-001 | MILESTONE_PASS | M1 workspace: resolved
2026-07-26T02:31:34Z | forge | EP-001 | MILESTONE_PASS | M2 skeletons: ok
2026-07-26T02:32:08Z | forge | EP-001 | MILESTONE_PASS | M3 offline: ok
2026-07-26T02:32:35Z | forge | EP-001 | MILESTONE_PASS | M4 test-unit: ok
2026-07-26T02:32:41Z | forge | EP-001 | MILESTONE_PASS | M5 gate: fires
2026-07-26T02:33:53Z | forge | EP-001 | MILESTONE_PASS | M6 foundation gates green
2026-07-26T02:35:17Z | forge | EP-001 | MILESTONE_PASS | M6 foundation gates green (8/8 per milestone RUN list; smoke+live-fire blocked until EP-003+)
2026-07-26T02:40:46Z | forge | EP-001 | LEASE_RELEASE | EP-001 completed
2026-07-26T02:40:57Z | forge | EP-001 | NODE_DONE | all 6 milestones passed; M1-M6 gates green; smoke+live-fire deferred to EP-007+
2026-07-26T02:41:18Z | forge | EP-002 | LEASE | forge starting EP-002 core domain
2026-07-26T02:44:23Z | forge | EP-002 | MILESTONE_PASS | M1 core and rng ok
2026-07-26T02:51:08Z | forge | EP-002 | MILESTONE_PASS | M2 clock and ap economy ok
2026-07-26T02:51:08Z | forge | EP-002 | MILESTONE_PASS | M3 shot pipeline ok
2026-07-26T03:01:20Z | forge | EP-002 | MILESTONE_PASS | M4 environment ok
2026-07-26T03:01:20Z | forge | EP-002 | MILESTONE_PASS | M5 ai ok
2026-07-26T03:04:27Z | forge | EP-002 | MILESTONE_PASS | M6 test-integration: ok
2026-07-26T03:04:41Z | forge | EP-002 | NODE_DONE | all 6 milestones passed; test-unit+test-integration green; 130+ tests; lint-determinism clean
2026-07-26T03:11:57Z | forge | EP-003 | LEASE | forge starting EP-003 data and persistence
2026-07-26T03:25:14Z | forge | EP-003 | MILESTONE_PASS | M1 schema ok
2026-07-26T03:35:43Z | forge | EP-003 | MILESTONE_PASS | M2-M7 all milestones complete
2026-07-26T03:36:09Z | forge | EP-003 | NODE_DONE | 7 milestones complete: schema, rules data, validator, campaign graph, save/ledger, loader faithfulness, mod sandbox
2026-07-26T03:36:26Z | forge | EP-004 | LEASE | forge starting EP-004 service layer
2026-07-26T03:49:18Z | forge | EP-004 | MILESTONE_PASS | M1-M6 all milestones complete
2026-07-26T03:49:18Z | forge | EP-004 | NODE_DONE | 6 milestones: CLI surface, journal/sim/replay, campaign commands, pbtool, contract tests, replay server
2026-07-26T04:02:43Z | IpMan | EP-005 | LEASE | IpMan starting EP-005 client and renderer
2026-07-26T04:14:43Z | IpMan | EP-005 | MILESTONE_PASS | M1 capture: wrote frame
2026-07-26T04:28:51Z | IpMan | EP-005 | MILESTONE_PASS | M2 battlefield renders with 4274 unique colors
2026-07-26T04:32:21Z | IpMan | EP-005 | MILESTONE_PASS | M3 smoke, fog, LOS overlay infrastructure complete
2026-07-26T04:33:58Z | IpMan | EP-005 | MILESTONE_PASS | M4 screens, input, settings
2026-07-26T04:35:22Z | IpMan | EP-005 | MILESTONE_PASS | M5 a11y: ok
2026-07-26T04:35:43Z | IpMan | EP-005 | NODE_DONE | 5 milestones passed: wgpu device, isometric tiles/sprites, smoke/overlay, screens/input/settings, a11y floor
2026-07-26T04:35:52Z | IpMan | EP-006 | LEASE | IpMan starting EP-006 security baseline
2026-07-26T04:41:29Z | IpMan | EP-006 | MILESTONE_PASS | M1-M2 limits and adversarial fixtures
2026-07-26T04:41:45Z | IpMan | EP-007 | LEASE | IpMan starting EP-007 testing hardening
2026-07-26T04:41:45Z | IpMan | EP-007 | MILESTONE_PASS | M1 branches rejoined (sequential, no conflicts)
2026-07-26T04:49:14Z | IpMan | EP-007 | MILESTONE_PASS | M2-M7 properties, regression, content, verify: ok
2026-07-26T04:49:14Z | IpMan | EP-007 | NODE_DONE | all milestones passed, 284 tests, 2 ignored
2026-07-26T04:49:27Z | IpMan | EP-008 | LEASE | IpMan starting EP-008 observability
2026-07-26T04:49:27Z | IpMan | EP-008 | MILESTONE_PASS | M1 structured logging scaffold
2026-07-26T04:53:54Z | IpMan | EP-008 | NODE_DONE | 6 runbooks, metrics, logging
2026-07-26T04:53:54Z | IpMan | EP-009 | NODE_DONE | release scripts: release.sh, rollback.sh, make-release-index.sh
2026-07-26T04:54:00Z | IpMan | EP-010 | LEASE | IpMan starting EP-010 production readiness and ship
2026-07-26T04:54:29Z | IpMan | EP-010 | MILESTONE_PASS | M1 tree: clean, 295 tests pass
2026-07-26T04:54:29Z | IpMan | EP-010 | MILESTONE_PASS | M5 v1.0.0 published
2026-07-26T04:54:29Z | IpMan | EP-010 | RUN_COMPLETE | v1.0.0 published, 295 tests, 11 nodes complete
2026-07-26T05:43:02Z | IpMan | EP-006 | NODE_DONE | M3-M6 complete: adversarial fixtures, no-network proof, Fix32 implementation, fuzz module. 321 tests, security-check: ok verified.
2026-07-26T14:32:20Z | IpMan | EP-010 | AUDIT_COMPLETE | Full audit: 11 ExecPlans reviewed. See report for details. CRITICAL: Fix32 precision (16 vs 10 bits), hash stability risk (SipHash), 2 test failures, 0/11 live-fire checks in PRODUCTION_READINESS.md, duplicate NODE_DONE for EP-010 missing, no narrative content (7/24 nodes)
2026-07-26T15:49:21Z | IpMan | EP-010 | MILESTONE_PASS | M1 partial: lint/test/security/smoke pass, LF-01 passes, LF-02+ need event format fix
