# ENVIRONMENT

## Required tools and exact versions (mirrors scripts/preflight.sh)

| Tool | Required version | Why |
| --- | --- | --- |
| rustc | exactly 1.85.0 | Lint output and codegen must be pinned for determinism and stable gate sentinels |
| cargo | ships with 1.85.0 | Build and test driver |
| rustup | any | Toolchain pinning |
| git | 2.30 or newer | Worktree and tag behavior relied on by the rollback protocol |
| zstd | 1.5 or newer | Release artifact compression |
| tar, awk, grep, sed, sh | POSIX | Scripts |
| sha256sum | coreutils | Artifact hashing |
| minisign | any | Artifact signing and verification |
| python3 | 3.10 or newer | Atlas packing only; never in the shipped product |
| A software graphics adapter | Mesa llvmpipe or lavapipe | Headless renderer proof |
| butler | optional | Optional external publication only |

## Environment variable reference

| Name | Required | Environment | Example | Secret | Description | Validation |
| --- | --- | --- | --- | --- | --- | --- |
| RUSTUP_TOOLCHAIN | yes | build | 1.85.0 | no | Pins the compiler | probe asserts rustc 1.85.0 |
| PB_CARGO_OFFLINE | yes | build | 1 | no | Forces offline cargo | presence |
| PB_HOME | yes | all | /srv/src/powderburn | no | Repository root | absolute, contains AGENTS.md |
| PB_ASSET_ROOT | yes | build, test | $PB_HOME/assets | no | Authored assets | absolute, parent writable |
| PB_GOLDEN_DIR | yes | test | $PB_HOME/tests/golden | no | Golden corpus | absolute, parent writable |
| PB_CACHE_DIR | yes | build, test | $PB_HOME/.pbcache | no | Scratch | absolute |
| PB_HEADLESS_ADAPTER | yes | test | gl | no | wgpu backend for capture | gl or vulkan, software adapter present |
| PB_RELEASE_SIGNING_KEY | yes | release | ~/.keys/powderburn.key | yes | minisign secret key | file exists, mode 600 or 400, outside repo |
| PB_RELEASE_DIR | yes | release | /srv/releases/powderburn | no | Self-hosted publish target | absolute, exists, writable |
| PB_ITCH_API_KEY | no | release | (empty) | yes | Optional external publish | length at least 20, butler present |
| PB_LOG | no | runtime | pb_sim=debug | no | Log filter | standard directive syntax |
| PB_CONFIG_DIR | no | runtime | ~/.config/powderburn | no | Player settings and saves | absolute, writable |

This table and PREFLIGHT.md must agree exactly. A variable in one and not the other is a defect.

## Local setup

    git clone <repo> && cd powderburn
    rustup toolchain install 1.85.0
    rustup component add rustfmt clippy --toolchain 1.85.0
    cp .env.example .env    # then edit every REQUIRED value
    sh scripts/preflight.sh # must print preflight: ok
    sh scripts/install.sh   # vendors dependencies, prints install: ok
    sh scripts/verify.sh    # full gate chain, prints verify: ok

## Test environment

Identical to local. Tests never require a display, a network, or elevated privileges. Every test that
writes uses `$PB_CACHE_DIR/test/<name>/`.

## Staging

There is no staging service. The staging equivalent is a build published into
`$PB_RELEASE_DIR/staging/` and smoke tested with `sh scripts/smoke-test.sh --released` before the
release directory index is updated. This is drilled in EP-009.

## Production

Production is the player's machine. The only production configuration is the shipped defaults plus
`$PB_CONFIG_DIR`. Config validation: every field range checked at read; an out-of-range value is
replaced by the default and logged at WARN with the field name.

## The reference machine

Performance budgets in SPEC-008 are defined against the machine the release is cut on, recorded at
release time in `$PB_RELEASE_DIR/<version>/REFERENCE_MACHINE.txt` containing CPU model, core count,
RAM, GPU, kernel version, and Mesa version. A budget failure on a different machine is not a
regression; a budget failure on the reference machine is.

## Parity rules

The same `verify.sh` runs everywhere. There is no environment-specific test path, no CI-only skip,
and no local-only shortcut. If a gate cannot run somewhere, that place is not a supported environment.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `preflight: FAIL - rustc must be exactly 1.85.0` | Wrong default toolchain | `rustup override set 1.85.0` in the repository |
| `install: FAIL - toolchain 1.85.0 not installed` | Missing toolchain | `rustup toolchain install 1.85.0` |
| cargo tries to reach the network | `.cargo/config.toml` missing or vendor absent | `sh scripts/install.sh` |
| `probe failed: PB_HEADLESS_ADAPTER` | No software adapter | install `libgl1-mesa-dri` and `mesa-utils`, or `mesa-vulkan-drivers` |
| `capture` fails with E-RENDER-001 | Adapter name does not match the installed driver | set PB_HEADLESS_ADAPTER to the other value |
| `probe failed: PB_RELEASE_SIGNING_KEY` | Key inside the repository or wrong mode | move it out, `chmod 600` |
| Determinism drift after a dependency change | A dependency introduced a float or a hash map in a hot path | `pbcli sim --trace-actor`, then `scripts/lint-determinism.sh` |
