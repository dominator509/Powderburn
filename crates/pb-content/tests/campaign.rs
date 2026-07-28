use std::collections::BTreeMap;

use pb_content::{
    campaign::{
        advance_interludes, complete_node, compute_reachable, next_missions, structural_issues,
        CampaignGraph,
    },
    load::load_all,
    schema::CampaignNodeData,
};

fn node(
    id: &str,
    kind: &str,
    requires: &[&str],
    grants: &[&str],
    unlocks: &[&str],
    companion_gates: &[&str],
) -> CampaignNodeData {
    CampaignNodeData {
        id: id.to_string(),
        kind: kind.to_string(),
        date: "1867-01-01".to_string(),
        requires: requires.iter().map(|value| (*value).to_string()).collect(),
        grants: grants.iter().map(|value| (*value).to_string()).collect(),
        unlocks: unlocks.iter().map(|value| (*value).to_string()).collect(),
        historical_tag: None,
        citations: Vec::new(),
        companion_gates: companion_gates
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        recruits: Vec::new(),
        scenario_id: (kind == "Mission").then(|| format!("scn_{id}")),
    }
}

fn graph() -> CampaignGraph {
    CampaignGraph {
        nodes: BTreeMap::from([
            (
                "m01".to_string(),
                node("m01", "Mission", &[], &["a1_started"], &["camp01"], &[]),
            ),
            (
                "camp01".to_string(),
                node(
                    "camp01",
                    "Camp",
                    &["a1_started"],
                    &["a1_rested"],
                    &["m02"],
                    &[],
                ),
            ),
            (
                "m02".to_string(),
                node("m02", "Mission", &["a1_rested"], &[], &[], &["c_scout"]),
            ),
            (
                "unrelated".to_string(),
                node("unrelated", "Mission", &["a1_started"], &[], &[], &[]),
            ),
            (
                "blocked_path".to_string(),
                node(
                    "blocked_path",
                    "HistoricalBeat",
                    &["never"],
                    &[],
                    &["unrelated"],
                    &[],
                ),
            ),
        ]),
    }
}

#[test]
fn flags_do_not_unlock_nodes_without_a_graph_edge() {
    let graph = graph();
    let available = next_missions(&graph, &["a1_started".into()], &["m01".into()]);
    assert!(
        available.is_empty(),
        "camp interlude must resolve before m02"
    );
    assert!(
        !available.contains(&"unrelated".to_string()),
        "a satisfied predicate must not unlock an unrelated root after play begins"
    );
}

#[test]
fn completion_writes_only_authored_grants_and_is_idempotent() {
    let graph = graph();
    let mut flags = Vec::new();
    let mut completed = Vec::new();
    assert_eq!(
        complete_node(&graph, "m01", &mut flags, &mut completed),
        Ok(true)
    );
    assert_eq!(flags, ["a1_started"]);
    assert_eq!(completed, ["m01"]);
    assert_eq!(
        complete_node(&graph, "m01", &mut flags, &mut completed),
        Ok(false)
    );
    assert_eq!(flags, ["a1_started"]);
}

#[test]
fn reachable_respects_edges_requirements_and_dead_companion_gates() {
    let graph = graph();
    let flags = vec!["a1_started".into(), "a1_rested".into()];
    let alive = compute_reachable(&graph, &flags, &[]);
    assert!(alive.contains(&"m02".to_string()));
    assert!(!alive.contains(&"unrelated".to_string()));

    let dead = compute_reachable(&graph, &flags, &["c_scout".into()]);
    assert!(!dead.contains(&"m02".to_string()));
}

#[test]
fn interludes_are_traversed_when_selecting_the_next_mission() {
    let graph = graph();
    let available = next_missions(
        &graph,
        &["a1_started".into(), "a1_rested".into()],
        &["m01".into()],
    );
    assert_eq!(available, ["m02"]);
}

#[test]
fn unresolved_choice_blocks_missions_and_selected_branch_closes_its_sibling() {
    let graph = CampaignGraph {
        nodes: BTreeMap::from([
            (
                "opening".to_string(),
                node(
                    "opening",
                    "Mission",
                    &[],
                    &["choice_ready"],
                    &["spare", "kill"],
                    &[],
                ),
            ),
            (
                "spare".to_string(),
                node(
                    "spare",
                    "Choice",
                    &["choice_ready"],
                    &["spared"],
                    &["spare_end"],
                    &[],
                ),
            ),
            (
                "kill".to_string(),
                node(
                    "kill",
                    "Choice",
                    &["choice_ready"],
                    &["killed"],
                    &["kill_end"],
                    &[],
                ),
            ),
            (
                "spare_end".to_string(),
                node("spare_end", "Mission", &["spared"], &[], &[], &[]),
            ),
            (
                "kill_end".to_string(),
                node("kill_end", "Mission", &["killed"], &[], &[], &[]),
            ),
        ]),
    };
    let mut flags = vec!["choice_ready".to_string()];
    let mut completed = vec!["opening".to_string()];

    assert!(
        next_missions(&graph, &flags, &completed).is_empty(),
        "an unresolved player choice must not be traversed"
    );
    let Ok(advance) = advance_interludes(&graph, &mut flags, &mut completed, &[]) else {
        panic!("choice frontier should resolve");
    };
    assert_eq!(advance.choices, ["kill", "spare"]);

    assert_eq!(
        complete_node(&graph, "spare", &mut flags, &mut completed),
        Ok(true)
    );
    let Ok(advance) = advance_interludes(&graph, &mut flags, &mut completed, &[]) else {
        panic!("selected branch should resolve");
    };
    assert_eq!(advance.missions, ["spare_end"]);
    assert!(
        advance.choices.is_empty(),
        "the unselected sibling must remain closed"
    );
}

#[test]
fn shipped_campaign_is_one_connected_graph_from_the_opening() {
    let Ok(content) = load_all(std::path::Path::new("../../content")) else {
        panic!("shipped content must load");
    };
    let graph = pb_content::campaign::build_graph(&content);
    assert_eq!(
        structural_issues(&graph, "m01_elk_creek"),
        Vec::<String>::new()
    );
    assert_eq!(
        graph
            .nodes
            .values()
            .filter(|node| node.kind == "Mission")
            .count(),
        24
    );
    assert_eq!(
        graph
            .nodes
            .values()
            .filter(|node| node.kind == "Camp")
            .count(),
        12
    );
}

fn walk_shipped_branch(
    graph: &CampaignGraph,
    choice_node: &str,
) -> std::collections::BTreeSet<String> {
    let mut flags = Vec::new();
    let mut completed = Vec::new();
    let dead = Vec::new();

    for _ in 0..100 {
        let Ok(advance) = advance_interludes(graph, &mut flags, &mut completed, &dead) else {
            panic!("shipped campaign must advance");
        };
        if !advance.choices.is_empty() {
            assert!(
                advance.choices.iter().any(|choice| choice == choice_node),
                "unexpected choice frontier: {:?}",
                (advance.choices, completed.clone(), flags.clone())
            );
            assert_eq!(
                complete_node(graph, choice_node, &mut flags, &mut completed),
                Ok(true)
            );
            continue;
        }
        if advance.missions.is_empty() {
            break;
        }
        for mission in advance.missions {
            assert_eq!(
                complete_node(graph, &mission, &mut flags, &mut completed),
                Ok(true)
            );
        }
    }

    completed
        .into_iter()
        .filter(|id| {
            graph
                .nodes
                .get(id)
                .is_some_and(|node| node.kind == "Mission")
        })
        .collect()
}

#[test]
fn both_authored_branches_collectively_reach_all_twenty_four_missions() {
    let Ok(content) = load_all(std::path::Path::new("../../content")) else {
        panic!("shipped content must load");
    };
    let graph = pb_content::campaign::build_graph(&content);
    let warned = walk_shipped_branch(&graph, "a2_choice_warn_adobe_walls");
    let contracted = walk_shipped_branch(&graph, "a2_choice_take_hide_contract");
    let union: std::collections::BTreeSet<&String> = warned.union(&contracted).collect();

    assert_eq!(warned.len(), 23);
    assert_eq!(contracted.len(), 23);
    assert_eq!(union.len(), 24);
    assert!(warned.contains("m12_adobe_walls_relief"));
    assert!(!warned.contains("m12_hide_yard_reckoning"));
    assert!(contracted.contains("m12_hide_yard_reckoning"));
    assert!(!contracted.contains("m12_adobe_walls_relief"));
    assert!(warned.contains("m003_adobe_walls"));
    assert!(contracted.contains("m003_adobe_walls"));
    assert!(warned.contains("m24_ledger_reckoning"));
    assert!(contracted.contains("m24_ledger_reckoning"));
}

#[test]
fn shipped_campaign_is_chronological_and_visits_all_twelve_camps() {
    let Ok(content) = load_all(std::path::Path::new("../../content")) else {
        panic!("shipped content must load");
    };
    let graph = pb_content::campaign::build_graph(&content);
    let mut flags = Vec::new();
    let mut completed = Vec::new();
    let dead = Vec::new();
    let mut previous_date = String::new();
    let mut played_missions = Vec::new();

    for _ in 0..100 {
        let Ok(advance) = advance_interludes(&graph, &mut flags, &mut completed, &dead) else {
            panic!("campaign advancement must succeed");
        };
        if !advance.choices.is_empty() {
            assert!(
                advance
                    .choices
                    .iter()
                    .any(|choice| choice == "a2_choice_warn_adobe_walls"),
                "unexpected choice frontier: {:?}",
                (advance.choices, completed.clone(), flags.clone())
            );
            assert!(complete_node(
                &graph,
                "a2_choice_warn_adobe_walls",
                &mut flags,
                &mut completed
            )
            .is_ok());
            continue;
        }
        if advance.missions.is_empty() {
            break;
        }
        assert_eq!(
            advance.missions.len(),
            1,
            "the authored main path must expose one chronological mission at a time"
        );
        let mission = advance.missions[0].clone();
        let Some(node) = graph.nodes.get(&mission) else {
            panic!("available mission must exist");
        };
        assert!(
            previous_date.is_empty() || node.date >= previous_date,
            "mission '{}' at {} regresses behind {}",
            mission,
            node.date,
            previous_date
        );
        previous_date = node.date.clone();
        played_missions.push(mission.clone());
        assert!(complete_node(&graph, &mission, &mut flags, &mut completed).is_ok());
    }

    assert_eq!(
        played_missions.len(),
        23,
        "one complete playthrough must include 23 of 24 missions (one of two endings)"
    );
    assert_eq!(
        completed
            .iter()
            .filter(|id| graph.nodes.get(*id).is_some_and(|node| node.kind == "Camp"))
            .count(),
        12
    );
    assert_eq!(
        played_missions.last().map(String::as_str),
        Some("m24_ledger_reckoning")
    );
}
