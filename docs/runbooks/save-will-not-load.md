# Runbook: Save File Will Not Load

## Symptom

- The game fails to load a `.pb_save` file, printing an error such as
  "corrupt save", "checksum mismatch", or "unexpected format".
- The Ledger hash-chain verification fails on load.
- The application exits with an error referencing the save path.

## Immediate Check

1. **Verify the save file exists and has content:**
   ```sh
   ls -la "$PB_HOME/saves/"*.pb_save
   file "$PB_HOME/saves/"*.pb_save
   ```
   A zero-byte save file indicates a failed write.

2. **Check the log for load errors:**
   ```sh
   grep -i "save\|load\|checksum\|ledger" "$PB_HOME/logs/powderburn.log" | tail -20
   ```

3. **Check available disk space:**
   ```sh
   df -h "$PB_HOME"
   ```
   A full disk during write can truncate the save file.

4. **Attempt a manual Ledger re-verify:**
   ```sh
   powderburn --verify-save "$PB_HOME/saves/autosave.pb_save"
   ```

## Diagnosis Steps

### 1. Inspect the save file header

Save files have a magic header.  Use `xxd` or `od` to check:

```sh
xxd -l 32 "$PB_HOME/saves/autosave.pb_save"
```

Expected magic: `PB1\n` (4 bytes).  If the magic is missing or garbled the
file is not a valid save.

### 2. Check version compatibility

Save files embed the POWDERBURN version that created them:

```sh
od -A x -t x1z -j 4 -l 4 "$PB_HOME/saves/autosave.pb_save"
```

Compare against the running version:

```sh
powderburn --version
```

If the save version is newer than the running binary (e.g. rollback), the
binary must be upgraded or the save migrated.  If the save version is older,
a forward migration path must exist.

### 3. Verify Ledger hash-chain integrity

The Ledger is an append-only hash chain embedded in the save.  Each entry
commits to the previous hash.  A broken chain means the save was tampered
with or corrupted after writing.

```sh
powderburn --verify-ledger "$PB_HOME/saves/autosave.pb_save"
```

If this fails, the save cannot be loaded — it is considered compromised.

### 4. Check for partial or concurrent writes

If the game crashed during a save, the file may be truncated:

```sh
# Compare actual size to expected size from the save header
stat --format=%s "$PB_HOME/saves/autosave.pb_save"
```

A truncated save cannot be recovered.  The only recovery is from the
previous save (autosave rotation or manual save).

### 5. Attempt load with `--repair` (if available)

POWDERBURN may offer a repair mode:

```sh
powderburn --repair-save "$PB_HOME/saves/autosave.pb_save" -o "$PB_HOME/saves/repaired.pb_save"
```

Repair attempts to re-derive the valid Ledger hash-chain from the existing
entries, discarding any trailing garbage bytes.

## Resolution

1. **Corrupt save:** Load the previous autosave rotation or a manual save.
   Autosaves rotate 3 deep (`autosave.pb_save.0`, `.1`, `.2`).  Copy the
   newest non-corrupt candidate:

   ```sh
   cp "$PB_HOME/saves/autosave.pb_save.1" "$PB_HOME/saves/autosave.pb_save"
   ```

2. **Version mismatch:** If the save is from a newer version, rebuild or
   redeploy the matching binary.  If the save is from an older version and
   migration is not yet implemented, the save is stuck until migration code
   is written.

3. **Ledger chain break:** The save is unrecoverable by design.  The Ledger
   is intentionally append-only and hash-chained — a broken chain means the
   data cannot be trusted.  Revert to the last valid save.

## Prevention

- Always keep the 3 most recent autosaves.
- Never kill the process during a save operation (wait for "save_complete"
  in the log).
- Run `powderburn --verify-save` periodically in CI to detect regressions.
- Run `cargo test --test save_roundtrip` before every release.
- The save format version is checked at load — bump it when the format
  changes and implement forward migration.
