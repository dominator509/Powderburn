# SPEC-002 Data Model and Vocabulary Lock

Layer L2. Every name in this file is canonical. No agent may introduce a name outside these tables
without a Decision Log entry and a spec update.

## 1. Crates

| Crate | Purpose | May import |
| --- | --- | --- |
| pb-core | Fix32, ids, geometry, event types, hashing | nothing in workspace |
| pb-rng | PbRng counter-based generator and stream tags | pb-core |
| pb-rules | Loaded rule tables, weapon and mark records, lookup | pb-core, pb-rng |
| pb-sim | Sequence clock, actors, actions, shot pipeline, environment | pb-core, pb-rng, pb-rules |
| pb-ai | Utility scoring and squad behaviors | pb-core, pb-rng, pb-rules, pb-sim |
| pb-content | RON schema, loader, validator, campaign graph | pb-core, pb-rules |
| pb-save | Save format, Ledger chain, migration refusal | pb-core, pb-sim, pb-content |
| pb-render | wgpu isometric renderer, sprites, camera, capture | pb-core, pb-sim, pb-content |
| pb-audio | Mixer, cue system | pb-core |
| pb-app | winit shell, screens, input map, the `powderburn` binary | all of the above |
| pb-cli | `pbcli` binary: sim, replay, capture, campaign, bench, selftest, a11y-report | all except pb-app |
| pb-tools | `pbtool` binary: validate, golden, image, atlas | pb-core, pb-content, pb-rules |

## 2. Identifier conventions

- Actor instance ids: `e_<archetype>_<nn>`, for example `e_bandit_02`.
- Companion ids: `c_<name>`, from the SPEC-000 roster only.
- Mission ids: `m<nn>_<slug>`, for example `m04_medicine_lodge`.
- Scenario files: `content/scenarios/<id>.ron`. Proving scenarios are prefixed `prov_`.
- Weapon ids: snake_case model names, for example `colt_army_1860`, `winchester_1873`.
- Item ids: `it_<slug>`. Mark ids: `mk_<slug>`. Way ids: `wy_<slug>`. Faction ids: `f_<slug>`.
- Journals: `tests/journals/<id>.jrnl`. Scripts: `tests/journals/<id>.script`.
- Golden hashes: `$PB_GOLDEN_DIR/<id>.hash` and `<id>.frame.sha256`.

## 3. Core entities

### Actor
`id`, `archetype_id`, `faction_id`, `attributes` (seven values), `level`, `hp`, `hp_max`, `sand`,
`sand_max`, `ap`, `ap_max`, `sequence`, `next_act_at`, `pos` (TileXY), `facing` (0 to 7), `stance`,
`wounds` (BTreeSet), `inventory` (Vec of ItemStack), `equipped_primary`, `equipped_sidearm`,
`marks` (Vec), `ways` (Vec), `morale_state`, `fatigue`, `is_companion`, `is_dead`.

### Weapon
`id`, `display_name`, `damage_dice`, `accuracy`, `range_bands` (four thresholds in tiles),
`ap_override` (map of Action to cost), `capacity`, `reload_class` (Cartridge, CapAndBall, Cylinder,
Breech, Tube), `fouling_rate`, `base_misfire`, `smoke_output`, `two_handed`, `first_year_available`,
`historical_note`, `sources`.

### Tile
`terrain`, `elevation`, `cover_edges` (eight values of None, Soft, Hard, Full), `smoke_density`,
`fire`, `blood`, `occupant`, `items`.

### Scenario
`id`, `display_name`, `date` (ISO year-month-day), `map`, `light`, `weather`, `wind_dir`,
`deployment_zones`, `actors`, `objectives`, `victory_conditions`, `defeat_conditions`,
`historical_tag` (None or `HISTORICAL_FIXED`), `citations`.

### CampaignNode
`id`, `kind` (Mission, Camp, HistoricalBeat), `date`, `requires` (predicate over campaign flags),
`grants` (flags set on completion), `unlocks` (node ids), `historical_tag`, `citations`,
`companion_gates` (companion ids whose death or absence changes this node).

### LedgerEntry
`index`, `prev_hash`, `name`, `role`, `place`, `date`, `chosen_line`, `written_by`, `hash`.
`hash = blake3(index || prev_hash || name || role || place || date || chosen_line)`. Entry 0 has
`prev_hash` of thirty-two zero bytes.

### SaveFile
`format_version`, `ruleset_hash`, `content_hash`, `campaign_seed`, `ledger_head_hash`,
`campaign_flags`, `company`, `sim_snapshot` (present only for a mid-combat save), `written_at_tick`.

## 4. Event vocabulary (the event log; LF proofs grep these)

`TurnBegin`, `TurnEnd`, `Moved`, `StanceChanged`, `Fired`, `Misfire`, `Jammed`, `Missed`,
`HitLocation`, `DamageApplied`, `WoundApplied`, `Critical`, `WeaponDropped`, `SmokeDeposited`,
`SmokeDecayed`, `SandLost`, `MoraleStateChanged`, `Routed`, `ActorKilled`, `CompanionKilled`,
`ObjectiveComplete`, `LedgerEntryWritten`, `ScenarioEnded`.

Event lines in `--emit-events` output are exactly `event: <Name> <k>=<v> <k>=<v>` with keys in
declaration order. This format is a contract; LF-02 depends on it byte for byte.

## 5. Campaign flags

Snake case, prefixed by act: `a1_treaty_witnessed`, `a1_whitehorse_recruited`,
`a2_promontory_present`, `a2_patron_ruined`, `a3_adobe_walls_survived`, `a3_hides_burned`,
`a4_teague_spared`, `a4_teague_killed`, `a4_salt_war_sided_with_ruelas`. Flags are set only by
`grants` on a CampaignNode. Nothing else writes them.

## 6. Environment variables (the complete set; matches PREFLIGHT.md exactly)

`RUSTUP_TOOLCHAIN`, `PB_CARGO_OFFLINE`, `PB_HOME`, `PB_ASSET_ROOT`, `PB_GOLDEN_DIR`, `PB_CACHE_DIR`,
`PB_HEADLESS_ADAPTER`, `PB_RELEASE_SIGNING_KEY`, `PB_RELEASE_DIR`, `PB_ITCH_API_KEY`, and at runtime
only `PB_LOG` (log filter) and `PB_CONFIG_DIR` (player settings location). No other variable is read
by any binary. Introducing one requires updating PREFLIGHT.md, .env.example, ENVIRONMENT.md, and this
table in the same milestone.

## 7. On-disk layout

    content/
      rules/            weapons.ron items.ron marks.ron ways.ron factions.ron tables.ron
      scenarios/        one file per scenario, including prov_ proving scenarios
      campaign/         nodes.ron acts.ron flags.ron
      dialogue/         one file per companion plus one per act
      lint/             forbidden_tokens.txt
      PROVENANCE.toml   every asset, its origin, its license
      BIBLIOGRAPHY.md   every historical source cited by any content record
    assets/
      sprites/ tiles/ ui/ fonts/ audio/
    tests/
      journals/ fixtures/ golden/

## 8. Save and content hashing

`ruleset_hash = blake3 over the sorted contents of content/rules/`.
`content_hash = blake3 over the sorted contents of content/` excluding BIBLIOGRAPHY.md.
A save whose `ruleset_hash` or `content_hash` does not match the running build is refused with
`E-SAVE-INCOMPAT`. It is never silently migrated. This is LBI-08.
