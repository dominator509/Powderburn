//! Content validator — runs semantic checks on a fully loaded [`Content`]
//! structure and returns a list of diagnostics.
//!
//! All checks are purely functional: [`validate`] takes a [`Content`] reference
//! and returns a `Vec<Diagnostic>`.  An empty vector means the content is valid.
//!
//! # Checks performed
//!
//! * **Duplicate IDs** — detects identical IDs used in different content
//!   collections (scenarios, weapons, companions, campaign\_nodes).
//! * **Unknown references** — weapon, companion, scenario, and actor
//!   references that do not resolve to a known definition.
//! * **Structural invariants** — non-empty identifiers, sorted range bands,
//!   valid dice rolls.

use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{Content, DiceRoll};

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single validation finding.
///
/// Code uses an `E-` prefix (e.g. `E-DUP-ID`, `E-REF-WEAPON`).
/// `file` and `line` are optional context hints; they are set when the
/// diagnostic can be attributed to a specific source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl Diagnostic {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            file: None,
            line: None,
        }
    }

    #[allow(dead_code)]
    fn with_file(self, file: &str) -> Self {
        Self {
            file: Some(file.to_string()),
            ..self
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate `content` and return all diagnostics found.
///
/// Returns an empty `Vec` if the content is valid.  The function never panics.
pub fn validate(content: &Content) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Duplicate IDs across all content collections.
    check_duplicate_ids(content, &mut diagnostics);

    // 2. Unknown weapon references in actor data.
    check_weapon_references(content, &mut diagnostics);

    // 3. Unknown companion references in campaign node gates.
    check_companion_references(content, &mut diagnostics);

    // 4. Unknown scenario references in campaign nodes.
    check_scenario_references(content, &mut diagnostics);
    check_campaign_scenario_alignment(content, &mut diagnostics);

    // 5. Dialogue structure, references, translation, and act coverage.
    check_dialogue(content, &mut diagnostics);

    // 6. ActorData structural invariants.
    check_actor_structure(content, &mut diagnostics);

    // 7. Weapon structural invariants (range_bands sorted ascending).
    check_weapon_structure(content, &mut diagnostics);

    // 8. DiceRoll structural invariants.
    check_dice_rolls(content, &mut diagnostics);

    // 9. Faction / archetype non-empty checks.
    check_faction_and_archetype(content, &mut diagnostics);

    // 10. Scenario objective actor references.
    check_objective_actor_references(content, &mut diagnostics);

    // 11. Anachronistic weapons — E-ANACHRONISM-001.
    check_anachronisms(content, &mut diagnostics);

    // 12. Every Ledger writing category offers exactly three usable lines.
    check_ledger_line_sets(content, &mut diagnostics);

    diagnostics
}

fn check_ledger_line_sets(content: &Content, out: &mut Vec<Diagnostic>) {
    let mut categories = BTreeSet::new();
    for line_set in content.ledger_line_sets.values() {
        if line_set.id.trim().is_empty() {
            out.push(Diagnostic::new(
                "E-CONTENT-002",
                "Ledger line-set id may not be empty",
            ));
        }
        if !matches!(line_set.applies_to.as_str(), "companion" | "combatant") {
            out.push(Diagnostic::new(
                "E-CONTENT-002",
                format!(
                    "Ledger line set `{}` has invalid applies_to `{}`",
                    line_set.id, line_set.applies_to
                ),
            ));
        } else {
            categories.insert(line_set.applies_to.as_str());
        }
        if line_set.lines.iter().any(|line| line.trim().is_empty()) {
            out.push(Diagnostic::new(
                "E-CONTENT-002",
                format!("Ledger line set `{}` contains an empty line", line_set.id),
            ));
        }
        if line_set.default_index >= 3 {
            out.push(Diagnostic::new(
                "E-CONTENT-002",
                format!(
                    "Ledger line set `{}` default index {} is outside 0..3",
                    line_set.id, line_set.default_index
                ),
            ));
        }
    }
    if !content.ledger_line_sets.is_empty() {
        for required in ["companion", "combatant"] {
            if !categories.contains(required) {
                out.push(Diagnostic::new(
                    "E-CONTENT-002",
                    format!("no Ledger line set applies to `{required}`"),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal check functions
// ---------------------------------------------------------------------------

/// E-DUP-ID — detect identical IDs used in different content collections.
fn check_duplicate_ids(content: &Content, out: &mut Vec<Diagnostic>) {
    // Collect all IDs with their source collection name.
    struct IdEntry<'a> {
        id: &'a str,
        collection: &'a str,
    }

    let mut entries: Vec<IdEntry<'_>> = Vec::new();

    for id in content.scenarios.keys() {
        entries.push(IdEntry {
            id,
            collection: "scenarios",
        });
    }
    for id in content.weapons.keys() {
        entries.push(IdEntry {
            id,
            collection: "weapons",
        });
    }
    for id in content.companions.keys() {
        entries.push(IdEntry {
            id,
            collection: "companions",
        });
    }
    for id in content.campaign_nodes.keys() {
        entries.push(IdEntry {
            id,
            collection: "campaign_nodes",
        });
    }

    // Use a set to track seen IDs; when we see an ID a second time, emit a
    // diagnostic for every entry that shares that ID.
    let mut seen: BTreeMap<&str, Vec<&IdEntry<'_>>> = BTreeMap::new();

    for entry in &entries {
        seen.entry(entry.id).or_default().push(entry);
    }

    for group in seen.values() {
        if group.len() > 1 {
            let ids: Vec<String> = group
                .iter()
                .map(|e| format!("{}::{}", e.collection, e.id))
                .collect();
            out.push(Diagnostic::new(
                "E-DUP-ID",
                format!("duplicate ID across collections: {}", ids.join(", ")),
            ));
        }
    }
}

/// E-REF-WEAPON — check that equipped_primary, equipped_sidearm, and
/// ItemStack.item_id values refer to a known weapon.
fn check_weapon_references(content: &Content, out: &mut Vec<Diagnostic>) {
    let weapon_ids: BTreeSet<&str> = content.weapons.keys().map(String::as_str).collect();

    for scenario in content.scenarios.values() {
        for actor in &scenario.actors {
            if let Some(ref wid) = actor.equipped_primary {
                if !weapon_ids.contains(wid.as_str()) {
                    out.push(Diagnostic::new(
                        "E-REF-WEAPON",
                        format!(
                            "actor `{}` references unknown weapon `{}` in equipped_primary",
                            actor.id, wid
                        ),
                    ));
                }
            }
            if let Some(ref wid) = actor.equipped_sidearm {
                if !weapon_ids.contains(wid.as_str()) {
                    out.push(Diagnostic::new(
                        "E-REF-WEAPON",
                        format!(
                            "actor `{}` references unknown weapon `{}` in equipped_sidearm",
                            actor.id, wid
                        ),
                    ));
                }
            }
            for item in &actor.inventory {
                if !weapon_ids.contains(item.item_id.as_str()) {
                    out.push(Diagnostic::new(
                        "E-REF-WEAPON",
                        format!(
                            "actor `{}` inventory references unknown weapon `{}`",
                            actor.id, item.item_id
                        ),
                    ));
                }
            }
        }
    }
}

/// E-REF-COMPANION — check that gates and authored recruits in campaign nodes
/// refer to known companions.
fn check_companion_references(content: &Content, out: &mut Vec<Diagnostic>) {
    let companion_ids: BTreeSet<&str> = content.companions.keys().map(String::as_str).collect();

    for node in content.campaign_nodes.values() {
        for gate_id in &node.companion_gates {
            if !companion_ids.contains(gate_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-COMPANION",
                    format!(
                        "campaign node `{}` references unknown companion `{}` in companion_gates",
                        node.id, gate_id
                    ),
                ));
            }
        }
        for recruit_id in &node.recruits {
            if !companion_ids.contains(recruit_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-COMPANION",
                    format!(
                        "campaign node `{}` references unknown companion `{}` in recruits",
                        node.id, recruit_id
                    ),
                ));
            }
        }
    }
}

/// E-REF-SCENARIO — check that scenario_id values in campaign nodes refer
/// to a known scenario.
fn check_scenario_references(content: &Content, out: &mut Vec<Diagnostic>) {
    let scenario_ids: BTreeSet<&str> = content.scenarios.keys().map(String::as_str).collect();

    for node in content.campaign_nodes.values() {
        if let Some(ref sc_id) = node.scenario_id {
            if !scenario_ids.contains(sc_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-SCENARIO",
                    format!(
                        "campaign node `{}` references unknown scenario `{}`",
                        node.id, sc_id
                    ),
                ));
            }
        }
    }
}

/// Validate dialogue references, accessibility fields, translation law, and act coverage.
/// Keep each campaign node and its playable scenario synchronized so the map,
/// briefing, Ledger, and battlefield cannot silently describe different
/// moments in the story.
fn check_campaign_scenario_alignment(content: &Content, out: &mut Vec<Diagnostic>) {
    for node in content.campaign_nodes.values() {
        let Some(scenario_id) = node.scenario_id.as_deref() else {
            continue;
        };
        let Some(scenario) = content.scenarios.get(scenario_id) else {
            continue;
        };
        if node.date != scenario.date {
            out.push(Diagnostic::new(
                "E-CAMPAIGN-DATE",
                format!(
                    "campaign node `{}` is dated {} but scenario `{}` is dated {}",
                    node.id, node.date, scenario.id, scenario.date
                ),
            ));
        }
        if node.kind == "Mission" {
            if scenario.briefing.len() < 2 {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!(
                        "mission `{}` needs at least two authored briefing beats",
                        node.id
                    ),
                ));
            }
            if scenario.prebattle_dialogue.len() < 4 {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!(
                        "mission `{}` needs at least four pre-battle dialogue lines",
                        node.id
                    ),
                ));
            }
            if scenario.battle_dialogue.len() < 4 {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!(
                        "mission `{}` needs at least four in-battle dialogue lines",
                        node.id
                    ),
                ));
            }
            if scenario.score.is_none() {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!("mission `{}` needs an authored mood score", node.id),
                ));
            }
        }
    }
}

fn check_dialogue(content: &Content, out: &mut Vec<Diagnostic>) {
    if content.dialogue.is_empty() {
        return;
    }

    let companion_ids: BTreeSet<&str> = content.companions.keys().map(String::as_str).collect();
    let node_ids: BTreeSet<&str> = content.campaign_nodes.keys().map(String::as_str).collect();

    for scene in content.dialogue.values() {
        if scene.id.is_empty() || !(1..=4).contains(&scene.act) || scene.lines.is_empty() {
            out.push(Diagnostic::new(
                "E-DIALOGUE-STRUCT",
                format!(
                    "dialogue scene `{}` has an invalid id, act, or empty line list",
                    scene.id
                ),
            ));
        }
        if matches!(
            (scene.min_ledger_weight, scene.max_ledger_weight),
            (Some(minimum), Some(maximum)) if minimum > maximum
        ) {
            out.push(Diagnostic::new(
                "E-DIALOGUE-STRUCT",
                format!(
                    "dialogue scene `{}` has minimum Ledger Weight above its maximum",
                    scene.id
                ),
            ));
        }
        if let Some(node_id) = &scene.node_id {
            if !node_ids.contains(node_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-DIALOGUE-NODE",
                    format!(
                        "dialogue scene `{}` references unknown node `{node_id}`",
                        scene.id
                    ),
                ));
            }
        }
        for companion_id in &scene.requires_alive {
            if !companion_ids.contains(companion_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-DIALOGUE-COMPANION",
                    format!(
                        "dialogue scene `{}` requires unknown companion `{companion_id}`",
                        scene.id
                    ),
                ));
            }
        }
        for line in &scene.lines {
            if !companion_ids.contains(line.speaker_id.as_str()) {
                out.push(Diagnostic::new(
                    "E-REF-DIALOGUE-COMPANION",
                    format!(
                        "dialogue scene `{}` has unknown speaker `{}`",
                        scene.id, line.speaker_id
                    ),
                ));
            }
            if !scene.requires_alive.contains(&line.speaker_id) {
                out.push(Diagnostic::new(
                    "E-PERMADEATH-DIALOGUE",
                    format!(
                        "dialogue scene `{}` does not gate living speaker `{}`",
                        scene.id, line.speaker_id
                    ),
                ));
            }
            if line.original.trim().is_empty()
                || line.subtitle.trim().is_empty()
                || line.language.trim().is_empty()
            {
                out.push(Diagnostic::new(
                    "E-DIALOGUE-STRUCT",
                    format!(
                        "dialogue scene `{}` has an empty authored line field",
                        scene.id
                    ),
                ));
            }
            if line.language != "en"
                && line
                    .translation
                    .as_deref()
                    .is_none_or(|translation| translation.trim().is_empty())
            {
                out.push(Diagnostic::new(
                    "E-REPRESENTATION-TRANSLATION",
                    format!(
                        "dialogue scene `{}` has untranslated `{}` dialogue",
                        scene.id, line.language
                    ),
                ));
            }
        }
    }

    for companion_id in content.companions.keys() {
        for act in 1..=4 {
            if !content
                .dialogue
                .values()
                .any(|scene| scene.act == act && scene.requires_alive.contains(companion_id))
            {
                out.push(Diagnostic::new(
                    "E-DIALOGUE-COVERAGE",
                    format!("companion `{companion_id}` has no gated dialogue in act {act}"),
                ));
            }
        }
    }
}

/// E-STRUCT — check that ActorData has non-empty id, and that it has
/// non-empty faction_id and archetype_id (also checked under
/// check_faction_and_archetype).
fn check_actor_structure(content: &Content, out: &mut Vec<Diagnostic>) {
    const SKILL_IDS: [&str; 8] = [
        "Pistols",
        "LongGuns",
        "Scatterguns",
        "Blades",
        "Explosives",
        "FieldMedicine",
        "Scouting",
        "Talk",
    ];
    for scenario in content.scenarios.values() {
        for actor in &scenario.actors {
            if actor.id.is_empty() {
                out.push(Diagnostic::new(
                    "E-STRUCT",
                    format!("actor in scenario `{}` has empty id", scenario.id),
                ));
            }
            for (skill, level) in &actor.skill_levels {
                if !SKILL_IDS.contains(&skill.as_str()) || *level > 10 {
                    out.push(Diagnostic::new(
                        "E-STRUCT",
                        format!(
                            "actor `{}` has invalid skill `{skill}` at level {level}",
                            actor.id
                        ),
                    ));
                }
            }
        }
    }
}

/// E-STRUCT — check that weapons have range_bands sorted ascending.
fn check_weapon_structure(content: &Content, out: &mut Vec<Diagnostic>) {
    for weapon in content.weapons.values() {
        let bands = &weapon.range_bands;
        if bands.len() < 2 {
            out.push(Diagnostic::new(
                "E-STRUCT",
                format!(
                    "weapon `{}` has fewer than 2 range bands ({:?})",
                    weapon.id, bands
                ),
            ));
            continue;
        }
        for i in 1..bands.len() {
            if bands[i] <= bands[i - 1] {
                out.push(Diagnostic::new(
                    "E-STRUCT",
                    format!(
                        "weapon `{}` range_bands not strictly ascending: {:?}",
                        weapon.id, bands
                    ),
                ));
                break;
            }
        }
    }
}

/// E-STRUCT — check that DiceRoll values have count >= 1 and sides >= 1.
fn check_dice_rolls(content: &Content, out: &mut Vec<Diagnostic>) {
    for weapon in content.weapons.values() {
        check_dice("weapon", &weapon.id, &weapon.damage_dice, out);
    }

    // Also check dice rolls in scenario actors? ActorData doesn't have
    // a DiceRoll field directly — only weapons do.
}

fn check_dice(context: &str, id: &str, roll: &DiceRoll, out: &mut Vec<Diagnostic>) {
    if roll.count < 1 {
        out.push(Diagnostic::new(
            "E-STRUCT",
            format!(
                "{} `{}` has invalid DiceRoll count ({}, expected >= 1)",
                context, id, roll.count
            ),
        ));
    }
    if roll.sides < 1 {
        out.push(Diagnostic::new(
            "E-STRUCT",
            format!(
                "{} `{}` has invalid DiceRoll sides ({}, expected >= 1)",
                context, id, roll.sides
            ),
        ));
    }
}

/// E-STRUCT — check that faction_id and archetype_id in ActorData are
/// non-empty.
fn check_faction_and_archetype(content: &Content, out: &mut Vec<Diagnostic>) {
    for scenario in content.scenarios.values() {
        for actor in &scenario.actors {
            if actor.faction_id.is_empty() {
                out.push(Diagnostic::new(
                    "E-STRUCT",
                    format!(
                        "actor `{}` in scenario `{}` has empty faction_id",
                        actor.id, scenario.id
                    ),
                ));
            }
            if actor.archetype_id.is_empty() {
                out.push(Diagnostic::new(
                    "E-STRUCT",
                    format!(
                        "actor `{}` in scenario `{}` has empty archetype_id",
                        actor.id, scenario.id
                    ),
                ));
            }
        }
        let mut battle_line_ids = BTreeSet::new();
        for line in &scenario.battle_dialogue {
            if line.id.trim().is_empty()
                || line.speaker.trim().is_empty()
                || line.text.trim().is_empty()
            {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!(
                        "scenario `{}` has an in-battle dialogue line with an empty field",
                        scenario.id
                    ),
                ));
            }
            if !battle_line_ids.insert(line.id.as_str()) {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-STORY",
                    format!(
                        "scenario `{}` repeats in-battle dialogue id `{}`",
                        scenario.id, line.id
                    ),
                ));
            }
            if !matches!(
                line.trigger.as_str(),
                "first_player_move"
                    | "first_player_shot"
                    | "first_player_hit"
                    | "first_enemy_hit"
                    | "first_missed_shot"
                    | "first_ally_wounded"
                    | "first_enemy_down"
                    | "first_ally_down"
                    | "first_critical"
                    | "first_smoke"
                    | "first_ally_routed"
            ) {
                out.push(Diagnostic::new(
                    "E-CAMPAIGN-TRIGGER",
                    format!(
                        "scenario `{}` uses unknown in-battle dialogue trigger `{}`",
                        scenario.id, line.trigger
                    ),
                ));
            }
        }
    }
}

/// E-STRUCT — check that scenario objectives reference actor_ids that
/// actually exist in the scenario's actor list.
fn check_objective_actor_references(content: &Content, out: &mut Vec<Diagnostic>) {
    for scenario in content.scenarios.values() {
        let actor_ids: BTreeSet<&str> = scenario.actors.iter().map(|a| a.id.as_str()).collect();

        for objective in &scenario.objectives {
            for target_id in &objective.actor_ids {
                if !actor_ids.contains(target_id.as_str()) {
                    out.push(Diagnostic::new(
                        "E-STRUCT",
                        format!(
                            "scenario `{}` objective `{}` references unknown actor `{}`",
                            scenario.id, objective.id, target_id
                        ),
                    ));
                }
            }
        }
    }
}

/// E-ANACHRONISM-001 — check that weapons equipped in a scenario existed
/// on or before the scenario date.  Parses the first four characters of
/// `ScenarioData.date` as a year and compares against `WeaponData.first_year_available`.
fn check_anachronisms(content: &Content, out: &mut Vec<Diagnostic>) {
    for scenario in content.scenarios.values() {
        // Parse the scenario date: format is "YYYY-MM-DD".
        let scenario_year: u16 = match scenario.date.get(..4).and_then(|y| y.parse().ok()) {
            Some(y) => y,
            None => continue, // unparseable date — skip silently
        };

        for actor in &scenario.actors {
            // Check equipped_primary.
            if let Some(ref wid) = actor.equipped_primary {
                if let Some(weapon) = content.weapons.get(wid.as_str()) {
                    if weapon.first_year_available > scenario_year {
                        out.push(Diagnostic::new(
                            "E-ANACHRONISM-001",
                            format!(
                                "actor `{}` in scenario `{}` (dated {}) is equipped with \
                                 `{}` which was not available until {}",
                                actor.id,
                                scenario.id,
                                scenario.date,
                                wid,
                                weapon.first_year_available,
                            ),
                        ));
                    }
                }
            }

            // Check equipped_sidearm.
            if let Some(ref wid) = actor.equipped_sidearm {
                if let Some(weapon) = content.weapons.get(wid.as_str()) {
                    if weapon.first_year_available > scenario_year {
                        out.push(Diagnostic::new(
                            "E-ANACHRONISM-001",
                            format!(
                                "actor `{}` in scenario `{}` (dated {}) is equipped with \
                                 `{}` which was not available until {}",
                                actor.id,
                                scenario.id,
                                scenario.date,
                                wid,
                                weapon.first_year_available,
                            ),
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::manual_range_contains,
    clippy::option_map_unit_fn
)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::schema::{
        ActorData, Attributes, CampaignNodeData, CompanionData, DiceRoll, ItemStack, MapData,
        ObjectiveData, ScenarioData, TileXYData, WeaponData,
    };

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn empty_content() -> Content {
        Content::default()
    }

    fn dummy_actor(id: &str) -> ActorData {
        ActorData {
            id: id.into(),
            archetype_id: "scout".into(),
            faction_id: "player".into(),
            attributes: Attributes {
                grit: 5,
                nerve: 5,
                wind: 5,
                hands: 5,
                eyes: 5,
                savvy: 5,
                luck: 5,
            },
            level: 1,
            xp: 0,
            skill_points: 0,
            skill_levels: std::collections::BTreeMap::new(),
            hp: 20,
            hp_max: 20,
            sand: 10,
            sand_max: 10,
            ap: 6,
            ap_max: 6,
            sequence: 0,
            pos: TileXYData { x: 0, y: 0 },
            facing: 0,
            stance: "standing".into(),
            wounds: vec![],
            inventory: vec![],
            equipped_primary: None,
            equipped_sidearm: None,
            marks: vec![],
            ways: vec![],
            is_companion: false,
            is_dead: false,
        }
    }

    fn dummy_weapon(id: &str) -> WeaponData {
        WeaponData {
            id: id.into(),
            display_name: id.into(),
            damage_dice: DiceRoll {
                count: 2,
                sides: 6,
                bonus: 0,
            },
            accuracy: 0,
            range_bands: [5, 15, 30, 50],
            ap_override: BTreeMap::new(),
            capacity: 15,
            reload_class: "tube_magazine".into(),
            fouling_rate: 3,
            base_misfire: 2,
            smoke_output: 1,
            two_handed: true,
            first_year_available: 1873,
            historical_note: String::new(),
            sources: vec!["core".into()],
        }
    }

    fn dummy_scenario(
        id: &str,
        actors: Vec<ActorData>,
        objectives: Vec<ObjectiveData>,
    ) -> ScenarioData {
        ScenarioData {
            id: id.into(),
            display_name: "Test".into(),
            briefing: Vec::new(),
            prebattle_dialogue: Vec::new(),
            battle_dialogue: Vec::new(),
            score: None,
            date: "1867-10-21".into(),
            map: MapData {
                width: 10,
                height: 10,
                tiles: BTreeMap::new(),
            },
            light: "daylight".into(),
            weather: "clear".into(),
            wind_dir: "north".into(),
            deployment_zones: BTreeMap::new(),
            actors,
            objectives,
            victory_conditions: vec![],
            defeat_conditions: vec![],
            historical_tag: None,
            citations: vec![],
        }
    }

    fn dummy_campaign_node(id: &str) -> CampaignNodeData {
        CampaignNodeData {
            id: id.into(),
            kind: "story".into(),
            date: "1867-10-21".into(),
            requires: vec![],
            grants: vec![],
            unlocks: vec![],
            historical_tag: None,
            citations: vec![],
            companion_gates: vec![],
            recruits: vec![],
            scenario_id: None,
        }
    }

    fn dummy_companion(id: &str) -> CompanionData {
        CompanionData {
            id: id.into(),
            display_name: id.into(),
            nation: None,
            community: None,
            sources: vec!["core".into()],
            attributes: Attributes {
                grit: 5,
                nerve: 5,
                wind: 5,
                hands: 5,
                eyes: 5,
                savvy: 5,
                luck: 5,
            },
            starting_weapons: vec![],
            arc_anchor: "test".into(),
            dialogue_file: "test.ron".into(),
        }
    }

    fn objective_data(id: &str, actor_ids: Vec<String>) -> ObjectiveData {
        ObjectiveData {
            id: id.into(),
            description: "test objective".into(),
            kind: "eliminate".into(),
            actor_ids,
        }
    }

    // ------------------------------------------------------------------
    // Empty content — valid
    // ------------------------------------------------------------------

    #[test]
    fn empty_content_is_valid() {
        let result = validate(&empty_content());
        assert!(
            result.is_empty(),
            "empty content should be valid: {:?}",
            result
        );
    }

    // ------------------------------------------------------------------
    // Duplicate IDs
    // ------------------------------------------------------------------

    #[test]
    fn duplicate_id_across_collections() {
        let mut content = empty_content();
        content.scenarios.insert(
            "shared_id".into(),
            dummy_scenario("shared_id", vec![], vec![]),
        );
        content
            .weapons
            .insert("shared_id".into(), dummy_weapon("shared_id"));

        let result = validate(&content);
        let diags: Vec<&Diagnostic> = result.iter().filter(|d| d.code == "E-DUP-ID").collect();

        assert_eq!(diags.len(), 1, "should find one duplicate ID group");
        assert!(diags[0].message.contains("shared_id"));
        assert!(diags[0].message.contains("scenarios"));
        assert!(diags[0].message.contains("weapons"));
    }

    #[test]
    fn no_duplicate_ids_when_disjoint() {
        let mut content = empty_content();
        content
            .scenarios
            .insert("scen_1".into(), dummy_scenario("scen_1", vec![], vec![]));
        content
            .weapons
            .insert("wpn_1".into(), dummy_weapon("wpn_1"));
        content
            .companions
            .insert("comp_1".into(), dummy_companion("comp_1"));
        content
            .campaign_nodes
            .insert("node_1".into(), dummy_campaign_node("node_1"));

        let diags = validate(&content);
        let dup_diags: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == "E-DUP-ID").collect();
        assert!(
            dup_diags.is_empty(),
            "no duplicates expected: {:?}",
            dup_diags
        );
    }

    // ------------------------------------------------------------------
    // Weapon references
    // ------------------------------------------------------------------

    #[test]
    fn unknown_weapon_in_equipped_primary() {
        let actor = ActorData {
            equipped_primary: Some("nonexistent_gun".into()),
            ..dummy_actor("p1")
        };
        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> =
            diags.iter().filter(|d| d.code == "E-REF-WEAPON").collect();
        assert_eq!(ref_diags.len(), 1);
        assert!(ref_diags[0].message.contains("nonexistent_gun"));
    }

    #[test]
    fn unknown_weapon_in_inventory() {
        let actor = ActorData {
            inventory: vec![ItemStack {
                item_id: "ghost_gun".into(),
                count: 1,
            }],
            ..dummy_actor("p2")
        };
        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> =
            diags.iter().filter(|d| d.code == "E-REF-WEAPON").collect();
        assert_eq!(ref_diags.len(), 1);
        assert!(ref_diags[0].message.contains("ghost_gun"));
    }

    #[test]
    fn known_weapon_reference_is_valid() {
        let actor = ActorData {
            equipped_primary: Some("winchester_73".into()),
            equipped_sidearm: Some("colt_peacemaker".into()),
            inventory: vec![ItemStack {
                item_id: "knife".into(),
                count: 1,
            }],
            ..dummy_actor("p1")
        };
        let mut content = empty_content();
        content
            .weapons
            .insert("winchester_73".into(), dummy_weapon("winchester_73"));
        content
            .weapons
            .insert("colt_peacemaker".into(), dummy_weapon("colt_peacemaker"));
        content
            .weapons
            .insert("knife".into(), dummy_weapon("knife"));
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> =
            diags.iter().filter(|d| d.code == "E-REF-WEAPON").collect();
        assert!(
            ref_diags.is_empty(),
            "known weapons should not produce errors: {:?}",
            ref_diags
        );
    }

    // ------------------------------------------------------------------
    // Companion references
    // ------------------------------------------------------------------

    #[test]
    fn unknown_companion_in_gate() {
        let mut node = dummy_campaign_node("node_a");
        node.companion_gates.push("ghost_companion".into());

        let mut content = empty_content();
        content.campaign_nodes.insert("node_a".into(), node);

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == "E-REF-COMPANION")
            .collect();
        assert_eq!(ref_diags.len(), 1);
        assert!(ref_diags[0].message.contains("ghost_companion"));
    }

    #[test]
    fn known_companion_gate_is_valid() {
        let mut node = dummy_campaign_node("node_a");
        node.companion_gates.push("elena".into());

        let mut content = empty_content();
        content
            .companions
            .insert("elena".into(), dummy_companion("elena"));
        content.campaign_nodes.insert("node_a".into(), node);

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == "E-REF-COMPANION")
            .collect();
        assert!(ref_diags.is_empty(), "known companion: {:?}", ref_diags);
    }

    // ------------------------------------------------------------------
    // Scenario references
    // ------------------------------------------------------------------

    #[test]
    fn unknown_scenario_in_campaign_node() {
        let mut node = dummy_campaign_node("node_a");
        node.scenario_id = Some("missing_scenario".into());

        let mut content = empty_content();
        content.campaign_nodes.insert("node_a".into(), node);

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == "E-REF-SCENARIO")
            .collect();
        assert_eq!(ref_diags.len(), 1);
        assert!(ref_diags[0].message.contains("missing_scenario"));
    }

    #[test]
    fn known_scenario_reference_is_valid() {
        let mut node = dummy_campaign_node("node_a");
        node.scenario_id = Some("s1".into());

        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![], vec![]));
        content.campaign_nodes.insert("node_a".into(), node);

        let diags = validate(&content);
        let ref_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == "E-REF-SCENARIO")
            .collect();
        assert!(ref_diags.is_empty(), "known scenario: {:?}", ref_diags);
    }

    #[test]
    fn campaign_scenario_date_mismatch_is_reported() {
        let mut node = dummy_campaign_node("mission_a");
        node.kind = "Mission".into();
        node.date = "1868-10-21".into();
        node.scenario_id = Some("s1".into());

        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![], vec![]));
        content.campaign_nodes.insert("mission_a".into(), node);

        let diags = validate(&content);
        assert!(
            diags
                .iter()
                .any(|diagnostic| diagnostic.code == "E-CAMPAIGN-DATE"
                    && diagnostic.message.contains("mission_a")),
            "date drift should be rejected: {:?}",
            diags
        );
    }

    // ------------------------------------------------------------------
    // Actor structure
    // ------------------------------------------------------------------

    #[test]
    fn empty_actor_id_reported() {
        let actor = ActorData {
            id: String::new(),
            ..dummy_actor("")
        };
        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let struct_diags: Vec<&Diagnostic> =
            diags.iter().filter(|d| d.code == "E-STRUCT").collect();
        let has_empty_id = struct_diags.iter().any(|d| d.message.contains("empty id"));
        assert!(has_empty_id, "should report empty actor id");
    }

    #[test]
    fn empty_faction_id_reported() {
        let actor = ActorData {
            faction_id: String::new(),
            ..dummy_actor("p1")
        };
        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let has_faction_diag = diags.iter().any(|d| d.message.contains("empty faction_id"));
        assert!(has_faction_diag);
    }

    #[test]
    fn empty_archetype_id_reported() {
        let actor = ActorData {
            archetype_id: String::new(),
            ..dummy_actor("p1")
        };
        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![]));

        let diags = validate(&content);
        let has_arch_diag = diags
            .iter()
            .any(|d| d.message.contains("empty archetype_id"));
        assert!(has_arch_diag);
    }

    // ------------------------------------------------------------------
    // Weapon structure — range_bands
    // ------------------------------------------------------------------

    #[test]
    fn unsorted_range_bands_reported() {
        let mut wpn = dummy_weapon("bad_wpn");
        wpn.range_bands = [50, 30, 15, 5]; // descending

        let mut content = empty_content();
        content.weapons.insert("bad_wpn".into(), wpn);

        let diags = validate(&content);
        let has_bands_diag = diags.iter().any(|d| {
            d.message.contains("range_bands") && d.message.contains("not strictly ascending")
        });
        assert!(has_bands_diag);
    }

    #[test]
    fn sorted_range_bands_ok() {
        let wpn = dummy_weapon("good_wpn"); // [5, 15, 30, 50] is ascending

        let mut content = empty_content();
        content.weapons.insert("good_wpn".into(), wpn);

        let diags = validate(&content);
        let bands_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.message.contains("range_bands"))
            .collect();
        assert!(bands_diags.is_empty(), "sorted bands: {:?}", bands_diags);
    }

    // ------------------------------------------------------------------
    // DiceRoll structure
    // ------------------------------------------------------------------

    #[test]
    fn invalid_dice_count_reported() {
        let mut wpn = dummy_weapon("bad_dice");
        wpn.damage_dice.count = 0; // invalid

        let mut content = empty_content();
        content.weapons.insert("bad_dice".into(), wpn);

        let diags = validate(&content);
        let has_dice_diag = diags
            .iter()
            .any(|d| d.message.contains("invalid DiceRoll count"));
        assert!(has_dice_diag);
    }

    #[test]
    fn invalid_dice_sides_reported() {
        let mut wpn = dummy_weapon("bad_sides");
        wpn.damage_dice.sides = 0; // invalid

        let mut content = empty_content();
        content.weapons.insert("bad_sides".into(), wpn);

        let diags = validate(&content);
        let has_dice_diag = diags
            .iter()
            .any(|d| d.message.contains("invalid DiceRoll sides"));
        assert!(has_dice_diag);
    }

    #[test]
    fn valid_dice_ok() {
        let wpn = dummy_weapon("good_dice"); // count=2, sides=6

        let mut content = empty_content();
        content.weapons.insert("good_dice".into(), wpn);

        let diags = validate(&content);
        let dice_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.message.contains("DiceRoll"))
            .collect();
        assert!(dice_diags.is_empty(), "valid dice: {:?}", dice_diags);
    }

    // ------------------------------------------------------------------
    // Objective actor references
    // ------------------------------------------------------------------

    #[test]
    fn objective_references_unknown_actor() {
        let actor = dummy_actor("hero");
        let obj = objective_data("obj1", vec!["ghost".into()]);

        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![obj]));

        let diags = validate(&content);
        let obj_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.message.contains("objective") && d.message.contains("unknown actor"))
            .collect();
        assert_eq!(obj_diags.len(), 1);
        assert!(obj_diags[0].message.contains("ghost"));
    }

    #[test]
    fn objective_references_known_actor() {
        let actor = dummy_actor("hero");
        let obj = objective_data("obj1", vec!["hero".into()]);

        let mut content = empty_content();
        content
            .scenarios
            .insert("s1".into(), dummy_scenario("s1", vec![actor], vec![obj]));

        let diags = validate(&content);
        let obj_diags: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.message.contains("objective"))
            .collect();
        assert!(obj_diags.is_empty(), "known actor: {:?}", obj_diags);
    }

    // ------------------------------------------------------------------
    // Comprehensive valid content
    // ------------------------------------------------------------------

    #[test]
    fn comprehensive_valid_content() {
        let actor = ActorData {
            id: "hero".into(),
            equipped_primary: Some("rifle".into()),
            ..dummy_actor("hero")
        };
        let obj = objective_data("obj1", vec!["hero".into()]);
        let mut scenario = dummy_scenario("s1", vec![actor], vec![obj]);
        // The test scenario uses "1867-10-21" but test weapon has
        // first_year_available = 1873 — set the scenario date to match.
        scenario.date = "1873-04-14".into();

        let mut node = dummy_campaign_node("node_a");
        node.scenario_id = Some("s1".into());
        node.date = "1873-04-14".into();
        node.companion_gates.push("elena".into());

        let mut content = empty_content();
        content.scenarios.insert("s1".into(), scenario);
        content
            .weapons
            .insert("rifle".into(), dummy_weapon("rifle"));
        content
            .companions
            .insert("elena".into(), dummy_companion("elena"));
        content.campaign_nodes.insert("node_a".into(), node);

        let diags = validate(&content);
        assert!(diags.is_empty(), "comprehensive valid content: {:?}", diags);
    }

    // ------------------------------------------------------------------
    // Panic safety — validate never panics
    // ------------------------------------------------------------------

    #[test]
    fn validate_never_panics_on_bogus_input() {
        // Build content with all kinds of edge cases.
        let mut content = empty_content();

        // Empty strings for IDs.
        content
            .scenarios
            .insert(String::new(), dummy_scenario("", vec![], vec![]));
        content.weapons.insert(String::new(), dummy_weapon(""));

        // Actor with empty fields.
        let broken_actor = ActorData {
            id: String::new(),
            archetype_id: String::new(),
            faction_id: String::new(),
            equipped_primary: Some(String::new()),
            equipped_sidearm: Some(String::new()),
            inventory: vec![ItemStack {
                item_id: String::new(),
                count: 0,
            }],
            ..dummy_actor("")
        };
        content.scenarios.get_mut("").map(|s| {
            s.actors.push(broken_actor);
        });

        // Weapon with invalid dice and range bands.
        let mut bad_wpn = dummy_weapon("bad");
        bad_wpn.range_bands = [0, 0, 0, 0];
        bad_wpn.damage_dice = DiceRoll {
            count: 0,
            sides: 0,
            bonus: 0,
        };
        content.weapons.insert("bad".into(), bad_wpn);

        // Campaign node referencing missing IDs.
        let mut node = dummy_campaign_node("lonely_node");
        node.companion_gates.push("nobody".into());
        node.scenario_id = Some("nowhere".into());
        content.campaign_nodes.insert("lonely_node".into(), node);

        // Should not panic.
        let diags = validate(&content);
        // We expect many diagnostics — just confirm no panic.
        assert!(
            !diags.is_empty(),
            "edge case content should have diagnostics"
        );
    }
}
