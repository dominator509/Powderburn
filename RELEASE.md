# RELEASE

## Release types

| Type | Meaning | Version change |
| --- | --- | --- |
| Patch | Bug fix with no rule change and no state hash change | 0.0.x |
| Minor | Content or feature addition; `content_hash` changes so saves are invalidated | 0.x.0 |
| Major | Save `format_version` bump or a rule change that moves the state hash | x.0.0 |

## Versioning

Semantic versioning over the player-visible contract, which is: the rules, the content, and the save
format. The single source of the version string is `crates/pb-app/Cargo.toml`. Every artifact name,
the tag, and the release directory take that value.

A version is never reused. A tag is never moved. A published artifact is never replaced in place.

## Changelog

`CHANGELOG.md` at the repository root, newest first, with these headings per release: Rules,
Content, Interface, Fixes, Save compatibility. The Save compatibility heading is mandatory and states
plainly whether saves from the previous version load, and if not, why.

## Branch strategy

Trunk based. `main` is always releasable. The graph commits directly to `main` with `[EP-XXX][Mk]`
messages. Release tags are `v<version>`; node tags are `green/EP-XXX`. No release branches; a fix to a
released version is a new patch release from trunk.

## Release candidate criteria

1. `sh scripts/verify.sh` green from a clean tree.
2. `sh scripts/production-readiness-check.sh` green.
3. Two builds of the same commit produce identical sha256.
4. The golden campaign replay is green and under its wall budget.
5. CHANGELOG.md has an entry with a Save compatibility line.

## Release checklist

See `.agent/checklists/release.md`. It is the operator-facing form of the same list and includes the
exact commands.

## Smoke

`sh scripts/smoke-test.sh --released` after publication. Verifies the signature first, then unpacks,
then runs the binary and the selftest.

## Approvals

None required. When AUTO_DEPLOY is authorized and every gate passes, the run publishes to
`PB_RELEASE_DIR` hands-off. This is stated explicitly so that nobody inserts a human gate that the
pack did not ask for. External publication to itch.io is the only human step and it is MANUAL by
ADR-0012.

## Release notes

Written from the CHANGELOG, plus: the reference machine, the sha256 of each artifact, the public key
fingerprint, and the save compatibility statement. Published as `$PB_RELEASE_DIR/<version>/NOTES.md`.

## Post-release monitoring

There is no telemetry, so monitoring means: watch the inbox. A player report is triaged with the
three files named in OBSERVABILITY.md production debugging. A determinism regression that reached
players is treated as a rollback trigger, not a patch-later item.
