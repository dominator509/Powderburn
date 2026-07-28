//! Campaign graph: node traversal, flag resolution, and permadeath propagation.
//! See SPEC-002 section 5 for campaign flags and SPEC-000 section 5.4 for structure.
#![forbid(unsafe_code)]

use crate::schema::{CampaignNodeData, Content};
use std::collections::{BTreeMap, BTreeSet};

/// A campaign graph that can be traversed.
#[derive(Debug, Clone)]
pub struct CampaignGraph {
    pub nodes: BTreeMap<String, CampaignNodeData>,
}

/// Build a campaign graph from the loaded content.
pub fn build_graph(content: &Content) -> CampaignGraph {
    CampaignGraph {
        nodes: content.campaign_nodes.clone(),
    }
}

/// Compute the set of reachable node IDs given the current campaign flags and dead companions.
pub fn compute_reachable(
    graph: &CampaignGraph,
    flags: &[String],
    dead_companions: &[String],
) -> Vec<String> {
    let flag_set: BTreeSet<&str> = flags.iter().map(|s| s.as_str()).collect();
    let dead_set: BTreeSet<&str> = dead_companions.iter().map(|s| s.as_str()).collect();
    let mut reachable = BTreeSet::new();
    let mut frontier: Vec<&str> = Vec::new();
    let incoming = incoming_edges(graph);

    // Only graph roots start traversal. A flag predicate alone must never make
    // an unrelated node reachable.
    for (id, node) in &graph.nodes {
        if !incoming.contains_key(id)
            && requirements_met(node, &flag_set)
            && !companion_blocked(node, &dead_set)
        {
            frontier.push(id);
        }
    }

    while let Some(current_id) = frontier.pop() {
        if !reachable.insert(current_id) {
            continue;
        }

        let Some(node) = graph.nodes.get(current_id) else {
            continue;
        };

        for unlocked in &node.unlocks {
            let Some(candidate) = graph.nodes.get(unlocked) else {
                continue;
            };
            if requirements_met(candidate, &flag_set)
                && !companion_blocked(candidate, &dead_set)
                && !reachable.contains(unlocked.as_str())
                && !frontier.contains(&unlocked.as_str())
            {
                frontier.push(unlocked);
            }
        }
    }

    reachable.into_iter().map(str::to_string).collect()
}

/// Check permadeath propagation: for each dead companion, verify that no reachable
/// dialogue scene can still select a line spoken by them.
pub fn check_permadeath_propagation(
    graph: &CampaignGraph,
    content: &Content,
    dead_companions: &[String],
    flags: &[String],
) -> Vec<String> {
    let reachable = compute_reachable(graph, flags, dead_companions);
    let reachable_set: BTreeSet<&str> = reachable.iter().map(String::as_str).collect();
    let dead_set: BTreeSet<&str> = dead_companions.iter().map(String::as_str).collect();
    let flag_set: BTreeSet<&str> = flags.iter().map(String::as_str).collect();
    let mut violations = Vec::new();

    for scene in content.dialogue.values() {
        if scene
            .node_id
            .as_deref()
            .is_some_and(|node| !reachable_set.contains(node))
            || !scene
                .requires_flags
                .iter()
                .all(|flag| flag_set.contains(flag.as_str()))
        {
            continue;
        }

        // An alive gate makes the complete scene unreachable after that death.
        if scene
            .requires_alive
            .iter()
            .any(|companion| dead_set.contains(companion.as_str()))
        {
            continue;
        }

        for line in &scene.lines {
            if dead_set.contains(line.speaker_id.as_str()) {
                violations.push(format!(
                    "dialogue scene `{}` leaves dead companion `{}` reachable",
                    scene.id, line.speaker_id
                ));
            }
        }
    }

    violations.sort();
    violations.dedup();
    violations
}

/// Get the next mission in the campaign progression.
pub fn next_missions(graph: &CampaignGraph, flags: &[String], completed: &[String]) -> Vec<String> {
    next_missions_with_deaths(graph, flags, completed, &[])
}

/// Get available missions while enforcing permanent companion deaths.
pub fn next_missions_with_deaths(
    graph: &CampaignGraph,
    flags: &[String],
    completed: &[String],
    dead_companions: &[String],
) -> Vec<String> {
    let flag_set: BTreeSet<&str> = flags.iter().map(|s| s.as_str()).collect();
    let completed_set: BTreeSet<&str> = completed.iter().map(|s| s.as_str()).collect();
    let dead_set: BTreeSet<&str> = dead_companions.iter().map(String::as_str).collect();
    let incoming = incoming_edges(graph);
    let mut available = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut frontier = Vec::new();

    if completed_set.is_empty() {
        frontier.extend(
            graph
                .nodes
                .keys()
                .filter(|id| !incoming.contains_key(*id))
                .cloned(),
        );
    } else {
        for completed_id in &completed_set {
            if let Some(node) = graph.nodes.get(*completed_id) {
                frontier.extend(node.unlocks.iter().cloned());
            }
        }
    }

    while let Some(id) = frontier.pop() {
        if !visited.insert(id.clone()) || completed_set.contains(id.as_str()) {
            continue;
        }
        let Some(node) = graph.nodes.get(&id) else {
            continue;
        };
        if !requirements_met(node, &flag_set) || companion_blocked(node, &dead_set) {
            continue;
        }
        match node.kind.as_str() {
            "Mission" => {
                available.insert(id);
            }
            "Choice" => {
                // An unresolved choice is a hard campaign boundary. Only the
                // explicitly completed branch may expose its descendants.
            }
            _ => {
                // Camps and historical beats are interludes, not combat
                // missions. Traverse through them for the read-only query.
                frontier.extend(node.unlocks.iter().cloned());
            }
        }
    }

    available.into_iter().collect()
}

/// Result of advancing every deterministic non-combat node currently reached.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CampaignAdvance {
    pub missions: Vec<String>,
    pub choices: Vec<String>,
    pub interludes_completed: Vec<String>,
}

/// Complete reached Camp and HistoricalBeat nodes, applying their authored
/// grants, until the campaign reaches a mission or an explicit player choice.
///
/// Choice nodes are never auto-completed. This is the state-mutating companion
/// to `next_missions_with_deaths`; clients use it when progressing a save.
pub fn advance_interludes(
    graph: &CampaignGraph,
    flags: &mut Vec<String>,
    completed: &mut Vec<String>,
    dead_companions: &[String],
) -> Result<CampaignAdvance, String> {
    let dead_set: BTreeSet<&str> = dead_companions.iter().map(String::as_str).collect();
    let incoming = incoming_edges(graph);
    let mut result = CampaignAdvance::default();

    loop {
        let flag_values = flags.clone();
        let completed_values = completed.clone();
        let flag_set: BTreeSet<&str> = flag_values.iter().map(String::as_str).collect();
        let completed_set: BTreeSet<&str> = completed_values.iter().map(String::as_str).collect();
        let mut frontier = BTreeSet::new();
        if completed_set.is_empty() {
            frontier.extend(
                graph
                    .nodes
                    .keys()
                    .filter(|id| !incoming.contains_key(*id))
                    .cloned(),
            );
        } else {
            for completed_id in &completed_set {
                if let Some(node) = graph.nodes.get(*completed_id) {
                    frontier.extend(node.unlocks.iter().cloned());
                }
            }
        }

        let mut completed_an_interlude = false;
        result.missions.clear();
        result.choices.clear();
        for id in frontier {
            if completed_set.contains(id.as_str()) {
                continue;
            }
            let Some(node) = graph.nodes.get(&id) else {
                return Err(format!("campaign node `{id}` is missing"));
            };
            if !requirements_met(node, &flag_set) || companion_blocked(node, &dead_set) {
                continue;
            }
            match node.kind.as_str() {
                "Mission" => result.missions.push(id),
                "Choice" => {
                    if !choice_sibling_selected(graph, &id, &completed_set, &incoming) {
                        result.choices.push(id);
                    }
                }
                _ => {
                    if complete_node(graph, &id, flags, completed)? {
                        result.interludes_completed.push(id);
                        completed_an_interlude = true;
                    }
                }
            }
        }
        if !completed_an_interlude {
            result.missions.sort();
            result.choices.sort();
            result.interludes_completed.sort();
            result.interludes_completed.dedup();
            return Ok(result);
        }
    }
}

/// Return structural campaign-graph defects independent of runtime flags.
pub fn structural_issues(graph: &CampaignGraph, expected_root: &str) -> Vec<String> {
    let incoming = incoming_edges(graph);
    let roots: Vec<&str> = graph
        .nodes
        .keys()
        .filter(|id| !incoming.contains_key(*id))
        .map(String::as_str)
        .collect();
    let mut issues = Vec::new();
    if roots != [expected_root] {
        issues.push(format!(
            "campaign must have exactly root `{expected_root}`, found {}",
            roots.join(",")
        ));
    }
    if !graph.nodes.contains_key(expected_root) {
        return issues;
    }

    let mut visited = BTreeSet::new();
    let mut frontier = vec![expected_root];
    while let Some(id) = frontier.pop() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(node) = graph.nodes.get(id) {
            for unlocked in &node.unlocks {
                if graph.nodes.contains_key(unlocked) {
                    frontier.push(unlocked);
                }
            }
        }
    }
    for id in graph.nodes.keys() {
        if !visited.contains(id.as_str()) {
            issues.push(format!(
                "campaign node `{id}` is unreachable from `{expected_root}`"
            ));
        }
    }
    issues
}

/// Apply a completed node exactly once and return whether state changed.
///
/// Campaign flags are written only from the node's authored `grants`, as
/// required by SPEC-002 section 5.
pub fn complete_node(
    graph: &CampaignGraph,
    node_id: &str,
    flags: &mut Vec<String>,
    completed: &mut Vec<String>,
) -> Result<bool, String> {
    let node = graph
        .nodes
        .get(node_id)
        .ok_or_else(|| format!("unknown campaign node `{node_id}`"))?;
    if completed.iter().any(|id| id == node_id) {
        return Ok(false);
    }
    for flag in &node.grants {
        if !flags.contains(flag) {
            flags.push(flag.clone());
        }
    }
    flags.sort();
    flags.dedup();
    completed.push(node_id.to_string());
    completed.sort();
    completed.dedup();
    Ok(true)
}

fn incoming_edges(graph: &CampaignGraph) -> BTreeMap<String, Vec<String>> {
    let mut incoming: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (source, node) in &graph.nodes {
        for target in &node.unlocks {
            incoming
                .entry(target.clone())
                .or_default()
                .push(source.clone());
        }
    }
    incoming
}

/// Return whether an alternative choice reached from the same authored
/// predecessor has already been completed.
///
/// Choice nodes are exclusive branches: completing one must permanently close
/// its siblings even though their shared predecessor remains in the completed
/// history.
fn choice_sibling_selected(
    graph: &CampaignGraph,
    choice_id: &str,
    completed: &BTreeSet<&str>,
    incoming: &BTreeMap<String, Vec<String>>,
) -> bool {
    let Some(predecessors) = incoming.get(choice_id) else {
        return false;
    };
    predecessors.iter().any(|predecessor| {
        graph.nodes.get(predecessor).is_some_and(|node| {
            node.unlocks.iter().any(|sibling| {
                sibling != choice_id
                    && completed.contains(sibling.as_str())
                    && graph
                        .nodes
                        .get(sibling)
                        .is_some_and(|candidate| candidate.kind == "Choice")
            })
        })
    })
}

fn requirements_met(node: &CampaignNodeData, flags: &BTreeSet<&str>) -> bool {
    node.requires
        .iter()
        .all(|requirement| flags.contains(requirement.as_str()))
}

fn companion_blocked(node: &CampaignNodeData, dead: &BTreeSet<&str>) -> bool {
    node.companion_gates
        .iter()
        .any(|companion| dead.contains(companion.as_str()))
}
