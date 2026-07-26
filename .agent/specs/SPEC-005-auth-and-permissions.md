# SPEC-005 Trust Boundaries and Permissions

Layer L2. POWDERBURN has no accounts, no login, no server, and no multi-user model, so there is no
authentication and no authorization in the usual sense. This spec defines what replaces them: the
trust boundaries that do exist, and the permission model for data mods.

## 1. The trust boundaries

| Boundary | Untrusted side | Rule |
| --- | --- | --- |
| Save file | Any `.pbsave` on disk, including one the player was sent | Parsed with explicit size limits before allocation, integrity checked against the Ledger chain, refused on hash mismatch |
| Journal file | Any `.jrnl` | Every line validated for legality at its tick; illegal lines are hard errors |
| Content tree | `content/` shipped, plus any enabled mod | Validated by the same validator as shipped content, with the same error codes |
| Data mod | Anything under `$PB_CONFIG_DIR/mods/` | Data only. See section 3 |
| Asset file | Anything under `assets/` or a mod's assets | Decoded with dimension and size caps; a decode failure is a refusal, not a fallback texture |
| Config file | `$PB_CONFIG_DIR/*.ron` | Every field range-checked; an out-of-range value is replaced by the default and logged |

## 2. Save integrity

A save carries `ruleset_hash`, `content_hash`, and `ledger_head_hash`. On load:
1. Format version must be known, else `E-SAVE-VERSION`.
2. Declared byte length of every variable-length section must be within the limit constant in
   `crates/pb-save/src/load.rs`, else `E-SAVE-OVERSIZE`. No allocation happens before this check.
3. `ruleset_hash` and `content_hash` must match the running build, else `E-SAVE-INCOMPAT`. There is
   no automatic migration; the player is told which mod or version mismatch caused it.
4. The Ledger chain is recomputed from entry zero and must reach `ledger_head_hash`, else
   `E-SAVE-TAMPERED`. The player may still load in a clearly labeled Unverified mode, which disables
   the Ledger-based ending. Refusing outright would punish a player whose disk went bad; silently
   accepting would make the Ledger meaningless.

## 3. The data mod permission model

Mods are data. There is no scripting engine, no dynamic library loading, no shell out, and no eval.
A mod may:
- add or override records in `content/rules/`, `content/scenarios/`, `content/dialogue/`;
- add assets under its own directory;
- declare a load order and dependencies.

A mod may not:
- reference a path outside its own directory; path traversal is rejected at load with `E-MOD-PATH`;
- override the campaign graph nodes tagged `HISTORICAL_FIXED`, rejected with `E-HIST-001`;
- disable the content validator; a mod that fails validation is disabled and reported, never
  partially applied;
- ship an executable; any file with an execute bit or a known executable magic number is rejected
  with `E-MOD-EXEC`.

Enabling a mod changes `content_hash`, which by section 2 makes existing saves incompatible. The UI
says so before enabling, in a sentence, with the number of affected saves.

## 4. Secret handling

The only secrets in this project belong to the build, not the player: `PB_RELEASE_SIGNING_KEY` and
the optional `PB_ITCH_API_KEY`. They live in `.env`, are never committed, never logged, never
embedded in an artifact, and never read by any shipped binary. `scripts/security-check.sh` fails if
`.env` is tracked, if key material appears in a tracked file, or if a credential-shaped literal
appears in source.

## 5. Filesystem permissions

The shipped game writes only inside `$PB_CONFIG_DIR` and its own save directory. It never writes to
the installation directory, never writes outside the user profile, and creates files with mode 0600
for saves and 0644 for settings. A write outside those roots is a bug of the same severity as a
memory safety fault.

## 6. What is deliberately absent

No anti-cheat. No integrity check that phones home. No hardware fingerprint. No launcher. Single
player games do not need these and their presence would violate LBI-09.
