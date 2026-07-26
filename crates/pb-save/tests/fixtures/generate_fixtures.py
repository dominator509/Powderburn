#!/usr/bin/env python3
"""Generate adversarial fixture files for security trust boundary tests."""

import os
import struct
import hashlib

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
# Find the repo root
REPO_ROOT = os.path.normpath(os.path.join(SCRIPT_DIR, "..", "..", "..", ".."))

PB_CONTENT_TESTS = os.path.join(REPO_ROOT, "crates", "pb-content", "tests")
PB_SAVE_TESTS = os.path.join(REPO_ROOT, "crates", "pb-save", "tests")

CONTENT_FIXTURES = os.path.join(PB_CONTENT_TESTS, "fixtures")
SAVE_FIXTURES = os.path.join(PB_SAVE_TESTS, "fixtures")

# ── 1. bomb/rules/weapons.ron — 8193 weapon records ──
def gen_bomb_weapons():
    path = os.path.join(CONTENT_FIXTURES, "bomb", "rules", "weapons.ron")
    os.makedirs(os.path.dirname(path), exist_ok=True)

    records = []
    for i in range(8193):
        records.append(f"""    WeaponData(
        id: "w{i}",
        display_name: "Weapon {i}",
        damage_dice: DiceRoll(count: 1, sides: 6, bonus: 0),
        accuracy: 0,
        range_bands: [5, 15, 30, 50],
        ap_override: {{}},
        capacity: 6,
        reload_class: "tube_magazine",
        fouling_rate: 3,
        base_misfire: 2,
        smoke_output: 1,
        two_handed: false,
        first_year_available: 1860,
        historical_note: "",
        sources: ["core"],
    ),""")

    content = "[\n" + "\n".join(records) + "\n]\n"
    with open(path, "w") as f:
        f.write(content)
    print(f"Generated {path} ({len(records)} records, {os.path.getsize(path)} bytes)")


# ── 2. deep_nest/scenarios/deep_nest.ron — deeply nested ──
def gen_deep_nest():
    path = os.path.join(CONTENT_FIXTURES, "deep_nest", "scenarios", "deep_nest.ron")
    os.makedirs(os.path.dirname(path), exist_ok=True)

    inner = "42"
    for _ in range(256):
        inner = f"Some({inner})"

    content = f"""ScenarioData(
    id: "deep_nest",
    display_name: "Deep Nest",
    date: "1867-10-21",
    map: MapData(
        width: 10,
        height: 10,
        tiles: {{}},
    ),
    light: "daylight",
    weather: "clear",
    wind_dir: "north",
    deployment_zones: {{}},
    actors: [
        ActorData(
            id: "deep_actor",
            archetype_id: "scout",
            faction_id: "player",
            attributes: Attributes(grit: 5, nerve: 5, wind: 5, hands: 5, eyes: 5, savvy: 5, luck: 5),
            level: 1,
            hp: 20,
            hp_max: 20,
            sand: 10,
            sand_max: 10,
            ap: 6,
            ap_max: 6,
            sequence: 0,
            pos: TileXYData(x: 0, y: 0),
            facing: 0,
            stance: "standing",
            wounds: [],
            inventory: [],
            equipped_primary: None,
            equipped_sidearm: None,
            marks: [],
            ways: [],
            is_companion: false,
            is_dead: false,
        ),
    ],
    objectives: [],
    victory_conditions: [],
    defeat_conditions: [],
    historical_tag: None,
    citations: [],
)
"""
    with open(path, "w") as f:
        f.write(content)
    print(f"Generated {path} ({os.path.getsize(path)} bytes)")


# ── 3. huge_declared_len.pbsave — valid PBSV with 5000 ledger entries ──
def gen_huge_declared_len():
    path = os.path.join(SAVE_FIXTURES, "huge_declared_len.pbsave")

    entries = []
    prev_hash = "0000000000000000000000000000000000000000000000000000000000000000"
    for i in range(5000):
        raw = f"{i}{prev_hash}Person{i}RolePlace1870-01-01Line{i}System"
        h = hashlib.sha256(raw.encode()).hexdigest()
        entry = f"""    LedgerEntryData(
        index: {i},
        prev_hash: "{prev_hash}",
        name: "Person{i}",
        role: "Role",
        place: "Place",
        date: "1870-01-01",
        chosen_line: "Line {i}",
        written_by: "System",
        hash: "{h}",
    ),"""
        entries.append(entry)
        prev_hash = h

    head_hash = prev_hash

    RULESET_HASH = hashlib.sha256(b"ruleset_hash_seed").hexdigest()
    CONTENT_HASH = hashlib.sha256(b"content_hash_seed").hexdigest()

    payload = f"""SaveFileData(
    format_version: 1,
    ruleset_hash: "{RULESET_HASH}",
    content_hash: "{CONTENT_HASH}",
    campaign_seed: 42,
    ledger_head_hash: "{head_hash}",
    ledger_entries: [
{chr(10).join(entries)}
    ],
    campaign_flags: ["test_flag"],
    company: [],
    sim_snapshot: None,
    written_at_tick: 100,
)
"""
    magic = b"PBSV"
    version = struct.pack("<I", 1)
    ron_bytes = payload.encode("utf-8")
    data = magic + version + ron_bytes

    with open(path, "wb") as f:
        f.write(data)
    print(f"Generated {path} ({len(data)} bytes, {len(entries)} entries)")


if __name__ == "__main__":
    gen_bomb_weapons()
    gen_deep_nest()
    gen_huge_declared_len()
