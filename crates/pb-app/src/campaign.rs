//! Graphical-client campaign progression backed by the shared save model.

#![forbid(unsafe_code)]

use std::path::Path;

use pb_content::campaign::{advance_interludes, build_graph, complete_node};
use pb_content::schema::{
    ActorData, Content, ItemStack, LedgerEntryData, SaveFileData, ScenarioData, TileXYData,
};
use pb_save::ledger::{LedgerChain, LedgerEntry};

use crate::state::GameState;

const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Resolve a campaign graph node to its authored playable scenario.
///
/// `GameState::current_mission` intentionally stores the graph node ID so
/// campaign completion and Ledger writes retain their authored identity.
/// Presentation and combat must cross this resolver before reading scenarios.
pub fn scenario_for_mission<'a>(
    content: &'a Content,
    mission_id: &str,
) -> Result<&'a ScenarioData, String> {
    let node = content
        .campaign_nodes
        .get(mission_id)
        .ok_or_else(|| format!("E-CAMPAIGN-GRAPH: missing mission node {mission_id}"))?;
    let scenario_id = node.scenario_id.as_deref().ok_or_else(|| {
        format!("E-CAMPAIGN-CONTENT: mission node {mission_id} has no playable scenario")
    })?;
    content.scenarios.get(scenario_id).ok_or_else(|| {
        format!(
            "E-CAMPAIGN-CONTENT: mission node {mission_id} references missing scenario {scenario_id}"
        )
    })
}

fn ledger_weight_increment(newly_written: u32, ledger_keeper: bool) -> u32 {
    newly_written.saturating_mul(if ledger_keeper { 1 } else { 2 })
}

#[cfg(test)]
pub fn start_new(state: &mut GameState, content_root: &Path, seed: u64) -> Result<(), String> {
    start_new_inner(state, content_root, seed, None)
}

pub fn start_new_with_way(
    state: &mut GameState,
    content_root: &Path,
    seed: u64,
    way_id: &str,
) -> Result<(), String> {
    start_new_inner(state, content_root, seed, Some(way_id))
}

fn start_new_inner(
    state: &mut GameState,
    content_root: &Path,
    seed: u64,
    way_id: Option<&str>,
) -> Result<(), String> {
    let ruleset_hash = pb_content::hash::ruleset_hash(content_root)
        .map_err(|error| format!("E-CAMPAIGN-HASH: {error}"))?;
    let content_hash = pb_content::hash::content_hash(content_root)
        .map_err(|error| format!("E-CAMPAIGN-HASH: {error}"))?;
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let company = initial_company(&content, way_id)?;
    state.campaign = Some(SaveFileData {
        format_version: 1,
        ruleset_hash: pb_content::hash::hex(&ruleset_hash),
        content_hash: pb_content::hash::hex(&content_hash),
        campaign_seed: seed,
        ledger_head_hash: ZERO_HASH.to_string(),
        ledger_weight: 0,
        ledger_entries: Vec::new(),
        campaign_flags: Vec::new(),
        completed_nodes: Vec::new(),
        company,
        sim_snapshot: None,
        written_at_tick: 0,
    });
    state.current_mission = None;
    state.last_victory = None;
    Ok(())
}

fn initial_company(
    content: &pb_content::schema::Content,
    way_id: Option<&str>,
) -> Result<Vec<ActorData>, String> {
    let Some(elias) = content.companions.get("c_elias") else {
        return Ok(vec![new_company_actor(
            "c_elias",
            pb_core::Attributes::BALANCED,
            "colt_army_1860",
            false,
        )]);
    };
    let weapon = elias
        .starting_weapons
        .first()
        .map_or("colt_army_1860", String::as_str);
    let mut actor = new_company_actor(&elias.id, elias.attributes, weapon, false);
    if let Some(way_id) = way_id {
        let way = content
            .ways
            .get(way_id)
            .ok_or_else(|| format!("E-CAMPAIGN-WAY: unknown authored Way {way_id}"))?;
        actor.ways.push(way.id.clone());
        actor
            .inventory
            .extend(way.starting_items.iter().map(|item_id| ItemStack {
                item_id: item_id.clone(),
                count: 1,
            }));
    }
    Ok(vec![actor])
}

fn sync_recruits(
    campaign: &mut SaveFileData,
    content: &pb_content::schema::Content,
) -> Result<(), String> {
    let mut recruits = campaign
        .completed_nodes
        .iter()
        .filter_map(|node_id| content.campaign_nodes.get(node_id))
        .flat_map(|node| node.recruits.iter().cloned())
        .collect::<Vec<_>>();
    recruits.sort();
    recruits.dedup();
    for recruit_id in recruits {
        if campaign.company.iter().any(|actor| actor.id == recruit_id) {
            continue;
        }
        let companion = content
            .companions
            .get(&recruit_id)
            .ok_or_else(|| format!("E-CAMPAIGN-CONTENT: unknown authored recruit {recruit_id}"))?;
        let weapon = companion
            .starting_weapons
            .first()
            .map_or("colt_army_1860", String::as_str);
        campaign.company.push(new_company_actor(
            &companion.id,
            companion.attributes,
            weapon,
            true,
        ));
    }
    campaign.company.sort_by(|left, right| {
        left.is_companion
            .cmp(&right.is_companion)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(())
}

fn new_company_actor(
    id: &str,
    attributes: pb_core::Attributes,
    weapon: &str,
    is_companion: bool,
) -> ActorData {
    let level = 1;
    let hp_max = attributes.hit_points(level);
    let sand_max = attributes.sand();
    let ap_max = attributes.action_points().clamp(0, i32::from(i16::MAX)) as i16;
    ActorData {
        id: id.to_string(),
        archetype_id: if is_companion {
            "companion".to_string()
        } else {
            "player".to_string()
        },
        faction_id: "player".to_string(),
        attributes,
        level,
        xp: 0,
        skill_points: 0,
        skill_levels: std::collections::BTreeMap::new(),
        hp: hp_max,
        hp_max,
        sand: sand_max,
        sand_max,
        ap: ap_max,
        ap_max,
        sequence: attributes.sequence(),
        pos: TileXYData { x: 0, y: 0 },
        facing: 2,
        stance: "Standing".to_string(),
        wounds: Vec::new(),
        inventory: Vec::new(),
        equipped_primary: Some(weapon.to_string()),
        equipped_sidearm: None,
        marks: Vec::new(),
        ways: Vec::new(),
        is_companion,
        is_dead: false,
    }
}

pub fn select_next(state: &mut GameState, content_root: &Path) -> Result<String, String> {
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let campaign = state
        .campaign
        .as_mut()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no active campaign".to_string())?;
    let dead = campaign
        .company
        .iter()
        .filter(|actor| actor.is_companion && actor.is_dead)
        .map(|actor| actor.id.clone())
        .collect::<Vec<_>>();
    let advance = advance_interludes(
        &build_graph(&content),
        &mut campaign.campaign_flags,
        &mut campaign.completed_nodes,
        &dead,
    )
    .map_err(|error| format!("E-CAMPAIGN-GRAPH: {error}"))?;
    sync_recruits(campaign, &content)?;
    if !advance.choices.is_empty() {
        if state.ledger_verification == pb_save::load::LedgerVerification::Unverified {
            return Err(
                "E-CAMPAIGN-UNVERIFIED: Ledger-based ending disabled for this save".to_string(),
            );
        }
        return Err(format!(
            "E-CAMPAIGN-CHOICE: choose one of {}",
            advance.choices.join(",")
        ));
    }
    let selected = advance
        .missions
        .first()
        .cloned()
        .ok_or_else(|| "E-CAMPAIGN-COMPLETE: no mission remains".to_string())?;
    state.current_mission = Some(selected.clone());
    Ok(selected)
}

/// Return the currently reached authored choice nodes.
///
/// Reached camps and historical beats are advanced exactly as they are when
/// selecting a mission, so the map screen and Enter-key path cannot disagree.
pub fn current_choices(state: &mut GameState, content_root: &Path) -> Result<Vec<String>, String> {
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let campaign = state
        .campaign
        .as_mut()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no active campaign".to_string())?;
    let dead = campaign
        .company
        .iter()
        .filter(|actor| actor.is_companion && actor.is_dead)
        .map(|actor| actor.id.clone())
        .collect::<Vec<_>>();
    let advance = advance_interludes(
        &build_graph(&content),
        &mut campaign.campaign_flags,
        &mut campaign.completed_nodes,
        &dead,
    )
    .map_err(|error| format!("E-CAMPAIGN-GRAPH: {error}"))?;
    sync_recruits(campaign, &content)?;
    Ok(advance.choices)
}

/// Complete one reached choice by its stable, sorted map-screen index.
pub fn choose_branch(
    state: &mut GameState,
    content_root: &Path,
    choice_index: usize,
) -> Result<String, String> {
    if state.ledger_verification == pb_save::load::LedgerVerification::Unverified {
        return Err(
            "E-CAMPAIGN-UNVERIFIED: campaign choices are disabled for this save".to_string(),
        );
    }
    let choices = current_choices(state, content_root)?;
    let choice = choices
        .get(choice_index)
        .cloned()
        .ok_or_else(|| format!("E-CAMPAIGN-CHOICE: no choice at index {choice_index}"))?;
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let campaign = state
        .campaign
        .as_mut()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no active campaign".to_string())?;
    complete_node(
        &build_graph(&content),
        &choice,
        &mut campaign.campaign_flags,
        &mut campaign.completed_nodes,
    )
    .map_err(|error| format!("E-CAMPAIGN-GRAPH: {error}"))?;
    sync_recruits(campaign, &content)?;
    state.message = format!("Campaign choice recorded: {choice}");
    Ok(choice)
}

fn dialogue_requirements_met(
    campaign: &SaveFileData,
    scene: &pb_content::schema::DialogueSceneData,
) -> bool {
    scene
        .requires_flags
        .iter()
        .all(|flag| campaign.campaign_flags.contains(flag))
        && scene.requires_alive.iter().all(|required| {
            campaign
                .company
                .iter()
                .any(|actor| actor.id == *required && !actor.is_dead)
        })
        && scene
            .min_ledger_weight
            .is_none_or(|minimum| campaign.ledger_weight >= minimum)
        && scene
            .max_ledger_weight
            .is_none_or(|maximum| campaign.ledger_weight <= maximum)
}

/// Return all currently legal camp dialogue, ordered by authored scene ID.
pub fn available_camp_dialogue<'a>(
    content: &'a pb_content::schema::Content,
    campaign: &SaveFileData,
) -> Vec<&'a pb_content::schema::DialogueSceneData> {
    let latest_camp = campaign
        .completed_nodes
        .iter()
        .filter_map(|id| content.campaign_nodes.get(id))
        .filter(|node| node.kind == "Camp")
        .max_by(|left, right| {
            left.date
                .cmp(&right.date)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|node| node.id.as_str());
    content
        .dialogue
        .values()
        .filter(|scene| scene.node_id.as_deref() == latest_camp)
        .filter(|scene| dialogue_requirements_met(campaign, scene))
        .collect()
}

/// Build the authored three-line Ledger prompts for every newly named death.
/// Idempotent so every path into AfterAction may call it safely.
pub fn prepare_ledger_writes(state: &mut GameState, content_root: &Path) -> Result<(), String> {
    if !state.pending_ledger_writes.is_empty() {
        return Ok(());
    }
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let campaign = state
        .campaign
        .as_ref()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no active campaign".to_string())?;
    let mission = state
        .current_mission
        .as_ref()
        .ok_or_else(|| "E-CAMPAIGN-STATE: battle has no campaign node".to_string())?;
    let node = content
        .campaign_nodes
        .get(mission)
        .ok_or_else(|| format!("E-CAMPAIGN-GRAPH: missing node {mission}"))?;
    let place = node
        .scenario_id
        .as_ref()
        .and_then(|id| content.scenarios.get(id))
        .map_or(mission.as_str(), |scenario| scenario.display_name.as_str());
    let sim = state
        .sim
        .as_ref()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no completed simulation".to_string())?;

    let mut pending = Vec::new();
    for actor in sim.actors.values().filter(|actor| !actor.alive) {
        if campaign
            .ledger_entries
            .iter()
            .any(|entry| entry.name == actor.name)
        {
            continue;
        }
        let category = if actor.is_companion {
            "companion"
        } else {
            "combatant"
        };
        let line_set = content
            .ledger_line_sets
            .values()
            .find(|set| set.applies_to == category)
            .ok_or_else(|| {
                format!("E-CONTENT-002: no Ledger line set for category `{category}`")
            })?;
        pending.push(crate::state::PendingLedgerWrite {
            name: actor.name.clone(),
            role: if actor.is_companion {
                "Companion".to_string()
            } else {
                "Combatant".to_string()
            },
            place: place.to_string(),
            date: node.date.clone(),
            lines: line_set.lines.clone(),
            selected_index: line_set.default_index,
        });
    }
    pending.sort_by(|left, right| left.name.cmp(&right.name));
    state.pending_ledger_writes = pending;
    state.ledger_write_cursor = 0;
    Ok(())
}

pub fn choose_ledger_line(state: &mut GameState, index: u8) -> Result<(), String> {
    if index >= 3 {
        return Err("E-LEDGER-CHOICE: line index must be 1, 2, or 3".to_string());
    }
    let count = state.pending_ledger_writes.len();
    let entry = state
        .pending_ledger_writes
        .get_mut(state.ledger_write_cursor)
        .ok_or_else(|| "E-LEDGER-CHOICE: no pending named death".to_string())?;
    entry.selected_index = index;
    if state.ledger_write_cursor + 1 < count {
        state.ledger_write_cursor += 1;
    }
    Ok(())
}

pub fn finish_battle(state: &mut GameState, content_root: &Path) -> Result<(), String> {
    let victory = state
        .last_victory
        .ok_or_else(|| "E-CAMPAIGN-STATE: battle has no outcome".to_string())?;
    if !victory {
        return Ok(());
    }
    let mission = state
        .current_mission
        .clone()
        .ok_or_else(|| "E-CAMPAIGN-STATE: battle has no campaign node".to_string())?;
    let content = pb_content::load::load_all(content_root)
        .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))?;
    let graph = build_graph(&content);
    let campaign = state
        .campaign
        .as_mut()
        .ok_or_else(|| "E-CAMPAIGN-STATE: no active campaign".to_string())?;
    if campaign.completed_nodes.iter().any(|node| node == &mission) {
        return Err(format!(
            "E-CAMPAIGN-STATE: mission '{mission}' is already resolved"
        ));
    }
    complete_node(
        &graph,
        &mission,
        &mut campaign.campaign_flags,
        &mut campaign.completed_nodes,
    )
    .map_err(|error| format!("E-CAMPAIGN-GRAPH: {error}"))?;
    sync_recruits(campaign, &content)?;

    let node = graph
        .nodes
        .get(&mission)
        .ok_or_else(|| format!("E-CAMPAIGN-GRAPH: missing node {mission}"))?;
    let scenario = node
        .scenario_id
        .as_ref()
        .and_then(|id| content.scenarios.get(id));
    let place = scenario.map_or(mission.as_str(), |scenario| scenario.display_name.as_str());
    let primary_objectives =
        u32::try_from(scenario.map_or(0, |scenario| scenario.objectives.len())).unwrap_or(u32::MAX);

    let previous_ledger_len = campaign.ledger_entries.len();
    let mut ledger = ledger_from_data(&campaign.ledger_entries)?;
    if let Some(sim) = &state.sim {
        let no_company_casualties =
            campaign
                .company
                .iter()
                .filter(|actor| !actor.is_dead)
                .all(|company_actor| {
                    sim.actors
                        .values()
                        .find(|runtime| runtime.name == company_actor.id)
                        .is_none_or(|runtime| runtime.alive)
                });
        let xp_award =
            crate::camp::compute_after_action_xp(primary_objectives, 0, 0, no_company_casualties)
                .total_xp;
        for company_actor in &mut campaign.company {
            let Some((runtime_actor_id, runtime_actor)) = sim
                .actors
                .iter()
                .find(|(_, runtime)| runtime.name == company_actor.id)
            else {
                continue;
            };
            company_actor.hp = runtime_actor.hit_points.max(0);
            company_actor.hp_max = runtime_actor.max_hp;
            company_actor.sand = runtime_actor.sand.max(0);
            company_actor.sand_max = runtime_actor.max_sand;
            company_actor.ap = runtime_actor.ap.0;
            company_actor.sequence = runtime_actor.sequence;
            company_actor.pos = TileXYData {
                x: runtime_actor.position.x,
                y: runtime_actor.position.y,
            };
            company_actor.facing = runtime_actor.facing.to_index() as u8;
            company_actor.stance = format!("{:?}", runtime_actor.stance);
            company_actor.wounds = runtime_actor
                .wounds
                .iter()
                .map(ToString::to_string)
                .collect();
            let mut progression = runtime_actor.progression.clone();
            let _award =
                pb_sim::progression::award_xp(*runtime_actor_id, &mut progression, xp_award);
            company_actor.level = progression.level;
            company_actor.xp = progression.xp;
            company_actor.skill_points = progression.skill_points;
            company_actor.skill_levels = progression
                .skill_levels
                .iter()
                .map(|(skill, level)| (format!("{skill:?}"), *level))
                .collect();
            company_actor.marks = progression.marks.clone();
            company_actor.ways = progression.way.iter().cloned().collect();
            company_actor.hp_max = company_actor.attributes.hit_points(progression.level);
            company_actor.hp = company_actor.hp.min(company_actor.hp_max);
            company_actor.equipped_primary = Some(runtime_actor.weapon.clone());
            company_actor.is_dead = !runtime_actor.alive;
        }
        for actor in sim.actors.values().filter(|actor| !actor.alive) {
            if campaign
                .ledger_entries
                .iter()
                .any(|entry| entry.name == actor.name)
            {
                continue;
            }
            let pending = state
                .pending_ledger_writes
                .iter()
                .find(|entry| entry.name == actor.name);
            let chosen_line = pending
                .and_then(|entry| entry.lines.get(usize::from(entry.selected_index)))
                .map(String::as_str)
                .or_else(|| {
                    content
                        .ledger_line_sets
                        .values()
                        .find(|set| {
                            set.applies_to
                                == if actor.is_companion {
                                    "companion"
                                } else {
                                    "combatant"
                                }
                        })
                        .and_then(|set| set.lines.get(usize::from(set.default_index)))
                        .map(String::as_str)
                })
                .ok_or_else(|| {
                    format!(
                        "E-CONTENT-002: no authored Ledger line for death `{}`",
                        actor.name
                    )
                })?;
            ledger.add_entry(
                &actor.name,
                if actor.is_companion {
                    "Companion"
                } else {
                    "Combatant"
                },
                place,
                &node.date,
                chosen_line,
                "Player",
            );
            if actor.is_companion {
                if let Some(company_actor) = campaign
                    .company
                    .iter_mut()
                    .find(|company_actor| company_actor.id == actor.name)
                {
                    company_actor.is_dead = true;
                }
            }
        }
    }
    let newly_written =
        u32::try_from(ledger.entries.len().saturating_sub(previous_ledger_len)).unwrap_or(u32::MAX);
    let ledger_keeper = campaign
        .company
        .iter()
        .find(|actor| actor.id == "c_elias")
        .is_some_and(|actor| actor.marks.iter().any(|mark| mark == "mk_ledger_keeper"));
    campaign.ledger_weight = campaign
        .ledger_weight
        .saturating_add(ledger_weight_increment(newly_written, ledger_keeper));
    campaign.ledger_entries = ledger.entries.iter().map(entry_to_data).collect();
    campaign.ledger_head_hash = ledger.entries.last().map_or_else(
        || ZERO_HASH.to_string(),
        |entry| pb_content::hash::hex(&entry.hash),
    );
    let recovery = crate::camp::recover_company(campaign);
    let dead = campaign
        .company
        .iter()
        .filter(|actor| actor.is_companion && actor.is_dead)
        .map(|actor| actor.id.clone())
        .collect::<Vec<_>>();
    let advance = advance_interludes(
        &graph,
        &mut campaign.campaign_flags,
        &mut campaign.completed_nodes,
        &dead,
    )
    .map_err(|error| format!("E-CAMPAIGN-GRAPH: {error}"))?;
    campaign.sim_snapshot = None;
    campaign.written_at_tick = state.sim.as_ref().map_or(0, |sim| sim.tick.0);
    state.message = format!(
        "Camp recovery: +{} HP, +{} Sand, {} wounds treated; {} interlude(s)",
        recovery.hp_restored,
        recovery.sand_restored,
        recovery.wounds_healed,
        advance.interludes_completed.len()
    );
    state.pending_ledger_writes.clear();
    state.ledger_write_cursor = 0;
    Ok(())
}

fn ledger_from_data(entries: &[LedgerEntryData]) -> Result<LedgerChain, String> {
    let mut chain = LedgerChain::new();
    for entry in entries {
        chain.entries.push(LedgerEntry {
            index: entry.index,
            prev_hash: parse_hash(&entry.prev_hash)?,
            name: entry.name.clone(),
            role: entry.role.clone(),
            place: entry.place.clone(),
            date: entry.date.clone(),
            chosen_line: entry.chosen_line.clone(),
            written_by: entry.written_by.clone(),
            hash: parse_hash(&entry.hash)?,
        });
    }
    if !chain.verify_chain() {
        return Err("E-SAVE-TAMPERED: Ledger chain mismatch".to_string());
    }
    Ok(chain)
}

fn parse_hash(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("E-SAVE-TAMPERED: invalid Ledger hash length".to_string());
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| "E-SAVE-TAMPERED: invalid Ledger hash".to_string())?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| "E-SAVE-TAMPERED: invalid Ledger hash".to_string())?;
    }
    Ok(bytes)
}

fn entry_to_data(entry: &LedgerEntry) -> LedgerEntryData {
    LedgerEntryData {
        index: entry.index,
        prev_hash: pb_content::hash::hex(&entry.prev_hash),
        name: entry.name.clone(),
        role: entry.role.clone(),
        place: entry.place.clone(),
        date: entry.date.clone(),
        chosen_line: entry.chosen_line.clone(),
        written_by: entry.written_by.clone(),
        hash: pb_content::hash::hex(&entry.hash),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn content_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }

    #[test]
    fn new_campaign_selects_the_authored_opening() {
        let mut state = GameState::new();
        assert!(start_new(&mut state, &content_root(), 90210).is_ok());
        let Some(campaign) = state.campaign.as_ref() else {
            panic!("campaign was not created");
        };
        let company = &campaign.company;
        assert_eq!(company.len(), 1);
        assert_eq!(company[0].id, "c_elias");
        assert!(!company[0].is_companion);
        assert_eq!(
            select_next(&mut state, &content_root()),
            Ok("m01_elk_creek".to_string())
        );
        assert_eq!(state.current_mission.as_deref(), Some("m01_elk_creek"));
    }

    #[test]
    fn on_screen_campaign_path_resolves_briefing_and_starts_battle() {
        use pb_render::ui_contract::GameScreen;

        let root = content_root();
        let content = pb_content::load::load_all(&root).unwrap();
        let way_id = content.ways.keys().next().cloned().unwrap();
        let mut state = GameState::new();

        assert_eq!(state.go_to(GameScreen::NewCompany), Ok(()));
        assert!(start_new_with_way(&mut state, &root, 90210, &way_id).is_ok());
        assert_eq!(state.go_to(GameScreen::Camp), Ok(()));
        assert_eq!(state.go_to(GameScreen::MapTravel), Ok(()));
        assert_eq!(
            select_next(&mut state, &root),
            Ok("m01_elk_creek".to_string())
        );
        assert_eq!(state.go_to(GameScreen::Briefing), Ok(()));

        let scenario = scenario_for_mission(&content, "m01_elk_creek").unwrap();
        assert_eq!(scenario.id, "scn_m01_elk_creek");
        assert!(crate::combat::init_combat(&mut state, &root).is_ok());
        assert_eq!(state.go_to(GameScreen::Battle), Ok(()));
        assert!(state.sim.is_some());
    }

    #[test]
    fn selected_way_and_starting_items_persist_into_the_company() {
        let mut state = GameState::new();
        assert!(start_new_with_way(&mut state, &content_root(), 90210, "wy_old_wound").is_ok());
        let Some(campaign) = state.campaign.as_ref() else {
            panic!("campaign was not created");
        };
        assert_eq!(campaign.company[0].ways, vec!["wy_old_wound"]);
        assert!(campaign.company[0]
            .inventory
            .iter()
            .any(|stack| stack.item_id == "it_painkillers" && stack.count == 1));
    }

    #[test]
    fn victory_completes_node_and_appends_a_verified_ledger_entry() {
        let root = content_root();
        let mut state = GameState::new();
        assert!(start_new(&mut state, &root, 90210).is_ok());
        assert!(select_next(&mut state, &root).is_ok());
        let mut simulation = pb_sim::state::SimState::new(90210, 1);
        let actor_id = pb_core::ids::ActorId(77);
        let mut fallen = pb_sim::clock::build_actor(
            actor_id,
            "e_outlaw_test",
            4,
            20,
            10,
            pb_core::geom::TileXY::new(4, 4),
        );
        fallen.faction_id = "outlaw".to_string();
        fallen.alive = false;
        fallen.hit_points = 0;
        simulation.actors.insert(actor_id, fallen);
        state.sim = Some(simulation);
        state.last_victory = Some(true);

        assert!(prepare_ledger_writes(&mut state, &root).is_ok());
        assert_eq!(state.pending_ledger_writes.len(), 1);
        assert_eq!(state.pending_ledger_writes[0].lines.len(), 3);
        let selected_line = state.pending_ledger_writes[0].lines[2].clone();
        assert!(choose_ledger_line(&mut state, 2).is_ok());
        assert!(finish_battle(&mut state, &root).is_ok());
        let campaign = state.campaign.as_ref().unwrap();
        assert!(campaign
            .completed_nodes
            .iter()
            .any(|node| node == "m01_elk_creek"));
        assert!(campaign
            .company
            .iter()
            .any(|actor| actor.id == "c_naomi" && actor.is_companion));
        assert_eq!(campaign.ledger_entries.len(), 1);
        assert_eq!(campaign.ledger_entries[0].chosen_line, selected_line);
        assert_eq!(campaign.ledger_entries[0].written_by, "Player");
        assert_eq!(campaign.ledger_weight, 2);
        assert_ne!(campaign.ledger_head_hash, ZERO_HASH);
        assert!(ledger_from_data(&campaign.ledger_entries)
            .map(|ledger| ledger.verify_chain())
            .unwrap_or(false));
    }

    #[test]
    fn battle_completion_persists_xp_skill_points_and_skill_levels() {
        let root = content_root();
        let mut state = GameState::new();
        assert!(start_new(&mut state, &root, 90210).is_ok());
        assert!(select_next(&mut state, &root).is_ok());

        let mut simulation = pb_sim::state::SimState::new(90210, 1);
        let actor_id = pb_core::ids::ActorId(78);
        let mut elias = pb_sim::clock::build_actor(
            actor_id,
            "c_elias",
            5,
            40,
            20,
            pb_core::geom::TileXY::new(4, 4),
        );
        elias.faction_id = "player".to_string();
        elias.progression.xp = 345;
        elias.progression.level = 3;
        elias.progression.skill_points = 2;
        elias
            .progression
            .skill_levels
            .insert(pb_core::progression::SkillLine::FieldMedicine, 4);
        simulation.actors.insert(actor_id, elias);
        state.sim = Some(simulation);
        state.last_victory = Some(true);

        assert!(finish_battle(&mut state, &root).is_ok());
        let Some(saved) = state
            .campaign
            .as_ref()
            .and_then(|campaign| campaign.company.iter().find(|actor| actor.id == "c_elias"))
        else {
            panic!("Elias must remain in the saved company");
        };
        let expected_award = crate::camp::compute_after_action_xp(1, 0, 0, true).total_xp;
        assert_eq!(saved.xp, 345 + expected_award);
        assert_eq!(saved.level, 3);
        assert_eq!(saved.skill_points, 2);
        assert_eq!(saved.skill_levels.get("FieldMedicine"), Some(&4));
    }

    #[test]
    fn ledger_keeper_halves_weight_accrual() {
        assert_eq!(ledger_weight_increment(3, false), 6);
        assert_eq!(ledger_weight_increment(3, true), 3);
    }

    #[test]
    fn dialogue_respects_both_ledger_weight_bounds() {
        let root = content_root();
        let Ok(content) = pb_content::load::load_all(&root) else {
            panic!("content must load");
        };
        let mut state = GameState::new();
        assert!(start_new(&mut state, &root, 90210).is_ok());
        let Some(campaign) = state.campaign.as_mut() else {
            panic!("campaign must exist");
        };
        campaign
            .campaign_flags
            .push("a4_salt_war_sided_with_ruelas".to_string());
        campaign
            .completed_nodes
            .push("camp_12_last_account".to_string());

        campaign.ledger_weight = 19;
        assert!(available_camp_dialogue(&content, campaign)
            .iter()
            .all(|scene| scene.id != "dlg_act4_weight_opens"));
        campaign.ledger_weight = 20;
        assert!(available_camp_dialogue(&content, campaign)
            .iter()
            .any(|scene| scene.id == "dlg_act4_weight_opens"));

        campaign
            .campaign_flags
            .push("a1_treaty_witnessed".to_string());
        campaign.completed_nodes = vec!["camp_01_ledger_fire".to_string()];
        campaign.ledger_weight = 5;
        assert!(available_camp_dialogue(&content, campaign)
            .iter()
            .all(|scene| scene.id != "dlg_act1_ledger_begins"));
    }

    #[test]
    fn reached_act_two_choice_can_be_selected_in_the_client() {
        let root = content_root();
        let Ok(content) = pb_content::load::load_all(&root) else {
            panic!("content must load");
        };
        let graph = build_graph(&content);
        let mut state = GameState::new();
        assert!(start_new(&mut state, &root, 90210).is_ok());
        let Some(campaign) = state.campaign.as_mut() else {
            panic!("campaign must exist");
        };
        for _ in 0..100 {
            let Ok(advance) = advance_interludes(
                &graph,
                &mut campaign.campaign_flags,
                &mut campaign.completed_nodes,
                &[],
            ) else {
                panic!("campaign must advance");
            };
            if !advance.choices.is_empty() {
                break;
            }
            let Some(mission) = advance.missions.first() else {
                panic!("Act II choice must be reachable");
            };
            assert!(complete_node(
                &graph,
                mission,
                &mut campaign.campaign_flags,
                &mut campaign.completed_nodes
            )
            .is_ok());
        }

        assert_eq!(
            current_choices(&mut state, &root),
            Ok(vec![
                "a2_choice_take_hide_contract".to_string(),
                "a2_choice_warn_adobe_walls".to_string()
            ])
        );
        assert_eq!(
            choose_branch(&mut state, &root, 1),
            Ok("a2_choice_warn_adobe_walls".to_string())
        );
        let Some(campaign) = state.campaign.as_ref() else {
            panic!("campaign was not created");
        };
        assert!(campaign
            .campaign_flags
            .iter()
            .any(|flag| flag == "a2_warned_adobe_walls"));
        assert!(!campaign
            .campaign_flags
            .iter()
            .any(|flag| flag == "a2_took_hide_contract"));
    }
}
