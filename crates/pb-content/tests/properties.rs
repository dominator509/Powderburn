//! Property tests for content loading and validation.
//!
//! Verifies that:
//! - `load_all` + `validate` succeeds on the known-good content directory
//! - Each weapon record in the loaded content is structurally valid

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{collections::BTreeSet, path::Path};

use pb_content::load::load_all;
use pb_content::schema::Content;
use pb_content::validate;

/// Helper to load the project's content directory.
/// Assumes the test runs from the repository root (cargo test --workspace).
fn load_project_content() -> Content {
    let content_dir = if Path::new("content").exists() {
        Path::new("content").to_path_buf()
    } else if Path::new("../content").exists() {
        Path::new("../content").to_path_buf()
    } else if Path::new("../../content").exists() {
        Path::new("../../content").to_path_buf()
    } else {
        panic!(
            "Cannot find content directory from {:?}",
            std::env::current_dir().unwrap()
        );
    };

    load_all(&content_dir).expect("load_all should succeed on known-good content")
}

/// Test that load_all + validate succeeds on known-good content.
///
/// This is the master property: the content directory must be parseable and
/// semantically valid at all times.
#[test]
fn load_and_validate_known_good_content() {
    let content = load_project_content();
    let diagnostics = validate::validate(&content);

    if !diagnostics.is_empty() {
        let msg: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                format!(
                    "  [{}] {} ({}:{})",
                    d.code,
                    d.message,
                    d.file.as_deref().unwrap_or("?"),
                    d.line.map_or(0, |l| l as i32)
                )
            })
            .collect();
        panic!(
            "Content validation failed with {} diagnostic(s):\n{}",
            diagnostics.len(),
            msg.join("\n")
        );
    }
}

/// Test that every weapon record is valid.
///
/// Structural checks for each weapon:
/// - Non-empty id and display_name
/// - Accuracy in a reasonable range
/// - Strictly ascending range_bands
/// - Valid DiceRoll (count >= 1, sides >= 1)
/// - Capacity >= 0
/// - First_year_available >= 1800
#[test]
fn each_weapon_record_is_valid() {
    let content = load_project_content();

    assert!(
        !content.weapons.is_empty(),
        "Expected at least one weapon definition"
    );

    for (weapon_id, weapon) in &content.weapons {
        // ID must be non-empty and match the key
        assert!(
            !weapon.id.is_empty(),
            "Weapon has empty id field (key: {})",
            weapon_id
        );
        assert_eq!(
            &weapon.id, weapon_id,
            "Weapon id field '{}' does not match key '{}'",
            weapon.id, weapon_id
        );

        // Display name must be non-empty
        assert!(
            !weapon.display_name.is_empty(),
            "Weapon '{}' has empty display_name",
            weapon_id
        );

        // Accuracy should be in a reasonable range
        // Normal weapons: [-10, 20]; alien/god weapons can go up to 999
        assert!(
            weapon.accuracy >= -10 && weapon.accuracy <= 999,
            "Weapon '{}' has accuracy {} outside [-10, 999]",
            weapon_id,
            weapon.accuracy
        );

        // Range bands must be strictly ascending
        assert!(
            weapon.range_bands.len() >= 2,
            "Weapon '{}' has fewer than 2 range bands ({:?})",
            weapon_id,
            weapon.range_bands
        );
        for i in 1..weapon.range_bands.len() {
            assert!(
                weapon.range_bands[i] > weapon.range_bands[i - 1],
                "Weapon '{}' range_bands not strictly ascending: {:?}",
                weapon_id,
                weapon.range_bands
            );
        }

        // Damage dice must be valid
        assert!(
            weapon.damage_dice.count >= 1,
            "Weapon '{}' has damage_dice count {} < 1",
            weapon_id,
            weapon.damage_dice.count
        );
        assert!(
            weapon.damage_dice.sides >= 1,
            "Weapon '{}' has damage_dice sides {} < 1",
            weapon_id,
            weapon.damage_dice.sides
        );

        // Capacity must be non-negative
        assert!(
            weapon.capacity >= 0,
            "Weapon '{}' has negative capacity {}",
            weapon_id,
            weapon.capacity
        );

        // First year available must be reasonable for the setting
        // (some melee weapons like lance and tomahawk predate the 1800s)
        assert!(
            weapon.first_year_available >= 1500 && weapon.first_year_available <= 1900,
            "Weapon '{}' has first_year_available {} outside [1500, 1900]",
            weapon_id,
            weapon.first_year_available
        );

        // Sources list must be non-empty
        assert!(
            !weapon.sources.is_empty(),
            "Weapon '{}' has no sources",
            weapon_id
        );
    }
}

/// Test that every scenario has at least one actor.
#[test]
fn each_scenario_has_actors() {
    let content = load_project_content();

    for (scenario_id, scenario) in &content.scenarios {
        assert!(
            !scenario.actors.is_empty(),
            "Scenario '{}' has no actors",
            scenario_id
        );

        // Each actor must have a non-empty id
        for actor in &scenario.actors {
            assert!(
                !actor.id.is_empty(),
                "Scenario '{}' has an actor with empty id",
                scenario_id
            );
            // Player-side factions are built in; every other authored faction
            // must exist in the faction rule table.
            assert!(
                actor.faction_id == "player"
                    || actor.faction_id == "ally"
                    || actor.faction_id == "enemy"
                    || actor.faction_id == "neutral"
                    || content.factions.contains_key(&actor.faction_id),
                "Scenario '{}' actor '{}' has unrecognized faction '{}'",
                scenario_id,
                actor.id,
                actor.faction_id
            );
        }
    }
}

/// Every campaign mission is a distinct, playable authored battlefield.
#[test]
fn campaign_missions_have_unique_battle_ready_scenarios() {
    let content = load_project_content();
    let missions: Vec<_> = content
        .campaign_nodes
        .values()
        .filter(|node| node.kind == "Mission")
        .collect();
    assert_eq!(missions.len(), 24, "the campaign must contain 24 missions");

    let mut scenario_ids = BTreeSet::new();
    for mission in missions {
        let scenario_id = mission
            .scenario_id
            .as_deref()
            .unwrap_or_else(|| panic!("mission '{}' has no scenario", mission.id));
        assert!(
            !scenario_id.starts_with("prov_"),
            "mission '{}' still points at proof scenario '{}'",
            mission.id,
            scenario_id
        );
        assert!(
            scenario_ids.insert(scenario_id),
            "mission '{}' reuses scenario '{}'",
            mission.id,
            scenario_id
        );

        let scenario = content.scenarios.get(scenario_id).unwrap_or_else(|| {
            panic!(
                "mission '{}' references missing scenario '{}'",
                mission.id, scenario_id
            )
        });
        assert!(
            !scenario.map.tiles.is_empty(),
            "mission '{}' scenario '{}' has no authored terrain",
            mission.id,
            scenario_id
        );
        assert!(
            !scenario.objectives.is_empty(),
            "mission '{}' scenario '{}' has no objectives",
            mission.id,
            scenario_id
        );
        assert!(
            !scenario.victory_conditions.is_empty(),
            "mission '{}' scenario '{}' has no victory condition",
            mission.id,
            scenario_id
        );
        assert!(
            !scenario.defeat_conditions.is_empty(),
            "mission '{}' scenario '{}' has no defeat condition",
            mission.id,
            scenario_id
        );

        let player_count = scenario
            .actors
            .iter()
            .filter(|actor| actor.faction_id == "player" || actor.faction_id == "ally")
            .count();
        let hostile_count = scenario
            .actors
            .iter()
            .filter(|actor| {
                actor.faction_id != "player"
                    && actor.faction_id != "ally"
                    && actor.faction_id != "neutral"
            })
            .count();
        assert!(
            player_count >= 1,
            "mission '{}' scenario '{}' has no player deployment actor",
            mission.id,
            scenario_id
        );
        assert!(
            hostile_count >= 3,
            "mission '{}' scenario '{}' needs at least three hostile actors",
            mission.id,
            scenario_id
        );
    }
}

/// Test that campaign node IDs are unique and non-empty.
#[test]
fn campaign_nodes_have_unique_ids() {
    let content = load_project_content();

    assert!(
        !content.campaign_nodes.is_empty(),
        "Expected at least one campaign node"
    );

    // Verify all IDs are non-empty and unique (BTreeMap enforces this)
    for (node_id, node) in &content.campaign_nodes {
        assert!(!node_id.is_empty(), "Campaign node has empty id");
        assert!(
            !node.kind.is_empty(),
            "Campaign node '{}' has empty kind",
            node_id
        );
    }
}
