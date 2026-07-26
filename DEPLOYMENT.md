# DEPLOYMENT

## Environments

| Name | What it is | Who writes it |
| --- | --- | --- |
| local | The build tree | The run |
| staging | `$PB_RELEASE_DIR/staging/` | The run, during EP-009 |
| production | `$PB_RELEASE_DIR/<version>/` plus the release index | The run, during EP-010, hands-off |
| external | itch.io | A human, MANUAL, never the run |

## Deployment architecture

There is no server to deploy. Deployment means: build reproducibly, package, sign, copy into the
self-hosted release directory, regenerate the index, and smoke test the published artifact by
verifying its signature, unpacking it, and running it. Rollback means republishing the previous
artifact and regenerating the index, which is why nothing is ever deleted from the release directory.

## Build artifact definition

`powderburn-<version>-x86_64-linux.tar.zst` containing:

    powderburn-<version>/
      bin/powderburn
      bin/pbcli
      content/
      assets/
      LICENSE
      BIBLIOGRAPHY.md
      REFERENCE_MACHINE.txt
      SHA256SUMS

Accompanied by `powderburn-<version>-x86_64-linux.tar.zst.minisig`.

Reproducibility: `SOURCE_DATE_EPOCH` is the commit timestamp, `RUSTFLAGS` carries
`--remap-path-prefix`, tar entries are sorted with fixed ownership and mtime. Two builds of the same
commit must produce identical sha256 sums; EP-009 asserts this by building twice.

## Release flow

1. Ship gate passes: clean tree, `verify: ok`, `production-readiness: ok`.
2. `git tag -a v<version> -m "POWDERBURN v<version>"`.
3. `sh scripts/build.sh` prints `build: ok`.
4. Package, sign, and publish per the exact steps below.
5. `sh scripts/smoke-test.sh --released` prints `smoke: ok`.
6. Append RUN_COMPLETE with the tag.

## Exact deploy steps (hands-off; AUTO_DEPLOY authorized for PB_RELEASE_DIR only)

    set -eu
    . ./.env
    VER=$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/pb-app/Cargo.toml | head -n 1)
    STAGE="$PB_CACHE_DIR/pkg/powderburn-$VER"
    rm -rf "$PB_CACHE_DIR/pkg" && mkdir -p "$STAGE/bin"
    cp target/release/powderburn target/release/pbcli "$STAGE/bin/"
    cp -r content assets LICENSE content/BIBLIOGRAPHY.md "$STAGE/"
    cp "$PB_CACHE_DIR/REFERENCE_MACHINE.txt" "$STAGE/"
    ( cd "$STAGE" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS )
    TAR="$PB_CACHE_DIR/pkg/powderburn-$VER-x86_64-linux.tar.zst"
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
        --use-compress-program='zstd -19' -cf "$TAR" -C "$PB_CACHE_DIR/pkg" "powderburn-$VER"
    minisign -Sm "$TAR" -s "$PB_RELEASE_SIGNING_KEY"
    mkdir -p "$PB_RELEASE_DIR/$VER"
    cp "$TAR" "$TAR.minisig" "$PB_RELEASE_DIR/$VER/"
    sh scripts/make-release-index.sh "$PB_RELEASE_DIR"
    sh scripts/smoke-test.sh --released

## Migration steps

There is no database. The only migration concern is the save `format_version`. A release that bumps
it ships a refusal message naming both versions and a line in the release notes. There is no
automatic save migration, by ADR-0006 and LBI-08.

## Rollback steps

See ROLLBACK.md. Summary: re-run `scripts/make-release-index.sh` with the previous version marked
current. No artifact is ever deleted.

## Post-deploy smoke

`sh scripts/smoke-test.sh --released` verifies the signature, unpacks the artifact into the cache,
runs `powderburn --version` and `pbcli selftest --emit-hash`, and requires `selftest: ok` plus a state
hash line.

## Deployment STOP conditions

Stop and report if: the signature does not verify; the two reproducibility builds differ; the release
directory is not writable; a version directory already exists with different contents; or the deploy
would overwrite a previously published artifact.

## External publication (MANUAL, never executed by the run)

    butler push "$PB_RELEASE_DIR/$VER/powderburn-$VER-x86_64-linux.tar.zst" <user>/powderburn:linux --userversion "$VER"

The run prints this line and stops clean.

## Production verification commands

    minisign -Vm "$PB_RELEASE_DIR/$VER/powderburn-$VER-x86_64-linux.tar.zst" -p "$PB_RELEASE_DIR/powderburn.pub"
    sha256sum -c "$PB_RELEASE_DIR/$VER/SHA256SUMS"
    sh scripts/smoke-test.sh --released
