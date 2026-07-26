NODE-META-BEGIN
ID: EP-009
DEPS: EP-008
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/verify.sh && sh scripts/smoke-test.sh --released
VERIFY_SENTINEL: smoke-test: ok
GREEN_TAG: green/EP-009
NODE-META-END

# EP-009 Deployment and Release

## 1. Purpose and Big Picture

Turn a green repository into artifacts a stranger can download, verify, and run, and prove that going
backwards works before it is ever needed. Reproducibility is the organizing idea: the same commit on
the same toolchain must produce byte-identical archives, because a build nobody can reproduce is a
build nobody can audit.

## 2. Scope

Reproducible release archives for Linux, Windows, and macOS. Checksums and detached signatures. The
release index. Automated publication to the self-hosted `PB_RELEASE_DIR` only. A rehearsed rollback
drill. The manual itch.io step documented but never automated, per ADR-0012.

## 3. Non-goals

- No itch.io push. AUTO_DEPLOY covers the self-hosted directory only. The butler command is printed
  for a human to run and is never executed by an agent.
- No installers, no code-signing certificates beyond the detached signature, no store packaging.
- No new features. A feature that appears in this node is a process failure.

## 4. Context and Orientation

Entry is `green/EP-008`. The release directory is the deployment target and it is a plain filesystem
path, which is why `AUTO_DEPLOY` can be yes without risk: the worst outcome of a bad automated
release is a bad file in a directory that a rollback replaces in one command.

## 5. Files to Read First

    DEPLOYMENT.md
    RELEASE.md
    ROLLBACK.md
    scripts/make-release-index.sh
    scripts/smoke-test.sh

## 6. Expected Changed Files

    scripts/build.sh
    scripts/make-release-index.sh
    scripts/smoke-test.sh
    scripts/release.sh
    scripts/rollback.sh
    RELEASE.md
    ROLLBACK.md
    DEPLOYMENT.md
    docs/runbooks/release-rollback.md

## 7. Interfaces and Contracts

An artifact is named `powderburn-<version>-<target>.tar.zst` on Unix targets and
`powderburn-<version>-<target>.zip` on Windows. Every artifact has a sibling `.sha256` and a sibling
`.sig`. The release index is `index.txt` in `PB_RELEASE_DIR` with one line per artifact:
`<version> <target> <sha256> <bytes> <iso8601>`. The index is append-only; a correction is a new line
with a new version, never an edit.

## 8. Milestones

### M1: Reproducible builds
GOAL: Two builds of the same commit produce identical bytes.
READ: ENVIRONMENT.md, scripts/build.sh
CHANGE: scripts/build.sh, Cargo.toml
CONTENT: set `SOURCE_DATE_EPOCH` from the commit timestamp, pass `--remap-path-prefix` for the source
root and the vendor directory, sort every archive member list before adding, and set every archive
member's mtime to `SOURCE_DATE_EPOCH`. Build with `--locked --offline` always.
RUN:
    sh scripts/build.sh --release --out "$PB_CACHE_DIR/a"
    sh scripts/build.sh --release --out "$PB_CACHE_DIR/b"
    a=$(sha256sum "$PB_CACHE_DIR/a"/*.tar.zst | awk '{print $1}'); b=$(sha256sum "$PB_CACHE_DIR/b"/*.tar.zst | awk '{print $1}')
    [ "$a" = "$b" ] && echo "reproducible: identical" || { echo "reproducible: DIFFER"; exit 1; }
EXPECT: `reproducible: identical`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M1 reproducible: identical"
FALLBACK: if a dependency embeds a build timestamp that cannot be suppressed, name it in
DEPLOYMENT.md as a known reproducibility exception, exclude that one file from the comparison, and
open a Decision Log entry. Do not abandon reproducibility for the rest of the archive.
COMMIT: git add -A && git commit -m "[EP-009][M1] reproducible release builds"

### M2: Checksums, signatures, and the index
GOAL: Every artifact is verifiable by a stranger with a public key.
READ: DEPLOYMENT.md, scripts/make-release-index.sh
CHANGE: scripts/release.sh, scripts/make-release-index.sh
CONTENT: `release.sh` builds all three targets, writes `.sha256` beside each artifact, signs each with
the key at `PB_RELEASE_SIGNING_KEY`, verifies its own signature immediately after writing it, and
appends to `index.txt`. If the signing key is absent, the script fails loudly rather than producing
unsigned artifacts.
RUN:
    sh scripts/release.sh --version 0.1.0-rc1 --dry-run
    sh scripts/make-release-index.sh --check
EXPECT: `release: dry-run ok` then `index: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M2 index: ok"
FALLBACK: none needed; PREFLIGHT already proved the key is present and usable.
COMMIT: git add -A && git commit -m "[EP-009][M2] checksums, signatures, release index"

### M3: Smoke test the released artifact, not the build tree
GOAL: What ships is what is tested.
READ: scripts/smoke-test.sh
CHANGE: scripts/smoke-test.sh
CONTENT: `--released` unpacks the artifact into a scratch directory, sets `PB_HOME` to a fresh path,
and runs against the unpacked binary only: `--version`, `selftest`, a headless capture, a campaign
creation, one mission played from a journal, and a save round trip. It must never fall back to
`cargo run`; if the unpacked binary is missing, that is the failure, not a reason to substitute.
RUN:
    sh scripts/smoke-test.sh --released "$PB_CACHE_DIR/a/powderburn-0.1.0-rc1-x86_64-unknown-linux-gnu.tar.zst"
EXPECT: `smoke-test: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M3 smoke-test: ok"
FALLBACK: none. A smoke test that silently tests the build tree instead of the artifact is worse than
no smoke test, because it produces false confidence.
COMMIT: git add -A && git commit -m "[EP-009][M3] smoke test the released artifact"

### M4: Publish to the self-hosted release directory
GOAL: The automated deployment target works and is idempotent.
READ: DEPLOYMENT.md, PREFLIGHT.md
CHANGE: scripts/release.sh
CONTENT: publication copies artifacts, checksums, and signatures into `PB_RELEASE_DIR/<version>/`,
appends to `index.txt`, and refuses to overwrite an existing version directory, because a released
version is immutable. Re-running the publish for the same version is a no-op that prints
`publish: already present` and exits zero, so a retried run is safe.
RUN:
    sh scripts/release.sh --version 0.1.0-rc1 --publish
    sh scripts/release.sh --version 0.1.0-rc1 --publish
EXPECT: `publish: ok` then `publish: already present`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M4 publish idempotent"
FALLBACK: if `PB_RELEASE_DIR` is not writable, halt with a blocked report naming the path and the
permission. Never publish somewhere else.
COMMIT: git add -A && git commit -m "[EP-009][M4] publish to the self-hosted release directory"

### M5: Rehearse the rollback
GOAL: Going backwards is proven to work before it is needed.
READ: ROLLBACK.md, docs/runbooks/release-rollback.md
CHANGE: scripts/rollback.sh, ROLLBACK.md
CONTENT: publish `0.1.0-rc2` as a deliberately broken artifact whose `selftest` exits non-zero. Detect
the failure with `smoke-test.sh --released`. Run `rollback.sh --to 0.1.0-rc1`, which repoints the
`current` symlink, appends a rollback line to `index.txt`, and re-runs the smoke test against the
restored version. Then append the exact literal line `ROLLBACK DRILL COMPLETED` to
`.agent/state/LEDGER.md` through `ledger.sh`, because SPEC-008 makes that line a ship criterion.
RUN:
    sh scripts/release.sh --version 0.1.0-rc2 --publish --inject-failure
    sh scripts/smoke-test.sh --released "$PB_RELEASE_DIR/0.1.0-rc2/powderburn-0.1.0-rc2-x86_64-unknown-linux-gnu.tar.zst" || echo "detected: broken release"
    sh scripts/rollback.sh --to 0.1.0-rc1
    sh scripts/smoke-test.sh --released "$PB_RELEASE_DIR/current/powderburn-0.1.0-rc1-x86_64-unknown-linux-gnu.tar.zst"
    sh scripts/ledger.sh append <AGENT_ID> EP-009 NOTE "ROLLBACK DRILL COMPLETED"
EXPECT: `detected: broken release`, `rollback: ok`, `smoke-test: ok`, and the literal line present in
the ledger
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M5 rollback drill complete"
FALLBACK: none. An unrehearsed rollback is a plan, not a capability, and SPEC-008 requires the
capability.
COMMIT: git add -A && git commit -m "[EP-009][M5] rehearsed rollback drill"

### M6: Document the manual itch.io step
GOAL: The one thing an agent must never do is written down clearly enough that it never happens by
accident.
READ: DECISIONS.md ADR-0012, RELEASE.md
CHANGE: RELEASE.md, DEPLOYMENT.md
CONTENT: RELEASE.md ends with a section headed `MANUAL STEP: itch.io publication` containing the exact
butler command with placeholders, and the sentence stating that no script in this repository runs it
and no agent may run it. `release.sh` prints that command and exits without executing it.
RUN:
    grep -q "MANUAL STEP: itch.io publication" RELEASE.md && echo "manual step documented"
    grep -rn "butler push" scripts/ | grep -v "echo" | wc -l
EXPECT: `manual step documented` and a count of 0
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-009 MILESTONE_PASS "M6 manual step documented, count 0"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-009][M6] document the manual itch.io publication step"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Builds are reproducible | M1 comparison | `reproducible: identical` |
| Artifacts are signed and indexed | `sh scripts/make-release-index.sh --check` | `index: ok` |
| The artifact itself passes smoke | `sh scripts/smoke-test.sh --released` | `smoke-test: ok` |
| Publication is idempotent | M4 twice | `publish: already present` |
| Rollback is rehearsed | M5 | `ROLLBACK DRILL COMPLETED` in the ledger |
| No script pushes to itch.io | M6 grep | count 0 |
| Whole node | `sh scripts/verify.sh` | `verify: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-008` for the repository. `PB_RELEASE_DIR` is external state: remove the
`0.1.0-rc1` and `0.1.0-rc2` directories and the lines they added to `index.txt` before re-entering,
and note the cleanup in the ledger so the append-only index's history stays honest.

## 11. Progress
- [ ] M1 Reproducible builds
- [ ] M2 Checksums, signatures, and the index
- [ ] M3 Smoke test the released artifact, not the build tree
- [ ] M4 Publish to the self-hosted release directory
- [ ] M5 Rehearse the rollback
- [ ] M6 Document the manual itch.io step

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
