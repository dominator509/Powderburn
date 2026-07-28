from __future__ import annotations

from pathlib import Path


MISSIONS = [
    ("m02_promontory", "Promontory: The Last Spike", "1869-05-10", "Dusk", "Clear", "W", "RailBed", "f_elk_creek_ring", "winchester_1866", True, "Stop Teague's saboteurs after the ceremony without changing the historical event."),
    ("m04_medicine_lodge", "Medicine Lodge: Broken Ink", "1867-10-21", "Dusk", "Wind", "NE", "Prairie", "f_elk_creek_ring", "henry_1860", False, "Drive armed provocateurs away from the treaty road."),
    ("m05_hide_yard", "Adobe Walls: The Hide Yard", "1874-06-27", "Day", "Wind", "S", "HideYard", "f_bandits", "sharps_1874", True, "Destroy the Ring's private killing ledger after the fixed battle has ended."),
    ("m002_pawnee_fork", "Pawnee Fork: Hancock's Shadow", "1867-04-19", "Day", "Wind", "E", "Prairie", "f_elk_creek_ring", "spencer_1860", True, "Recover proof from Ring riders operating beyond the historical column."),
    ("m003_adobe_walls", "Adobe Walls: Second Morning", "1874-06-27", "Day", "Clear", "SW", "Adobe", "f_elk_creek_ring", "sharps_1874", True, "Survive Ring gunmen exploiting the confusion without rewriting the fixed battle."),
    ("m12_adobe_walls_relief", "Relief at Adobe Walls", "1878-11-12", "Dusk", "Dust", "N", "Adobe", "f_elk_creek_ring", "winchester_1873", False, "Break Teague's siege and bring the surviving witnesses out."),
    ("m12_hide_yard_reckoning", "Hide Yard Reckoning", "1878-11-12", "Night", "Wind", "NW", "HideYard", "f_elk_creek_ring", "winchester_1873", False, "End the Ring's last armed account at the abandoned hide yard."),
    ("m06_smoky_hill_station", "Smoky Hill Station", "1867-09-03", "Dusk", "Wind", "E", "Station", "f_bandits", "henry_1860", False, "Hold the station yard and recover the stolen mail strongbox."),
    ("m07_washita_winter", "Washita Winter", "1868-11-26", "Moonlit", "Snow", "N", "Snow", "f_elk_creek_ring", "spencer_1860", False, "Extract civilians from Ring raiders at the edge of the historical disaster."),
    ("m08_washita_aftermath", "Washita Aftermath", "1868-12-01", "Dusk", "Snow", "NE", "Snow", "f_bandits", "remington_1858", False, "Escort the winter-count witnesses through a closing ambush."),
    ("m09_rail_grade", "The Rail Grade", "1869-06-02", "Day", "Dust", "W", "RailBed", "f_pinkerton", "winchester_1866", False, "Protect the unpaid grade crew and seize the agency payroll book."),
    ("m10_denver_extension", "Denver Extension", "1870-06-24", "Dusk", "Rain", "S", "RailBed", "f_pinkerton", "winchester_1866", False, "Clear hired guns from the switchyard before the night train arrives."),
    ("m11_los_angeles_telegram", "Los Angeles Telegram", "1871-10-27", "Night", "Clear", "W", "Town", "f_elk_creek_ring", "colt_army_1860", False, "Carry the decoded telegram across the depot under fire."),
    ("m13_divide_crossing", "Divide Crossing", "1872-08-14", "Dusk", "Snow", "E", "Mountain", "f_bandits", "spencer_1860", False, "Open the pass and recover the survey party."),
    ("m14_panic_of_1873", "Panic of 1873", "1873-09-18", "Dusk", "Rain", "S", "Town", "f_pinkerton", "colt_saa_1873", True, "Secure the patron's books after the fixed market collapse, not before it."),
    ("m15_red_river_supply", "Red River Supply", "1874-09-12", "Day", "Dust", "NE", "Canyon", "f_elk_creek_ring", "springfield_1873", False, "Cut the Ring supply column without attacking the historical campaign column."),
    ("m16_black_hills_dispatch", "Black Hills Dispatch", "1876-06-29", "Moonlit", "Rain", "W", "Forest", "f_elk_creek_ring", "winchester_1873", False, "Carry the delayed dispatch through Ring scouts and flooded ground."),
    ("m17_nicodemus", "Nicodemus", "1877-09-17", "Day", "Wind", "S", "Prairie", "f_bandits", "winchester_1873", True, "Defend the settlement's chosen work after its fixed founding."),
    ("m18_great_strike", "Great Railroad Strike", "1877-07-21", "Night", "Rain", "E", "RailYard", "f_pinkerton", "springfield_1873", True, "Expose hired provocateurs without changing the strike's fixed course."),
    ("m19_fort_marion", "Fort Marion: Names Carried", "1878-04-13", "Dusk", "Rain", "N", "Fort", "f_elk_creek_ring", "springfield_1873", True, "Recover the prisoners' names from Ring thieves outside the fixed incarceration."),
    ("m20_salt_war", "San Elizario Salt War", "1877-12-17", "Day", "Dust", "SW", "Adobe", "f_lawmen", "winchester_1873", True, "Protect community records after the fixed Salt War violence."),
    ("m21_yellow_fever", "Memphis Yellow Fever", "1878-09-01", "Night", "Rain", "S", "Town", "f_elk_creek_ring", "coach_gun", True, "Recover stolen medicine while the fixed epidemic remains immutable."),
    ("m24_ledger_reckoning", "The Ledger Reckoning", "1878-11-11", "Lanternlit", "Wind", "W", "LedgerHouse", "f_elk_creek_ring", "winchester_1873", False, "Reach Teague's counting room and end the Ring's armed resistance."),
]


PLAYER_ATTRIBUTES = [
    (6, 6, 6, 6, 6, 5, 5),
    (5, 7, 7, 6, 6, 5, 4),
]
ENEMY_ATTRIBUTES = [
    (5, 4, 6, 6, 5, 3, 3),
    (6, 5, 5, 7, 5, 3, 3),
    (4, 4, 7, 5, 7, 3, 3),
    (7, 6, 5, 6, 6, 2, 3),
]


def actor(
    actor_id: str,
    archetype: str,
    faction: str,
    attrs: tuple[int, int, int, int, int, int, int],
    level: int,
    x: int,
    y: int,
    facing: int,
    weapon: str,
    companion: bool = False,
) -> str:
    grit, nerve, wind, hands, eyes, savvy, luck = attrs
    hp_max = 20 + grit * 3 + level * 2
    sand_max = 10 + nerve * 2
    ap_max = 5 + wind // 2
    sequence = 2 + (eyes + hands) // 4
    return f'''      (
        id: "{actor_id}",
        archetype_id: "{archetype}",
        faction_id: "{faction}",
        attributes: (grit:{grit}, nerve:{nerve}, wind:{wind}, hands:{hands}, eyes:{eyes}, savvy:{savvy}, luck:{luck}),
        level: {level}, hp: {hp_max}, hp_max: {hp_max}, sand: {sand_max}, sand_max: {sand_max},
        ap: {ap_max}, ap_max: {ap_max}, sequence: {sequence},
        pos: (x:{x}, y:{y}), facing: {facing}, stance: "Standing", wounds: [], inventory: [],
        equipped_primary: Some("{weapon}"), equipped_sidearm: None, marks: [], ways: [],
        is_companion: {str(companion).lower()}, is_dead: false,
      )'''


def cover(edges: dict[int, str]) -> str:
    values = ["None"] * 8
    for index, value in edges.items():
        values[index] = value
    # RON serializes Rust fixed-size arrays as tuple-like values.
    return "(" + ", ".join(f'"{value}"' for value in values) + ")"


def scenario_text(mission: tuple[str, ...], ordinal: int) -> str:
    (
        node_id,
        display,
        date,
        light,
        weather,
        wind,
        terrain,
        enemy_faction,
        enemy_weapon,
        fixed,
        objective,
    ) = mission
    scenario_id = f"scn_{node_id}"
    difficult = {
        "Snow": "DeepSnow",
        "RailBed": "Ballast",
        "Adobe": "Rubble",
        "HideYard": "Offal",
        "Station": "Mud",
        "Town": "Mud",
        "Mountain": "Scree",
        "Canyon": "Scree",
        "Forest": "Brush",
        "RailYard": "Ballast",
        "Fort": "Mud",
        "LedgerHouse": "Debris",
        "Prairie": "Brush",
    }.get(terrain, "Brush")
    shift = ordinal % 3
    tiles = {
        f"4,{4 + shift}": ("Clear", {2: "HalfHeight"}),
        f"4,{8 - shift}": ("Clear", {2: "Hard"}),
        f"19,{4 + shift}": ("Clear", {6: "Hard"}),
        f"19,{8 - shift}": ("Clear", {6: "HalfHeight"}),
        "10,4": (difficult, {2: "Soft", 6: "Soft"}),
        "11,5": (difficult, {0: "Hard", 4: "Hard"}),
        "12,6": (difficult, {2: "Full", 6: "Full"}),
        "11,7": (difficult, {0: "Soft", 4: "Soft"}),
        "10,8": (difficult, {2: "Hard", 6: "Hard"}),
        "7,6": (difficult, {}),
        "8,6": (difficult, {}),
        "15,6": (difficult, {}),
        "16,6": (difficult, {}),
    }
    tile_lines = []
    for key, (kind, edges) in tiles.items():
        tile_lines.append(
            f'        "{key}": (terrain:"{kind}", elevation:0, cover_edges:{cover(edges)}, '
            f"smoke_density:0, fire:false, blood:false, occupant:None, items:[]),"
        )
    player_weapon = "henry_1860" if int(date[:4]) < 1873 else "winchester_1873"
    actors = [
        actor("p_scout", "scout", "player", PLAYER_ATTRIBUTES[0], 1, 3, 5, 2, "colt_army_1860"),
        actor("p_rifle", "rifleman", "player", PLAYER_ATTRIBUTES[1], 1, 3, 7, 2, player_weapon),
    ]
    enemy_ids = []
    enemy_positions = [(20, 3), (20, 5), (20, 7), (18, 9)]
    for index, (attrs, position) in enumerate(zip(ENEMY_ATTRIBUTES, enemy_positions), start=1):
        enemy_id = f"{node_id}_enemy_{index:02d}"
        enemy_ids.append(enemy_id)
        actors.append(
            actor(
                enemy_id,
                "ring_gunman" if enemy_faction == "f_elk_creek_ring" else "hired_gun",
                enemy_faction,
                attrs,
                1 + (index == 4),
                position[0],
                position[1],
                6,
                enemy_weapon if index != 3 else "colt_army_1860",
            )
        )
    actor_lines = ",\n".join(actors)
    enemy_list = ", ".join(f'"{enemy_id}"' for enemy_id in enemy_ids)
    historical_tag = 'Some("HISTORICAL_FIXED")' if fixed else "None"
    citations = (
        '["Mission date and fixed-event boundary from the campaign node.", '
        '"See content/BIBLIOGRAPHY.md for the cited historical sources."]'
        if fixed
        else '["POWDERBURN Narrative Bible", "content/BIBLIOGRAPHY.md"]'
    )
    return f'''(
  id: "{scenario_id}",
  display_name: "{display}",
  date: "{date}",
  map: (
    width: 24,
    height: 14,
    tiles: {{
{chr(10).join(tile_lines)}
    }},
  ),
  light: "{light}",
  weather: "{weather}",
  wind_dir: "{wind}",
  deployment_zones: {{
    "ally": [(x:3,y:5), (x:3,y:7), (x:2,y:4), (x:2,y:6), (x:2,y:8), (x:4,y:3), (x:4,y:9), (x:5,y:2), (x:5,y:10)],
    "enemy": [(x:20,y:3), (x:20,y:5), (x:20,y:7), (x:18,y:9)],
  }},
  actors: [
{actor_lines}
  ],
  objectives: [
    (id:"obj_primary", description:"{objective}", kind:"Eliminate", actor_ids:[{enemy_list}]),
  ],
  victory_conditions: ["all_enemies_dead"],
  defeat_conditions: ["all_allies_dead"],
  historical_tag: {historical_tag},
  citations: {citations},
)
'''


def update_nodes(nodes: str) -> str:
    mapping = {node_id: f"scn_{node_id}" for node_id, *_ in MISSIONS}
    lines = nodes.splitlines()
    current_id: str | None = None
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('id: "'):
            current_id = stripped.split('"', 2)[1]
        if stripped.startswith("scenario_id:") and current_id in mapping:
            indent = line[: len(line) - len(line.lstrip())]
            lines[index] = f'{indent}scenario_id: Some("{mapping[current_id]}"),'
    return "\n".join(lines) + "\n"


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    scenario_root = root / "content" / "scenarios"
    for ordinal, mission in enumerate(MISSIONS):
        node_id = mission[0]
        (scenario_root / f"{node_id}.ron").write_text(
            scenario_text(mission, ordinal), encoding="utf-8", newline="\n"
        )
    nodes_path = root / "content" / "campaign" / "nodes.ron"
    nodes_path.write_text(update_nodes(nodes_path.read_text(encoding="utf-8")), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
