# CONTRIBUTING

## Setup

See ENVIRONMENT.md local setup. In short: rustup 1.85.0, copy `.env.example` to `.env`, fill it,
`sh scripts/preflight.sh`, `sh scripts/install.sh`, `sh scripts/verify.sh`.

## Branch rules

Trunk based. Commit to `main`. The graph commits as `[EP-XXX][Mk] <imperative summary>`. Human
commits outside a graph run use the same format with `[MANUAL]` in place of the node id. Never force
push. Never rewrite history. Never move a tag.

## Coding standards

- Rust 2021, formatted by `rustfmt.toml`, linted by clippy with warnings denied.
- Comments carry the why, not the what. Where a line exists to uphold an invariant, cite it by
  number: `// LBI-02: Fix32 keeps this reproducible across targets.`
- `unwrap` and `expect` are denied outside tests. Where allowed, one line above states the invariant
  that makes it safe.
- Every `unsafe` block carries a `// SAFETY:` comment. There should be none.
- Names come from SPEC-002. If you need a new one, add it to SPEC-002 in the same change.
- Rules belong in `content/rules/`, not in code. If you are writing a numeric constant that a
  designer might want to change, you are writing content.
- No float, clock, OS randomness, or hash map in the determinism-critical crates. The lint will catch
  it; catching it yourself is faster.

## Test requirements

Every change carries a test at the lowest level that can own the behavior, using the real
implementation. Any change under the kernel crates ends with the determinism triple-run before the
commit. Any change to the CLI surface carries a contract test asserting the exact output line.

## Documentation requirements

A behavior change updates its SPEC. A new name updates SPEC-002. A new command updates COMMANDS.md
and SPEC-003. A new environment variable updates PREFLIGHT.md, `.env.example`, ENVIRONMENT.md, and
SPEC-002 section 6, all in the same change. A new dependency updates DECISIONS.md, ENVIRONMENT.md,
and `scripts/install.sh`.

## Commit format

    [EP-XXX][Mk] <imperative summary>

One milestone per commit. Nothing uncommitted between milestones.

## Pull request checklist

This is a single-operator, agent-driven repository, so the PR checklist is applied to every commit:

- [ ] Does the diff touch only paths in the milestone CHANGE list?
- [ ] Does every new name exist in SPEC-002?
- [ ] Does the change put a rule in code that belongs in content?
- [ ] Is there a test that fails if the behavior is removed?
- [ ] Did the state hash move, and if so, is there an ADR?
- [ ] Did `sh scripts/verify.sh` print `verify: ok` in this session?

## Review checklist

Run the architecture review checklist at the end of ARCHITECTURE.md, then the agent-readiness
checklist in `.agent/checklists/agent-readiness.md`.

## Agent-specific rules

Coding agents follow AGENTS.md. It is the control plane and it wins over this file on any conflict.
