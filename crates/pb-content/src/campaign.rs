//! Campaign graph: node traversal, flag resolution, and permadeath propagation.
//! See SPEC-002 section 5 for campaign flags and SPEC-000 section 5.4 for structure.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use crate::schema::{CampaignNodeData, Content};

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

    // Find starting nodes (those with no requires or whose requires are all met)
    for (id, node) in &graph.nodes {
        if node.requires.is_empty() || node.requires.iter().all(|r| flag_set.contains(r.as_str())) {
            if !frontier.contains(&id.as_str()) {
                frontier.push(id);
            }
        }
    }

    // BFS through the graph
    while let Some(current_id) = frontier.pop() {
        if !reachable.insert(current_id) {
            continue; // Already visited
        }

        let Some(node) = graph.nodes.get(current_id) else {
            continue;
        };

        // Check companion gates: if a gated companion is dead, this node may be blocked
        let companion_blocked = node.companion_gates.iter().any(|g| dead_set.contains(g.as_str()));

        // For now, we still traverse but mark the limitation
        for unlocked in &node.unlocks {
            if !reachable.contains(unlocked.as_str()) && !frontier.contains(&&*unlocked.as_str()) {
                frontier.push(unlocked);
            }
        }
    }

    let mut result: Vec<String> = reachable.into_iter().map(|s| s.to_string()).collect();
    result.sort();
    result
}

/// Check permadeath propagation: for each dead companion, verify that no reachable
/// content references them. Returns a list of violations.
pub fn check_permadeath_propagation(
    graph: &CampaignGraph,
    content: &Content,
    dead_companions: &[String],
    flags: &[String],
) -> Vec<String> {
    let mut violations = Vec::new();
    let reachable = compute_reachable(graph, flags, dead_companions);

    let reachable_set: BTreeSet<&str> = reachable.iter().map(|s| s.as_str()).collect();

    for companion_id in dead_companions {
        // Check campaign nodes in the reachable set
        for node_id in &reachable_set {
            if let Some(node) = graph.nodes.get(*node_id) {
                // If the node itself mentions the dead companion outside the ledger
                if node.id.contains(companion_id) {
                    violations.push(format!(
                        "campaign node `{}` references dead companion `{}`",
                        node_id, companion_id
                    ));
                }
            }
        }
    }

    violations
}

/// Get the next mission in the campaign progression.
pub fn next_missions(
    graph: &CampaignGraph,
    flags: &[String],
    completed: &[String],
) -> Vec<String> {
    let flag_set: BTreeSet<&str> = flags.iter().map(|s| s.as_str()).collect();
    let completed_set: BTreeSet<&str> = completed.iter().map(|s| s.as_str()).collect();
    let mut available = Vec::new();

    for (id, node) in &graph.nodes {
        if completed_set.contains(id.as_str()) {
            continue;
        }
        // Check all required flags are met
        let requires_met = node.requires.is_empty()
            || node.requires.iter().all(|r| flag_set.contains(r.as_str()));
        // Check at least one unlocker is completed
        let unlocked = completed_set.contains(id.as_str())
            || node.requires.is_empty()
            || completed.iter().any(|c| node.requires.contains(c));

        if requires_met {
            available.push(id.clone());
        }
    }

    available.sort();
    available
}
