# SECURITY

## Goals

POWDERBURN is an offline single-player game. It holds no user accounts, transmits nothing, and makes
zero network calls. The security work is therefore concentrated in three places: untrusted file
parsing, the mod sandbox, and the integrity of the release supply chain.

## Threat model summary

| Threat | Vector | Mitigation |
| --- | --- | --- |
| Malicious save file shared between players | Crafted `.pbsave` | Size limits before allocation, typed parse, chain verification, refusal codes; see SPEC-005 s2 |
| Malicious mod | Files under `$PB_CONFIG_DIR/mods/` | Data only, path confinement, executable rejection, validator gate; see SPEC-005 s3 |
| Malicious asset | Crafted PNG or audio file | Dimension and size caps, decoder refusal without placeholder fallback |
| Tampered release artifact | Man in the middle on download | Detached minisign signature, public key published separately, smoke verifies before unpack |
| Credential leakage from the build host | `.env`, signing key | Never committed, never logged, key required to be outside the repository at mode 600 |
| Supply chain injection via a dependency | Registry compromise | Everything vendored and committed; builds are offline; dependency count capped at 60; every crate license present |
| Unexpected exfiltration | A dependency opening a network connection | No network APIs, resolver, transport, server, or TLS symbols; Linux `socket`/`connect` are narrowly allowlisted for winit's local Wayland/X11 display IPC and all other checked symbols fail the `nm` gate |

## Authentication and authorization

None exist and none are needed. See SPEC-005 for what replaces them.

## Input validation at every trust boundary

Every boundary in SPEC-005 section 1 has exactly one parsing function with an explicit `MAX_` limit
constant checked before any allocation. `scripts/security-check.sh` fails if
`crates/pb-save/src/load.rs` or `crates/pb-content/src/load.rs` lacks such a constant.

## Output encoding

The only untrusted text rendered is a mod's display strings and a player's chosen character name.
Both are length capped, stripped of control characters other than newline, and rendered as text with
no markup interpretation anywhere.

## Secret management

Secrets live in `.env` only, are loaded by scripts, and are read by no shipped binary. `.env` is in
`.gitignore` from EP-001 milestone 1. No secret is ever logged, embedded in an artifact, or written
to a crash file.

## Dependency security policy

Every dependency is pinned to an exact version and vendored. The audit gate fails on any version
range, any unvendored crate, any crate without a license file, or a total count above 60. A waiver
requires an ADR in DECISIONS.md and an entry in `.agent/dep-waivers` naming the crate. There is no
severity threshold to negotiate because there is no network attack surface at runtime; the risk is
supply chain, and vendoring is the answer.

## Log redaction rules

Never logged: environment values, `.env` contents, absolute paths outside the config and install
directories, player-chosen names, and Ledger entry text. Enforced by a grep in
`scripts/security-check.sh`.

## Data protection

The game stores saves and settings under the user profile with mode 0600 for saves. It writes nowhere
else. It reads nothing about the machine beyond what the graphics and window system require.

## Production data rules

There is no production data. The nearest equivalent is the release directory and the signing key. The
run may write signed artifacts and an index into `PB_RELEASE_DIR`. It may never delete a previously
published artifact, because rollback depends on the previous artifact still being there.

## Safe migration rules

Save format changes are expand then refuse, never silently migrate: bump `format_version`, ship the
refusal message naming both versions, document in RELEASE.md. Content changes bump `content_hash`
which invalidates saves by design, and the UI states this before a mod is enabled.

## Hardening checklist (wired into scripts/security-check.sh)

1. `.env` untracked.
2. No private key material in tracked files.
3. No credential-shaped literal in `crates`, `content`, or `scripts`.
4. No network API or transport symbol in the release binary; only the documented local-display `socket`/`connect` exceptions are allowed.
5. No network API outside the feature-gated replay server.
6. The replay-server feature unreachable from `pb-app`.
7. Every untrusted parser has a `MAX_` limit constant.
8. Every `unsafe` block carries a `// SAFETY:` comment.
9. No environment value interpolated into a log macro.
10. `scripts/dependency-audit.sh` passes.

## Security STOP conditions

Stop and report, per AGENTS.md section 5, if: an action would publish, delete, or overwrite an
artifact outside `PB_RELEASE_DIR`; an asset's license or provenance cannot be determined from
`content/PROVENANCE.toml`; or a change would require running foreign code to load a mod.
