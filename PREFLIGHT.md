# PREFLIGHT - POWDERBURN

Read this file first. Obtain every REQUIRED item below, copy `.env.example` to `.env`, fill it,
then run `sh scripts/preflight.sh` until it prints `preflight: ok`.

This is the ONLY interactive moment in the life of a run. After `preflight: ok` appears, no node of
the graph will ever require a credential, an account, a payment, a permission, or a human answer.

POWDERBURN is a self-hosted, offline, zero-network product. It has no cloud services, no telemetry
sink, no analytics vendor, no auth provider, and no paid API. Consequently this preflight is almost
entirely toolchain and filesystem verification rather than credential collection. That is by design
and is enforced at runtime by LBI-09 (NO NETWORK).

## 1. Toolchain

### RUSTUP_TOOLCHAIN
- Purpose: pins the exact Rust compiler for every node. Consumed by EP-000 through EP-010.
- Value: `1.85.0`
- Type: toolchain identifier, not a secret.
- Obtain: install rustup from your distribution package manager or from the offline rustup archive
  you already keep in your workshop, then run `rustup toolchain install 1.85.0` and
  `rustup component add rustfmt clippy --toolchain 1.85.0`. If you build Rust from source, place a
  1.85.0 toolchain on PATH and set this variable to `host`.
- Cost: free.
- Probe: `scripts/probes/rustup_toolchain.sh`
- Fallback: none. REQUIRED. A different compiler version changes lint output and breaks the pinned
  gate sentinels.

### PB_CARGO_OFFLINE
- Purpose: forces `--offline` on every cargo invocation so the build never reaches a registry.
  Consumed by every node.
- Value: `1`
- Obtain: set it. Then run `cargo vendor` once at EP-001 milestone M2, which writes `vendor/` and
  `.cargo/config.toml`. From that point the repository builds with no network at all.
- Probe: `-` (presence only)
- Fallback: none. REQUIRED.

## 2. Filesystem roots

### PB_HOME
- Purpose: absolute path to the repository root. Used by scripts and by the headless simulator to
  resolve content and golden files without relying on the current working directory.
- Obtain: `pwd` at the repository root.
- Probe: `scripts/probes/pb_home.sh`
- Fallback: none. REQUIRED.

### PB_ASSET_ROOT
- Purpose: absolute path to the authored asset tree (sprites, tilesets, audio, fonts). Consumed by
  EP-005, EP-009, EP-010 and by live-fire proofs LF-08 and LF-09.
- Obtain: `"$PB_HOME/assets"`. EP-001 M4 creates this directory and its PROVENANCE.toml.
- Probe: `scripts/probes/pb_asset_root.sh`
- Fallback: none. REQUIRED.

### PB_GOLDEN_DIR
- Purpose: absolute path to the golden-state corpus used by determinism and replay proofs.
  Consumed by EP-002 onward and by live-fire proofs LF-01 through LF-07.
- Obtain: `"$PB_HOME/tests/golden"`. EP-002 M1 creates it.
- Probe: `scripts/probes/pb_golden_dir.sh`
- Fallback: none. REQUIRED.

### PB_CACHE_DIR
- Purpose: scratch directory for build intermediates, frame captures, and replay dumps. Never
  committed.
- Obtain: `"$PB_HOME/.pbcache"`.
- Probe: `-`
- Fallback: none. REQUIRED.

## 3. Headless rendering

### PB_HEADLESS_ADAPTER
- Purpose: names the wgpu adapter backend used by the headless frame-capture proof LF-08 so that
  the renderer can be verified with no display attached. Consumed by EP-005 and EP-010.
- Value: `gl` on a machine with Mesa llvmpipe, or `vulkan` on a machine with lavapipe.
- Obtain: install `mesa-utils` and `libgl1-mesa-dri` (llvmpipe) or `mesa-vulkan-drivers` (lavapipe)
  from your distribution. Verify with `LIBGL_ALWAYS_SOFTWARE=1 glxinfo -B` or `vulkaninfo --summary`.
- Cost: free.
- Probe: `scripts/probes/pb_headless_adapter.sh`
- Fallback: none. REQUIRED. Without a software adapter the renderer cannot be proven non-interactively
  and EP-005 has no exit evidence.

## 4. Release signing and publication

### PB_RELEASE_SIGNING_KEY
- Purpose: absolute path to a minisign or age secret key used to sign release tarballs. Consumed by
  EP-009 and EP-010.
- Type: path to a secret key file. The key material itself is never read into the environment and
  never logged.
- Obtain: `minisign -G -s ~/.keys/powderburn.key -p ~/.keys/powderburn.pub` (package `minisign`).
  Keep the secret key outside the repository. Set this variable to the secret key path.
- Minimum scope: a signing key used for nothing else.
- Cost: free.
- Probe: `scripts/probes/pb_release_signing_key.sh`
- Fallback: none. REQUIRED. Unsigned artifacts fail the ship gate.

### PB_RELEASE_DIR
- Purpose: absolute path to the self-hosted release directory that the hands-off deploy step writes
  into. Consumed by EP-009 and EP-010.
- Obtain: create a directory on the host that serves your static site, for example
  `/srv/releases/powderburn`, and make it writable by the account running the graph.
- Probe: `scripts/probes/pb_release_dir.sh`
- Fallback: none. REQUIRED. AUTO_DEPLOY is authorized for this target only.

### PB_ITCH_API_KEY
- Purpose: optional third-party publication of the same signed artifacts through butler.
- Type: API key, scope `upload` only.
- Obtain: itch.io account settings, API keys, generate new key.
- Cost: free.
- Probe: `scripts/probes/pb_itch_api_key.sh`
- Fallback: OPTIONAL. If unset, EP-010 emits the exact butler command as a MANUAL step and the run
  still completes. External publication is never performed hands-off.

## 5. Required host tools (checked by preflight, no variable)

`git`, `awk`, `grep`, `sed`, `sh`, `tar`, `zstd`, `cargo`, `rustc`, `rustup`, `minisign`, `sha256sum`,
`python3` (asset packing only, never in the shipped product).

Minimum versions asserted by `scripts/preflight.sh`: rustc 1.85.0 exactly, git 2.30 or newer,
zstd 1.5 or newer, python3 3.10 or newer.

## 6. Explicit non-needs

No database server. No message broker. No container runtime. No cloud account. No payment processor.
No email provider. No error-tracking vendor. No CDN. No package registry after `cargo vendor` runs.
If any node ever appears to need one of these, that is a generation defect: record it, take the
node fallback, or block with a report naming this section.

PREFLIGHT-TABLE-BEGIN
RUSTUP_TOOLCHAIN|REQUIRED|scripts/probes/rustup_toolchain.sh
PB_CARGO_OFFLINE|REQUIRED|-
PB_HOME|REQUIRED|scripts/probes/pb_home.sh
PB_ASSET_ROOT|REQUIRED|scripts/probes/pb_asset_root.sh
PB_GOLDEN_DIR|REQUIRED|scripts/probes/pb_golden_dir.sh
PB_CACHE_DIR|REQUIRED|-
PB_HEADLESS_ADAPTER|REQUIRED|scripts/probes/pb_headless_adapter.sh
PB_RELEASE_SIGNING_KEY|REQUIRED|scripts/probes/pb_release_signing_key.sh
PB_RELEASE_DIR|REQUIRED|scripts/probes/pb_release_dir.sh
PB_ITCH_API_KEY|OPTIONAL|scripts/probes/pb_itch_api_key.sh
PREFLIGHT-TABLE-END
