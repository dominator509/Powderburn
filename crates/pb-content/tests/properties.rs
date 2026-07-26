//! Property tests for content loading and validation.
//!
//! Verifies that:
//! - `load_all` + `validate` succeeds on the known-good content directory
//! - Each weapon record in the loaded content is structurally valid

use std::path::Path;

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
        assert!(
            weapon.accuracy >= -10 && weapon.accuracy <= 20,
            "Weapon '{}' has accuracy {} outside [-10, 20]",
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
            // Each actor must have at least one objective reference or be self-contained
            assert!(
                actor.faction_id == "player"
                    || actor.faction_id == "enemy"
                    || actor.faction_id == "neutral",
                "Scenario '{}' actor '{}' has unrecognized faction '{}'",
                scenario_id,
                actor.id,
                actor.faction_id
            );
        }
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
