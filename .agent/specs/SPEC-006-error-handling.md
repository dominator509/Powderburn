# SPEC-006 Error Handling

Layer L2. Every error in POWDERBURN has a stable code, a layer, a player-facing sentence, and a
defined recovery. Nothing fails silently and nothing panics on player input.

## 1. Taxonomy

| Prefix | Layer | Panics allowed |
| --- | --- | --- |
| `E-SIM-` | simulation kernel | Yes, on invariant violation only, because a violated invariant means the state is already wrong |
| `E-CONTENT-` | content load and validation | Never |
| `E-SAVE-` | save read and write | Never |
| `E-JOURNAL-` | journal parse and legality | Never |
| `E-MOD-` | mod loading | Never |
| `E-RENDER-` | renderer and capture | Never |
| `E-CLI-` | command line surface | Never |
| `E-HIST-` | historical and representation validation | Never |
| `E-ANACHRONISM-` | period consistency validation | Never |

## 2. The registry (complete for v1)

| Code | Meaning | Player sentence | Recovery |
| --- | --- | --- | --- |
| E-SIM-001 | AP went negative | none, internal | panic; this is LBI-10 and must never reach a player |
| E-SIM-002 | Illegal action for state | The order could not be carried out. | reject the command, no state change |
| E-SIM-003 | Actor id not present | none, internal | panic |
| E-CONTENT-001 | RON parse failure | Content file could not be read. | name the file and line, refuse to start |
| E-CONTENT-002 | Unknown reference id | none | name the referring record and the missing id |
| E-CONTENT-003 | Duplicate id | none | name both files |
| E-SAVE-VERSION | Unknown save format | This save was made by a different version. | offer to open the release notes |
| E-SAVE-OVERSIZE | Declared section exceeds limit | This save file is damaged. | refuse, no allocation |
| E-SAVE-INCOMPAT | Ruleset or content hash mismatch | This save needs the mods it was made with. | list the differing hashes and the enabled mods |
| E-SAVE-TAMPERED | Ledger chain does not verify | The Ledger in this save does not match itself. | offer Unverified mode, disable the Ledger ending |
| E-JOURNAL-ILLEGAL | Journal command illegal at its tick | none, tooling only | hard error naming tick, actor, and command |
| E-JOURNAL-PARSE | Journal line malformed | none, tooling only | hard error naming the line number |
| E-MOD-PATH | Mod referenced a path outside itself | A mod tried to read outside its own folder and was disabled. | disable that mod, continue |
| E-MOD-EXEC | Mod contained an executable | A mod contained a program file and was disabled. | disable that mod, continue |
| E-RENDER-001 | No suitable adapter | Graphics could not start. | name the adapters tried, exit non-zero |
| E-RENDER-002 | Asset decode failure | An image file could not be read. | name the file, refuse; never substitute a placeholder texture |
| E-CLI-001 | Missing required flag | none | usage line to stderr, exit 2 |
| E-HIST-001 | Content alters a HISTORICAL_FIXED outcome | none, authoring only | name the node, the edge, and the fixed outcome |
| E-HIST-002 | Character record missing nation, community, or sources | none, authoring only | name the record and the missing field |
| E-HIST-003 | Forbidden token in content | none, authoring only | name file, line, and token |
| E-ANACHRONISM-001 | Item predates its first_year_available in a dated scenario | none, authoring only | name scenario date, item, and its first year |

## 3. Rules

Errors are values, not exceptions. Every fallible function returns `Result` with a typed error that
carries its code. `unwrap` and `expect` are denied by clippy in every crate except tests, and every
allowed use carries a one line justification comment that names the invariant making it safe.

Panics are legal only for violated internal invariants in pb-sim and pb-core, never for anything a
player, a file, or a mod can cause. This distinction is what keeps LF-04 honest: a save that a player
corrupted is a refusal, and an AP counter going negative is a bug that must stop the world.

No error message ever contains a filesystem path outside the game's own directories, an environment
variable value, or a memory address in release builds.

## 4. Crash artifacts

On panic in a release build the game writes `$PB_CONFIG_DIR/crash/<utc>.txt` containing the panic
message, the backtrace, the build hash, the ruleset and content hashes, and the last two hundred
event log lines. It contains no player path outside the config directory and no environment values.
It is never uploaded; there is nowhere to upload it to. The screen tells the player exactly where the
file is and invites them to attach it to a report, which is the entire crash reporting strategy and
is sufficient.
