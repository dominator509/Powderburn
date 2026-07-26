# Runbook: Desync or Non-determinism

## Symptom

- Two runs of POWDERBURN with identical inputs (same seed, same actions,
  same environment) produce different outcomes.
- The determinism test suite (`determinism` crate) reports mismatches.
- Multiplayer or replay mode shows desynchronisation between peers.
- Reproducibility CI jobs fail intermittently.

## Immediate Check

1. **Re-run the test deterministically:**
   ```sh
   cargo test --test determinism -- --test-threads=1 2>&1 | tee /tmp/det.log
   ```
   A single-threaded run eliminates thread-interleaving variance.

2. **Check the seed is actually fixed:**
   ```sh
   grep "rng_seed\|deterministic_seed" "$PB_HOME/logs/powderburn.log" | tail -5
   ```
   The seed must be the same across runs.  If `PB_RNG_SEED` is unset, the
   RNG seeds from system entropy.

3. **Compare two run logs:**
   ```sh
   diff <(grep "^log:" run1.log | cut -d' ' -f3-) <(grep "^log:" run2.log | cut -d' ' -f3-)
   ```
   The first difference pinpoints the divergence point.

## Diagnosis Steps

### 1. Isolate the non-deterministic component

The POWDERBURN architecture separates determinism-critical code:
- `pb_rng` — deterministic seeded RNG
- `pb_sim` — simulation kernel (should be deterministic given seed)
- `pb_audio` — audio output (may use real-time clock)

Mask non-deterministic subsystems:

```sh
PB_DISABLE_AUDIO=1 cargo test --test determinism
```

If the test passes with audio disabled, the non-determinism is in the
audio subsystem or its interaction with the sim.

### 2. Identify unseeded randomness

Search for uses of non-deterministic sources:

```sh
grep -rn "thread_rng\|OsRng\|SystemTime::now\|Instant::now" crates/ --include='*.rs'
```

Every use of `thread_rng()` or wall-clock time in a determinism-critical
path is a bug.  `Instant::now` is acceptable for elapsed wall-clock timing
but must never feed back into game state.

### 3. Leak hash and metric values at every step

Enable metric emission to trace every numeric decision:

```sh
PB_METRICS=1 powderburn [args] 2>&1 | grep "^metric:" > /tmp/metrics1.txt
```

Compare metrics across runs:

```sh
diff /tmp/metrics1.txt /tmp/metrics2.txt
```

### 4. Check for HashMap / HashSet iteration order

Rust's `HashMap` and `HashSet` have non-deterministic iteration order
across processes (they use a randomized hasher by default).  Search for:

```sh
grep -rn "HashMap\|HashSet" crates/ --include='*.rs' | grep -v "use std::collections"
```

Any iteration that affects game state must use `BTreeMap`/`BTreeSet`
or `IndexMap`/`IndexSet` (preserves insertion order).

### 5. Check for floating-point accumulation

Floating-point operations are deterministic on the same CPU but can differ
across architectures or with different compiler optimisations.  If the
determinism test must pass across platforms, use fixed-point (`Fix32`)
for all game-state arithmetic.

```sh
grep -rn "f32\|f64" crates/pb-sim/src/ --include='*.rs'
```

## Resolution

1. **Unseeded RNG:** Replace `thread_rng()` with the deterministic
   `pb_rng::SeededRng` crate.
2. **Wall-clock feedback:** Remove `SystemTime::now` and `Instant::now`
   from game-state computation.
3. **HashMap order:** Convert to `BTreeMap`/`BTreeSet` for any map that
   is iterated to produce game-state output.
4. **Floating point:** Convert non-deterministic `f32`/`f64` accumulation
   to `Fix32` from `pb-core`.
5. **Threading race:** Add a mutex or use `std::sync::atomic` operations
   on shared state.  Re-run with `--test-threads=1` to confirm.

## Prevention

- CI must run `cargo test --test determinism -- --test-threads=1` on every
  PR and before every release.
- All game-state RNG use goes through `pb_rng::SeededRng`.
- `HashMap`/`HashSet` are banned in game-state paths; linters check for
  this.
- The `determinism` crate has regression tests for every known-fixed
  non-determinism bug.
- Every release is built from a known commit with `--release` and the
  same compiler version.
