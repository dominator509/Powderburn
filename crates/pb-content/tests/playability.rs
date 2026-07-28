//! Campaign-wide tactical playability contract.

use std::path::Path;

#[test]
fn every_campaign_mission_has_a_playable_scenario_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let Ok(content) = pb_content::load::load_all(&root) else {
        panic!("content must load");
    };
    let missions = content
        .campaign_nodes
        .values()
        .filter(|node| node.kind == "Mission")
        .collect::<Vec<_>>();
    assert_eq!(
        missions.len(),
        24,
        "the four-act campaign must have 24 missions"
    );

    for node in missions {
        let Some(scenario_id) = node.scenario_id.as_deref() else {
            panic!("every Mission node needs a scenario");
        };
        let Some(scenario) = content.scenarios.get(scenario_id) else {
            panic!("campaign scenario must exist");
        };
        let living_opponents = scenario
            .actors
            .iter()
            .filter(|actor| {
                !actor.is_dead && !matches!(actor.faction_id.as_str(), "player" | "ally")
            })
            .count();
        eprintln!(
            "{} scenario={} map={}x{} tiles={} actors={} opponents={} objectives={} zones={}",
            node.id,
            scenario.id,
            scenario.map.width,
            scenario.map.height,
            scenario.map.tiles.len(),
            scenario.actors.len(),
            living_opponents,
            scenario.objectives.len(),
            scenario.deployment_zones.len(),
        );
        assert!(scenario.map.width >= 12 && scenario.map.height >= 8);
        assert!(
            living_opponents >= 2,
            "{} needs at least two authored opponents",
            node.id
        );
        assert!(
            !scenario.objectives.is_empty(),
            "{} needs an authored objective",
            node.id
        );
        assert!(
            !scenario.victory_conditions.is_empty() && !scenario.defeat_conditions.is_empty(),
            "{} needs explicit victory and defeat conditions",
            node.id
        );
        assert!(
            scenario
                .deployment_zones
                .values()
                .filter(|zone| !zone.is_empty())
                .count()
                >= 2,
            "{} needs two non-empty deployment sides",
            node.id
        );
    }
}
