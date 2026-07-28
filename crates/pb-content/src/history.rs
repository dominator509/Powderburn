//! Historical immutability checks for authoring fixtures and mods.

use std::collections::BTreeSet;

use crate::schema::{ActorData, Content};

/// Return fixed scenario/actor pairs that an override would alter.
///
/// Unrelated records are accepted; only an actor ID already present in a
/// scenario referenced by a `HISTORICAL_FIXED` campaign node is immutable.
pub fn actor_override_violations(
    content: &Content,
    overrides: &[ActorData],
) -> Vec<(String, String)> {
    let fixed_scenarios: BTreeSet<_> = content
        .campaign_nodes
        .values()
        .filter(|node| node.historical_tag.as_deref() == Some("HISTORICAL_FIXED"))
        .filter_map(|node| node.scenario_id.as_deref())
        .collect();
    let override_ids: BTreeSet<_> = overrides.iter().map(|actor| actor.id.as_str()).collect();

    let mut violations = Vec::new();
    for scenario_id in fixed_scenarios {
        if let Some(scenario) = content.scenarios.get(scenario_id) {
            for actor in &scenario.actors {
                if override_ids.contains(actor.id.as_str()) {
                    violations.push((scenario_id.to_string(), actor.id.clone()));
                }
            }
        }
    }
    violations
}
