# Runbook: Crash Report Handling

## Symptom

- POWDERBURN process exits unexpectedly with a non-zero exit code.
- Operating system crash dialog appears.
- A crash dump or panic message appears on stderr.
- The log file (`$PB_HOME/logs/powderburn.log`) ends abruptly with a panic
  message or no final log line.

## Immediate Check

1. **Check the log tail:**
   ```sh
   tail -50 "$PB_HOME/logs/powderburn.log"
   ```
   Look for the last `event=` field and any `panicked at` message.

2. **Check last log for panic indicator:**
   A Rust panic prints `thread '<main>' panicked at` followed by a source
   location.  The log event immediately preceding the panic is the trigger.

3. **Capture the full crash context:**
   ```sh
   journalctl -u powderburn --since "5 minutes ago" --no-pager 2>/dev/null \
     || echo "no journald"
   ```
   On systems without journald, check the terminal output where POWDERBURN
   was launched.

4. **Check resource limits:**
   ```sh
   dmesg | grep -i "oom\|killed\|powderburn" | tail -5
   ```
   An OOM kill leaves no Rust panic — the process vanishes silently.

## Diagnosis Steps

### 1. Reproduce with verbose logging

```
PB_LOG=debug powderburn [usual arguments] 2>&1 | tee crash-replay.log
```

If the crash is deterministic the reproduction will produce the same panic
message.  If it is intermittent, run in a loop:

```sh
for i in $(seq 1 20); do
  echo "=== RUN $i ==="
  PB_LOG=debug timeout 30 powderburn [args] 2>&1 || true
done | tee crash-loop.log
```

### 2. Classify the crash type

| Pattern | Likely Cause | Action |
|---------|-------------|--------|
| `index out of bounds` | Array/vec access without bounds check | Fix indexing logic or add `get()` |
| `unwrap()` on `None` | Missing match on `Option` | Replace with `unwrap_or` / proper match |
| `unwrap()` on `Err` | Unhandled `Result` | Propagate error with `?` or match |
| `assertion failed` | Invariant broken | Investigate why invariant failed |
| OOM kill | Memory leak or unbounded allocation | Profile with `valgrind --tool=massif` |
| SIGSEGV / segfault | Unsound `unsafe` code (forbidden) | Gated by `#![forbid(unsafe_code)]` — inspect FFI |

### 3. Identify the code location

The panic contains a file path and line number matching the release binary.
Map it to source:

```sh
# If release binary was built from a known commit:
git log --oneline -1 <commit>
# The panic line tells you the exact location.
```

### 4. Check for recent changes

```sh
git log --oneline -20
git diff HEAD~5..HEAD -- crates/
```

Rollback the most recent change if the crash is new and the diff touches the
panicking module.

## Resolution

1. **File a bug:** Log a `NODE_BLOCKED` event in the ledger with the panic
   message, commit hash, and reproduction steps.
2. **Hotfix:** If the crash blocks a release, revert the offending commit and
   re-run the release pipeline.
3. **Permanent fix:** Add a test that reproduces the panic, fix the root
   cause, and verify the test passes.

```sh
# Example: creating a regression test
cargo test --lib crates/pb-core -- crash_regression_test
```

4. **Prevent recurrence:** Add a Rust `#[test]` that exercises the crash
   path with the exact input that triggered it.

## Prevention

- All array accesses must use `.get()` or checked indexing.
- All `Option` values must be matched or use `unwrap_or`/`unwrap_or_else`.
- All `Result` values must be handled with `?`, `.unwrap_or()`, or a match.
- The project uses `#![forbid(unsafe_code)]` — no unsafe code is permitted.
- Run `cargo test` and `scripts/live-fire.sh` before every release.
- Monitor crash rate: `grep -c "panicked" $PB_HOME/logs/powderburn.log`
