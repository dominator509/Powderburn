# ROLLBACK

## Triggers

Roll back immediately, without debate, on any of these:

1. The published artifact's signature does not verify from a clean download.
2. `pbcli selftest` fails on the published artifact on any machine.
3. A determinism regression is confirmed in a released build, meaning the same seed and journal
   produce different terminal hashes across runs or machines.
4. A save-corrupting defect is confirmed: any path where loading a valid save loses or alters state.
5. A crash reproducible within the first mission.
6. An asset ships whose provenance turns out to be wrong, or content ships that violates the
   representation law in SPEC-000 section 7.

Trigger 6 is not a lesser trigger. It gets the same response as a crash.

## Decision owner

The operator. Single operator project. The decision is made with evidence in hand, not by consensus,
and it is recorded in the ledger.

## Application rollback

There is no server, so rollback is republication of the previous artifact as current.

    set -eu
    . ./.env
    PREV=<previous version>
    [ -f "$PB_RELEASE_DIR/$PREV/powderburn-$PREV-x86_64-linux.tar.zst" ] || exit 1
    minisign -Vm "$PB_RELEASE_DIR/$PREV/powderburn-$PREV-x86_64-linux.tar.zst" -p "$PB_RELEASE_DIR/powderburn.pub"
    printf '%s\n' "$PREV" > "$PB_RELEASE_DIR/CURRENT"
    sh scripts/make-release-index.sh "$PB_RELEASE_DIR"
    sh scripts/smoke-test.sh --released

Nothing is deleted. The bad version stays on disk, marked in the index as withdrawn, because someone
already downloaded it and will ask about it.

## Database rollback

Not applicable. There is no database. This line exists so that during an incident nobody spends
twenty minutes looking for one.

## Configuration rollback

Player configuration lives on player machines and is never touched. Build configuration is in git;
`git revert` the offending commit, never `reset --hard` on a published tag.

## Feature flag rollback

There are no runtime feature flags in the shipped product, by design. A feature is in the build or it
is not. Rolling back a feature means rolling back the release.

## Verification after rollback

1. `minisign -Vm` on the now-current artifact passes.
2. `sh scripts/smoke-test.sh --released` prints `smoke: ok`.
3. The index lists the previous version as current and the withdrawn version as withdrawn.
4. A fresh download, unpack, and `pbcli selftest` on a machine that is not the build machine.

## Communication

Update `$PB_RELEASE_DIR/<withdrawn>/NOTES.md` with a Withdrawn heading naming the trigger, the date,
and what a player who already installed it should do. Then the same text wherever the game is
listed. Plain language, no euphemism, no minimization.

## Postmortem

Within a week, into DECISIONS.md as an ADR: what shipped, what gate should have caught it, and the
specific new gate or test that now does. A postmortem that ends in "be more careful" is not finished.
Every rollback in this project ends with a new line in `scripts/verify.sh` or a new row in the
TESTING.md matrix.

## The drill

EP-009 performs a real rollback drill against `$PB_RELEASE_DIR/staging/` with two versions published,
executes the steps above, verifies, and appends `ROLLBACK DRILL COMPLETED` to the ledger. The ship
gate checks for that string.
