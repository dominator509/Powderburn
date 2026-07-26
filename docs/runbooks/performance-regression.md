# Runbook: Performance Regression

## Symptom

- Frame rate drops below 30 FPS on the reference hardware.
- Turn computation takes noticeably longer than previous releases.
- CI benchmarks show a >10% regression in a tracked metric.
- Memory usage grows unboundedly during a session.
- Load times increase significantly.

## Immediate Check

1. **Check the current FPS / turn time:**
   ```sh
   powderburn --bench 2>&1 | grep -E "fps|turn_ms|avg"
   ```

2. **Compare against baseline:**
   ```sh
   # If a baseline file exists:
   cat "$PB_HOME/benchmarks/baseline.json"
   ```

3. **Check for resource exhaustion:**
   ```sh
   top -b -n1 -p "$(pgrep powderburn)" | tail -1
   free -m
   ```

4. **Check the log for performance warnings:**
   ```sh
   grep -i "slow\|warn.*ms\|leak\|allocation" "$PB_HOME/logs/powderburn.log" | tail -20
   ```

## Diagnosis Steps

### 1. Profile the binary

Build with profiling symbols and run `perf`:

```sh
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p powderburn
perf record -g ./target/release/powderburn --bench
perf report -g graph --stdio | head -80
```

This shows the hottest functions.

### 2. Run the micro-benchmark suite

```sh
cargo bench 2>&1 | tee /tmp/bench_results.txt
```

Compare against the last known-good run:

```sh
# If previously saved:
diff <(grep "time:" /tmp/bench_results.txt) <(grep "time:" /tmp/bench_baseline.txt)
```

### 3. Identify the regression window

```sh
git bisect start
git bisect bad HEAD
git bisect good <last-known-good-tag>
# For each candidate:
cargo bench 2>&1 | grep "main_loop" | grep -oP '[\d.]+' | head -1
git bisect <good|bad>
```

### 4. Check for common performance pitfalls

| Pattern | Impact | Tool |
|---------|--------|------|
| Unbounded `Vec::push` in a hot loop | O(n) per frame | `perf` / `valgrind --tool=callgrind` |
| `clone()` on large structs | Alloc churn | `perf` / `heaptrack` |
| HashMap resizing | Latency spikes | `dhat` (heap profiling) |
| Serialization in hot path | CPU waste | `perf` / flamegraph |
| Lock contention | Thread stalls | `perf lock` |

Search for common regressions:

```sh
grep -rn "\.clone()" crates/pb-sim/src/ --include='*.rs' | wc -l
grep -rn "HashMap::new\|push(" crates/pb-sim/src/ --include='*.rs'
```

### 5. Heap profiling

```sh
# Install and run with dhat
cargo install dhat
CARGO_PROFILE_RELEASE_DEBUG=true cargo run --release -p powderburn -- --dhat-heap
# Look for the dhat-heap.json output
```

### 6. Check for O(n²) algorithms

Search for nested loops over game entities:

```sh
grep -rn "for.*in.*\.iter()" crates/pb-sim/src/ --include='*.rs' | head -20
```

Loops over all entities inside other loops over all entities are the most
common source of quadratic blow-up.

## Resolution

1. **Optimise the hot path:** Use the profiling data to identify the top
   3 functions consuming CPU and optimise them (e.g. cache spatial queries,
   pre-allocate vectors, use `SmallVec` for small collections).
2. **Reduce allocations:** Replace `Vec` with `arrayvec` or `SmallVec` for
   fixed-size collections.  Use `String::with_capacity` where size is known.
3. **Fix memory leak:** Ensure every `Arc`/`Rc` cycle is broken.  Drop
   temporary state at turn boundaries.
4. **Add a benchmark regression test:** Commit a benchmark that alerts CI
   on >5% regression.

## Prevention

- CI runs `cargo bench --compare=baseline.json` before every release.
- A performance budget is defined in `ARCHITECTURE.md`: target 60 FPS on
  reference hardware, max 100ms per AI turn.
- All PRs with `crates/pb-sim/` changes must include before/after bench
  numbers.
- Monthly profiling review with `perf` and flamegraphs.
