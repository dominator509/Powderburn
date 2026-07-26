# Runbook: Content Validation Failure

## Symptom

- `scripts/reality-gate.sh` exits non-zero.
- `scripts/live-fire.sh` reports a content integrity violation.
- The game fails to start with a "content validation" error.
- A content file (JSON, TOML, image, audio) fails its checksum or schema
  check.
- `content/PROVENANCE.toml` lists a hash that does not match the on-disk
  file.

## Immediate Check

1. **Run the content validator:**
   ```sh
   scripts/reality-gate.sh 2>&1 | tee /tmp/reality-gate.log
   ```
   The first failing check tells you which file is invalid.

2. **Check file modification times:**
   ```sh
   ls -la content/
   stat content/<failing-file>
   ```
   A very recent modification date (or one in the future) suggests
   accidental or unauthorized modification.

3. **Check the provenance manifest:**
   ```sh
   cat content/PROVENANCE.toml
   ```
   Look for the hash entry of the failing file.  Compare with actual:

   ```sh
   sha256sum content/<failing-file>
   ```

4. **Check for missing files:**
   ```sh
   for f in $(cat content/MANIFEST); do
     test -f "content/$f" || echo "MISSING: $f"
   done
   ```

## Diagnosis Steps

### 1. Classify the failure type

| Error Message | Likely Cause | Action |
|---------------|-------------|--------|
| `checksum mismatch` | File content changed | Restore from git or rebuild asset |
| `schema violation` | JSON/TOML syntax error | Fix the file format |
| `missing file` | File deleted or not added to MANIFEST | Restore or add to manifest |
| `unlicensed asset` | File not listed in PROVENANCE.toml | Add provenance entry |
| `size mismatch` | File truncated or corrupted | Re-download from source |

### 2. Check git status

```sh
git status content/
git diff content/
```

If the file is tracked by git and shows unstaged changes, the content was
modified after the last commit.  This is the most common cause.

### 3. Rebuild content from source

Some content files are generated from source assets:

```sh
# Check for a rebuild script
ls scripts/rebuild-*
# If one exists, run it:
scripts/rebuild-content.sh 2>&1
```

Then re-run validation:

```sh
scripts/reality-gate.sh
```

### 4. Check the vendor or upstream source

If content was fetched from an external source:

```sh
# Check if a download script exists
ls scripts/fetch-*
# For external assets, verify the upstream hash:
cat content/PROVENANCE.toml | grep -A3 "<failing-file>"
```

### 5. Validate the schema

For structured data files:

```sh
# JSON validation
python3 -m json.tool content/<file>.json > /dev/null

# TOML validation
python3 -c "import toml; toml.load('content/<file>.toml')"

# For custom binary formats, use the built-in validator:
powderburn --validate-content content/<file>
```

## Resolution

1. **Unintended modification:** Restore from git and re-run validation:
   ```sh
   git checkout -- content/<failing-file>
   scripts/reality-gate.sh
   ```

2. **Intentional content change:** Update `content/PROVENANCE.toml` with the
   new hash and add a ledger entry explaining the change.  Then re-validate.

3. **Schema error:** Fix the file and re-run:
   ```sh
   # Fix the file manually or with a script
   scripts/reality-gate.sh
   ```

4. **Missing provenance:** Add an entry to `content/PROVENANCE.toml` with
   the source URL, license, and SHA-256 hash.  The entry format is:
   ```toml
   [files.<relative-path>]
   source = "https://..."
   license = "CC0-1.0"
   sha256 = "abcdef..."
   ```

5. **Irrecoverably corrupted asset:** Re-download from the source listed in
   `PROVENANCE.toml` and re-verify.

## Prevention

- `scripts/reality-gate.sh` runs on every commit via pre-commit hook.
- `scripts/live-fire.sh` must pass before every release.
- Content files are read-only after the first release (`chmod -w` on
  `content/` tree).
- `content/PROVENANCE.toml` is append-only — entries are never removed,
  only added.
- The CI pipeline rejects any PR that changes content without updating
  `PROVENANCE.toml`.
