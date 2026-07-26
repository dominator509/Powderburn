# Runbook: Release Rollback

## Symptom

- The release fails smoke tests after deployment.
- A critical bug (crash, data loss, security issue) is discovered post-release.
- Performance regression exceeds the 10% threshold in live-fire benchmarks.
- Content validation failure in the released build.
- Ledger verification fails on save files created by the new release.

## Immediate Check

1. **Identify the failing release version:**
   ```sh
   cat "$PB_RELEASE_DIR/current" 2>/dev/null || echo "no symlink"
   ls -la "$PB_RELEASE_DIR/"
   ```

2. **Check the release index for the last known-good version:**
   ```sh
   cat "$PB_RELEASE_DIR/index.txt"
   ```

3. **Confirm the failure is real:**
   ```sh
   $PB_RELEASE_DIR/current/powderburn --smoke-test 2>&1
   ```
   If the smoke test passes, the issue may be environmental; check the
   runbooks for the specific symptom (crash, performance, content, etc.).

4. **Verify the rollback target exists:**
   ```sh
   test -d "$PB_RELEASE_DIR/v<i.e. 0.3.0>" && echo "exists" || echo "not found"
   ```

## Diagnosis Steps

### 1. Confirm the rollback target

Check the release index for the previous version:

```sh
grep "PUBLISH" "$PB_RELEASE_DIR/index.txt" | tail -5
```

The latest `PUBLISH` line before the failing release is the rollback
target.  Verify it:

```sh
$PB_RELEASE_DIR/v<target>/powderburn --smoke-test
```

If the target itself is broken, go back further.

### 2. Check for save compatibility

If the release changed the save format, the rollback may leave saves in
a state the old binary cannot load:

```sh
$PB_RELEASE_DIR/v<target>/powderburn --verify-save "$PB_HOME/saves/autosave.pb_save"
```

If this fails, saves must be migrated or considered lost.

### 3. Review the release diff

```sh
git diff v<target>..v<current> -- crates/ content/ Cargo.toml Cargo.lock
```

This shows every change between the two releases.  Identify the breaking
change.

### 4. Check for data migration

If the release included a database or save migration, the rollback must
also revert the migration.  Check:

```sh
grep -i "migrate\|upgrade" "$PB_RELEASE_DIR/v<current>/CHANGELOG.md" 2>/dev/null || true
```

## Resolution

### Automated Rollback

Use the rollback script:

```sh
scripts/rollback.sh --to v0.3.0
```

This:
1. Repoints the `$PB_RELEASE_DIR/current` symlink to the target version.
2. Appends a `ROLLBACK` line to `$PB_RELEASE_DIR/index.txt` with the
   timestamp and version rolled back from and to.
3. Re-runs the smoke test on the rollback target.

### Manual Rollback

If the script is unavailable:

```sh
# 1. Switch the symlink
ln -sfn "$PB_RELEASE_DIR/v<target>" "$PB_RELEASE_DIR/current"

# 2. Record the rollback
echo "ROLLBACK $(date -u +%Y-%m-%dT%H:%M:%SZ) from=v<current> to=v<target>" \
  >> "$PB_RELEASE_DIR/index.txt"

# 3. Verify
$PB_RELEASE_DIR/current/powderburn --smoke-test
```

### Post-Rollback

1. **Prevent the broken release from being re-published:**
   The `scripts/release.sh` script refuses to overwrite an existing
   version directory.  The broken release must be deleted:

   ```sh
   rm -rf "$PB_RELEASE_DIR/v<broken>"
   ```

2. **Append a ledger entry:**
   ```
   ROLLBACK release v<current> → v<target> — reason: <brief summary>
   ```

3. **Fix the root cause:**
   Create a new issue (NODE) to fix the bug and ship a new release with a
   bumped patch version.

## Prevention

- Every release must pass `scripts/live-fire.sh` and `scripts/reality-gate.sh`
  before publication.
- The `current` symlink is only updated after a successful smoke test.
- Releases are immutable — never modify a published release directory.
- The release index provides an auditable rollback history.
- Save format version is checked at load; rollbacks that would break saves
  are detected ahead of time and blocked.
