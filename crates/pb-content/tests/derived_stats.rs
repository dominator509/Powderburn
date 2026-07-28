use std::path::Path;

use pb_content::load::load_all;

#[test]
fn shipped_actor_derived_stats_match_attributes() {
    let Ok(content) = load_all(Path::new("../../content")) else {
        panic!("shipped content must load");
    };
    let mut issues = Vec::new();
    for scenario in content.scenarios.values() {
        for actor in &scenario.actors {
            let attributes = actor.attributes;
            let expected_hp = attributes.hit_points(actor.level);
            let expected_sand = attributes.sand();
            let expected_ap = attributes.action_points() as i16;
            let expected_sequence = attributes.sequence();
            if actor.hp_max != expected_hp {
                issues.push(format!(
                    "{}:{} hp_max={} expected={expected_hp}",
                    scenario.id, actor.id, actor.hp_max
                ));
            }
            if actor.sand_max != expected_sand {
                issues.push(format!(
                    "{}:{} sand_max={} expected={expected_sand}",
                    scenario.id, actor.id, actor.sand_max
                ));
            }
            if actor.ap_max != expected_ap {
                issues.push(format!(
                    "{}:{} ap_max={} expected={expected_ap}",
                    scenario.id, actor.id, actor.ap_max
                ));
            }
            if actor.sequence != expected_sequence {
                issues.push(format!(
                    "{}:{} sequence={} expected={expected_sequence}",
                    scenario.id, actor.id, actor.sequence
                ));
            }
            if actor.hp > actor.hp_max || actor.sand > actor.sand_max || actor.ap > actor.ap_max {
                issues.push(format!(
                    "{}:{} current stat exceeds its maximum",
                    scenario.id, actor.id
                ));
            }
        }
    }
    assert!(issues.is_empty(), "{}", issues.join("\n"));
}
