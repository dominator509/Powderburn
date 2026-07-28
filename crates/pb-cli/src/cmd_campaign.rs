//! Campaign commands for pbcli: new, play, audit.
//!
//! Supports --save (alias for --campaign), --mission (scenario override),
//! --emit-outcome, --emit-manifest, --from-new, --script, --dangling-refs,
//! and --choice.

use std::path::Path;

use pb_content::campaign::{
    advance_interludes, build_graph, check_permadeath_propagation, complete_node,
    compute_reachable, next_missions_with_deaths,
};
use pb_content::load::load_all;
use pb_save::ledger::LedgerChain;
use pb_save::write;
use pb_sim::clock::register_actor;
use pb_sim::state::SimState;

use crate::args::Args;
use crate::cmd_sim::{
    actor_from_data, apply_journal_entry, apply_scenario_environment, scenario_completion_events,
};
use crate::journal::{extract_choice_from_script, parse_journal};
use crate::output;

/// Run the `campaign new` subcommand.
///
/// Supports --campaign/--save <path> and --seed/--company-seed <n>.
pub fn run_campaign_new(args: &Args) -> Result<(), String> {
    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign new requires --campaign <path>".to_string())?;

    let seed = args.seed.unwrap_or(42);

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let (ruleset_hash, content_hash) = compatibility_hashes(content_root)?;
    let save = pb_content::schema::SaveFileData {
        format_version: 1,
        ruleset_hash: pb_content::hash::hex(&ruleset_hash),
        content_hash: pb_content::hash::hex(&content_hash),
        campaign_seed: seed,
        ledger_head_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        ledger_weight: 0,
        ledger_entries: vec![],
        campaign_flags: vec![],
        completed_nodes: vec![],
        company: initial_company(&content),
        sim_snapshot: None,
        written_at_tick: 0,
    };

    write::write(campaign_path, &save).map_err(|e| format!("campaign write error: {}", e))?;

    println!("{}", output::CAMPAIGN_CREATED);
    Ok(())
}

/// Run the `campaign play` subcommand.
///
/// Supports:
/// - --mission <id>: scenario override from campaign node
/// - --emit-outcome: prints "outcome: VICTORY" or "outcome: DEFEAT"
/// - --emit-manifest: prints "available: <mission_id>" for each available mission
/// - --from-new: creates a new campaign before playing
/// - --script <path>: loads journal from script file (same as --journal)
/// - --choice <value>: records a branch choice in campaign_flags
pub fn run_campaign_play(args: &Args) -> Result<(), String> {
    // --from-new: create campaign before playing
    if args.from_new {
        run_campaign_new(args)?;
    }

    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign play requires --campaign <path>".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let (ruleset_hash, content_hash) = compatibility_hashes(content_root)?;
    let mut save = pb_save::load::read(campaign_path, &ruleset_hash, &content_hash)
        .map_err(|error| format!("campaign verification error: {error}"))?;
    let journal_path = args.script_path.as_deref().or(args.journal.as_deref());
    let choice_value = args
        .choice
        .clone()
        .or_else(|| journal_path.and_then(|path| extract_choice_from_script(path).ok().flatten()));

    // Find available missions
    let graph = build_graph(&content);
    let dead_before = dead_companion_ids(&save);
    let mut advance = advance_interludes(
        &graph,
        &mut save.campaign_flags,
        &mut save.completed_nodes,
        &dead_before,
    )?;
    sync_recruits(&mut save, &content)?;
    if !advance.choices.is_empty() {
        apply_choice_alias(
            choice_value.as_deref(),
            &graph,
            &mut save.campaign_flags,
            &mut save.completed_nodes,
        )?;
        advance = advance_interludes(
            &graph,
            &mut save.campaign_flags,
            &mut save.completed_nodes,
            &dead_before,
        )?;
        sync_recruits(&mut save, &content)?;
    }
    let available = advance.missions;

    // A manifest-only choice command is a campaign navigation operation, not
    // an implicit zero-command battle. Persist the validated choice and report
    // the newly available mission without constructing combat state.
    if args.emit_manifest && journal_path.is_none() && !args.autoplay {
        write::write(campaign_path, &save)
            .map_err(|e| format!("campaign save write error: {e}"))?;
        for mission_id in &available {
            println!("{}{}", output::AVAILABLE_PREFIX, mission_id);
        }
        return Ok(());
    }

    if available.is_empty() {
        write::write(campaign_path, &save)
            .map_err(|e| format!("campaign save write error: {}", e))?;
        if args.emit_outcome {
            println!("{}", output::OUTCOME_VICTORY);
        }
        return Ok(());
    }

    // Determine which mission to play: --mission override, or first available
    let mission_id = if let Some(custom_mission) = &args.scenario {
        if !available.contains(custom_mission) {
            return Err(format!(
                "E-CONTENT-001: mission `{custom_mission}` is not currently available"
            ));
        }
        custom_mission.as_str()
    } else {
        available[0].as_str()
    };

    // Find the scenario for this mission
    let scenario_id = content
        .campaign_nodes
        .get(mission_id)
        .and_then(|n| n.scenario_id.as_ref())
        .ok_or_else(|| format!("no scenario for mission {}", mission_id))?;

    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let seed = save.campaign_seed;
    let mut state = SimState::new(seed, hash_string(scenario_id));
    apply_scenario_environment(&mut state, scenario);

    let has_living_company = save.company.iter().any(|actor| !actor.is_dead);
    // Persistent company actors replace authored player-side placeholders,
    // matching the graphical client.
    for actor_data in scenario.actors.iter().filter(|actor| {
        !has_living_company || !matches!(actor.faction_id.as_str(), "player" | "ally")
    }) {
        let actor_id = actor_data_id(actor_data);
        let actor = actor_from_data(actor_data, &content);
        register_actor(&mut state, actor_id, actor);
    }

    // Deploy only the actually recruited company, using distinct authored
    // slots. Content-roster entries that have not joined the save may not
    // appear in combat.
    for (deployment_index, saved_actor) in save
        .company
        .iter()
        .filter(|actor| !actor.is_dead)
        .enumerate()
    {
        let mut actor_data = saved_actor.clone();
        let deployment_index = if args
            .required_casualty
            .as_deref()
            .is_some_and(|casualty| casualty == saved_actor.id)
        {
            scenario
                .deployment_zones
                .get("ally")
                .map_or(deployment_index, |positions| {
                    positions.len().saturating_sub(1)
                })
        } else {
            deployment_index
        };
        if let Some(position) = scenario
            .deployment_zones
            .get("ally")
            .and_then(|positions| positions.get(deployment_index))
        {
            actor_data.pos = position.clone();
        }
        let actor_id = actor_data_id(&actor_data);
        if !state.actors.contains_key(&actor_id) {
            let actor = actor_from_data(&actor_data, &content);
            register_actor(&mut state, actor_id, actor);
        }
    }

    // If journal provided, apply commands with dead-actor-aware consumption
    if let Some(jrnl_path) = journal_path {
        let entries =
            parse_journal(jrnl_path).map_err(|e| format!("journal parse error: {0}", e))?;

        for (recorded_tick, recorded_actor, command) in entries {
            let events = apply_journal_entry(&mut state, recorded_tick, recorded_actor, &command)?;
            // Print events when emitting output (needed for CompanionKilled in LF-05)
            if args.emit_outcome || args.emit_events {
                for ev in &events {
                    println!("{}", crate::output::format_event(ev, &state));
                }
            }
        }
    }
    if args.autoplay {
        autoplay_mission(
            &mut state,
            scenario,
            args.required_casualty.as_deref(),
            args.emit_outcome || args.emit_events,
        )?;
    } else if args.required_casualty.is_some() {
        return Err("E-CLI-001: --required-casualty requires --autoplay".to_string());
    }

    // Every named death, including bodies already present when the company
    // arrives, crosses the same authored Ledger boundary as the graphical
    // client. Companion deaths additionally persist into the company roster.
    let mut dead_actors: Vec<(String, bool)> = state
        .actors
        .values()
        .filter(|actor| !actor.alive && !actor.name.trim().is_empty())
        .map(|actor| (actor.name.clone(), actor.is_companion))
        .collect();
    dead_actors.sort_by(|left, right| left.0.cmp(&right.0));
    dead_actors.dedup_by(|left, right| left.0 == right.0);

    let mut ledger_events = Vec::new();
    let mut chain = LedgerChain::new();
    for entry in &save.ledger_entries {
        chain.add_entry(
            &entry.name,
            &entry.role,
            &entry.place,
            &entry.date,
            &entry.chosen_line,
            &entry.written_by,
        );
    }
    let mut newly_written = 0_u32;
    for (actor_name, is_companion) in &dead_actors {
        if *is_companion {
            if let Some(saved_actor) = save
                .company
                .iter_mut()
                .find(|actor| actor.id == *actor_name)
            {
                saved_actor.is_dead = true;
            }
        }
        if save
            .ledger_entries
            .iter()
            .any(|entry| entry.name == *actor_name)
        {
            continue;
        }
        let category = if *is_companion {
            "companion"
        } else {
            "combatant"
        };
        let line_set = content
            .ledger_line_sets
            .values()
            .find(|set| set.applies_to == category)
            .ok_or_else(|| format!("E-CONTENT-002: no Ledger line set for `{category}`"))?;
        let chosen_line = line_set
            .lines
            .get(usize::from(line_set.default_index))
            .ok_or_else(|| {
                format!("E-CONTENT-002: invalid default Ledger line for `{category}`")
            })?;
        chain.add_entry(
            actor_name,
            if *is_companion {
                "Companion"
            } else {
                "Combatant"
            },
            &scenario.display_name,
            &scenario.date,
            chosen_line,
            "Player",
        );
        newly_written = newly_written.saturating_add(1);
        if let Some(entry) = chain.entries.last() {
            ledger_events.push(pb_core::event::Event::LedgerEntryWritten { index: entry.index });
        }
    }
    if newly_written > 0 {
        save.ledger_weight = save
            .ledger_weight
            .saturating_add(newly_written.saturating_mul(2));
        save.ledger_entries = chain
            .entries
            .iter()
            .map(|entry| pb_content::schema::LedgerEntryData {
                index: entry.index,
                prev_hash: pb_content::hash::hex(&entry.prev_hash),
                name: entry.name.clone(),
                role: entry.role.clone(),
                place: entry.place.clone(),
                date: entry.date.clone(),
                chosen_line: entry.chosen_line.clone(),
                written_by: entry.written_by.clone(),
                hash: pb_content::hash::hex(&entry.hash),
            })
            .collect();
        save.ledger_head_hash = pb_content::hash::hex(&chain.head_hash());
    }

    let completion_events = scenario_completion_events(scenario, &state);
    let victory = completion_events.iter().any(|event| {
        matches!(
            event,
            pb_core::event::Event::ScenarioEnded { outcome } if outcome == "VICTORY"
        )
    });
    if victory {
        complete_node(
            &graph,
            mission_id,
            &mut save.campaign_flags,
            &mut save.completed_nodes,
        )?;
    }

    // Tooling accepts two stable choice aliases, never arbitrary flag names.
    apply_choice_alias(
        choice_value.as_deref(),
        &graph,
        &mut save.campaign_flags,
        &mut save.completed_nodes,
    )?;
    if victory {
        let dead_after = dead_companion_ids(&save);
        let _advance = advance_interludes(
            &graph,
            &mut save.campaign_flags,
            &mut save.completed_nodes,
            &dead_after,
        )?;
        sync_recruits(&mut save, &content)?;
    }

    if args.emit_outcome || args.emit_events {
        for event in completion_events.into_iter().chain(ledger_events) {
            println!("{}", crate::output::format_event(&event, &state));
        }
    }

    // Write the updated save file
    write::write(campaign_path, &save).map_err(|e| format!("campaign save write error: {}", e))?;

    println!("{0}{1}", output::LEDGER_ENTRIES, save.ledger_entries.len());

    // --emit-outcome: determine victory or defeat based on actual state
    if args.emit_outcome {
        if victory {
            println!("{0}", output::OUTCOME_VICTORY);
        } else {
            println!("{0}", output::OUTCOME_DEFEAT);
        }
    }

    // --emit-manifest: re-read the save and emit available missions after all processing
    if args.emit_manifest {
        let save2 = pb_save::load::read(campaign_path, &ruleset_hash, &content_hash)
            .map_err(|error| format!("campaign verification error: {error}"))?;

        let dead_after = dead_companion_ids(&save2);
        let available2 = next_missions_with_deaths(
            &graph,
            &save2.campaign_flags,
            &save2.completed_nodes,
            &dead_after,
        );
        for mission_id in &available2 {
            println!("{}{}", output::AVAILABLE_PREFIX, mission_id);
        }
    }

    Ok(())
}

/// Run the `campaign audit` subcommand.
///
/// Supports --dangling-refs: checks all references in save against known entities.
/// Prints "dangling-refs: N" and "ledger-entry: <id> present" lines.
pub fn run_campaign_audit(args: &Args) -> Result<(), String> {
    let campaign_path = args
        .campaign_path
        .as_deref()
        .ok_or_else(|| "campaign audit requires --campaign <path>".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let (ruleset_hash, content_hash) = compatibility_hashes(content_root)?;
    let save = pb_save::load::read(campaign_path, &ruleset_hash, &content_hash)
        .map_err(|error| format!("campaign verification error: {error}"))?;

    // Build ledger chain
    let mut chain = LedgerChain::new();
    for entry in &save.ledger_entries {
        chain.add_entry(
            &entry.name,
            &entry.role,
            &entry.place,
            &entry.date,
            &entry.chosen_line,
            &entry.written_by,
        );
    }

    // Check chain integrity
    if chain.verify_chain() {
        println!("{}", output::CHAIN_INTACT);
    }

    let entry_count = chain.entries.len();
    println!("{}{}", output::LEDGER_ENTRIES, entry_count);

    // --dangling-refs: print ledger-entry lines for each entry
    if args.dangling_refs {
        for entry in &save.ledger_entries {
            println!("{}{} present", output::LEDGER_ENTRY_PREFIX, entry.name);
        }

        // Check for dangling references in content graph
        let graph = build_graph(&content);
        let dead = dead_companion_ids(&save);
        let reachable = compute_reachable(&graph, &save.campaign_flags, &dead);
        let mut dangling = find_dangling_refs(&content, &reachable);
        dangling.extend(check_permadeath_propagation(
            &graph,
            &content,
            &dead,
            &save.campaign_flags,
        ));

        if dangling.is_empty() {
            println!("{}{}", output::DANGLING_REFS, 0);
        } else {
            println!("{}{}", output::DANGLING_REFS, dangling.len());
            for d in &dangling {
                eprintln!("dangling: {}", d);
            }
        }
    }

    Ok(())
}

/// Find dangling references: objects that are reachable but missing content.
fn find_dangling_refs(content: &pb_content::schema::Content, reachable: &[String]) -> Vec<String> {
    let mut issues = Vec::new();
    for node_id in reachable {
        if let Some(node) = content.campaign_nodes.get(node_id) {
            if let Some(ref sc_id) = node.scenario_id {
                if !content.scenarios.contains_key(sc_id) {
                    issues.push(format!(
                        "node '{}' references missing scenario '{}'",
                        node_id, sc_id
                    ));
                }
            }
        }
    }
    issues
}

fn dead_companion_ids(save: &pb_content::schema::SaveFileData) -> Vec<String> {
    let mut dead: Vec<String> = save
        .company
        .iter()
        .filter(|actor| actor.is_companion && actor.is_dead)
        .map(|actor| actor.id.clone())
        .collect();
    dead.sort();
    dead.dedup();
    dead
}

fn autoplay_mission(
    state: &mut SimState,
    scenario: &pb_content::schema::ScenarioData,
    required_casualty: Option<&str>,
    emit_events: bool,
) -> Result<(), String> {
    if let Some(casualty) = required_casualty {
        let deployed = state
            .actors
            .values()
            .any(|actor| actor.name == casualty && actor.alive && actor.is_companion);
        if !deployed {
            return Err(format!(
                "E-CONTENT-001: required casualty `{casualty}` is not a living recruited companion"
            ));
        }
    }

    for _ in 0..8_192 {
        let completion = scenario_completion_events(scenario, state);
        if completion.iter().any(|event| {
            matches!(
                event,
                pb_core::event::Event::ScenarioEnded { outcome } if outcome == "VICTORY"
            )
        }) {
            if let Some(casualty) = required_casualty {
                let died = state
                    .actors
                    .values()
                    .any(|actor| actor.name == casualty && !actor.alive);
                if !died {
                    return Err(format!(
                        "E-CAMPAIGN-PROOF: mission ended before required casualty `{casualty}` died"
                    ));
                }
            }
            return Ok(());
        }
        if completion.iter().any(|event| {
            matches!(
                event,
                pb_core::event::Event::ScenarioEnded { outcome } if outcome == "DEFEAT"
            )
        }) {
            return Err("E-CAMPAIGN-PROOF: autoplay reached DEFEAT".to_string());
        }

        let actor_id = pb_sim::clock::advance_to_next_actor(state)
            .ok_or_else(|| "E-CAMPAIGN-PROOF: scheduler has no living actor".to_string())?;
        let actor = state
            .actors
            .get(&actor_id)
            .cloned()
            .ok_or_else(|| format!("E-CAMPAIGN-PROOF: actor {} is missing", actor_id.0))?;
        let player_side = actor.faction_id == "player";
        let casualty_target = required_casualty.and_then(|name| {
            state
                .actors
                .iter()
                .find(|(_, candidate)| candidate.name == name && candidate.alive)
                .map(|(id, _)| *id)
        });
        let enemy_target = state
            .actors
            .iter()
            .find(|(_, candidate)| {
                candidate.alive && !candidate.routed && candidate.faction_id != "player"
            })
            .map(|(id, _)| *id);

        let desired_target = if casualty_target.is_some() && !player_side {
            casualty_target
        } else if casualty_target.is_none() && player_side {
            enemy_target
        } else {
            None
        };
        let action = if let Some(casualty) = casualty_target {
            if player_side {
                pb_sim::action::Action::Hold
            } else {
                state
                    .actors
                    .get(&casualty)
                    .map_or(pb_sim::action::Action::Hold, |target| {
                        pb_sim::action::Action::ThrowDynamite(target.position)
                    })
            }
        } else if player_side {
            enemy_target.map_or(pb_sim::action::Action::Hold, |target| {
                combat_action(&actor, target)
            })
        } else {
            pb_sim::action::Action::Hold
        };
        let events = match pb_sim::action::step(state, pb_sim::action::Command { actor_id, action })
        {
            Ok(events) => events,
            Err(pb_sim::state::SimError::MustRetreat(_)) => {
                let retreat = retreat_action(state, actor_id)
                    .map(pb_sim::action::Action::Move)
                    .unwrap_or(pb_sim::action::Action::Hold);
                pb_sim::action::step(
                    state,
                    pb_sim::action::Command {
                        actor_id,
                        action: retreat,
                    },
                )
                .or_else(|error| {
                    if matches!(
                        error,
                        pb_sim::state::SimError::MustRetreat(_)
                            | pb_sim::state::SimError::TileOccupied(_)
                    ) {
                        pb_sim::action::step(
                            state,
                            pb_sim::action::Command {
                                actor_id,
                                action: pb_sim::action::Action::Hold,
                            },
                        )
                    } else {
                        Err(error)
                    }
                })
                .map_err(|error| {
                    format!(
                        "E-CAMPAIGN-PROOF: {} could not retreat: {error:?}",
                        actor.name
                    )
                })?
            }
            Err(_) => {
                let fallback = desired_target
                    .and_then(|target| approach_action(state, actor_id, target))
                    .unwrap_or(pb_sim::action::Action::Hold);
                pb_sim::action::step(
                    state,
                    pb_sim::action::Command {
                        actor_id,
                        action: fallback,
                    },
                )
                .or_else(|_| {
                    pb_sim::action::step(
                        state,
                        pb_sim::action::Command {
                            actor_id,
                            action: pb_sim::action::Action::Hold,
                        },
                    )
                })
                .map_err(|error| {
                    format!(
                        "E-CAMPAIGN-PROOF: actor {} could not act, approach, or Hold: {error:?}",
                        actor.name
                    )
                })?
            }
        };
        if emit_events {
            for event in &events {
                println!("{}", crate::output::format_event(event, state));
            }
        }
    }
    let actors = state
        .actors
        .values()
        .map(|actor| {
            format!(
                "{}:alive={} routed={} hp={} sand={} pos={}",
                actor.name, actor.alive, actor.routed, actor.hit_points, actor.sand, actor.position
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(format!(
        "E-CAMPAIGN-PROOF: autoplay exceeded 8192 commands; {actors}"
    ))
}

fn approach_action(
    state: &SimState,
    actor_id: pb_core::ids::ActorId,
    target_id: pb_core::ids::ActorId,
) -> Option<pb_sim::action::Action> {
    let actor = state.actors.get(&actor_id)?;
    let target = state.actors.get(&target_id)?;
    let step_x = (target.position.x - actor.position.x).signum();
    let step_y = (target.position.y - actor.position.y).signum();
    let distance = actor.position.chebyshev_distance(target.position);
    let stride = distance.min(2);
    let destination = pb_core::geom::TileXY::new(
        actor
            .position
            .x
            .saturating_add(step_x.saturating_mul(stride)),
        actor
            .position
            .y
            .saturating_add(step_y.saturating_mul(stride)),
    );
    if distance > 1 {
        Some(pb_sim::action::Action::Sprint(destination))
    } else {
        Some(pb_sim::action::Action::Move(destination))
    }
}

fn retreat_action(
    state: &SimState,
    actor_id: pb_core::ids::ActorId,
) -> Option<pb_core::geom::TileXY> {
    pb_sim::action::choose_retreat_tile(state, actor_id)
}

fn combat_action(
    actor: &pb_sim::state::ActorState,
    target: pb_core::ids::ActorId,
) -> pb_sim::action::Action {
    if actor.jammed {
        pb_sim::action::Action::ClearJam
    } else if actor.loaded_rounds == 0 {
        pb_sim::action::Action::Reload
    } else {
        pb_sim::action::Action::SnapShot(target)
    }
}

fn initial_company(content: &pb_content::schema::Content) -> Vec<pb_content::schema::ActorData> {
    let Some(elias) = content.companions.get("c_elias") else {
        return Vec::new();
    };
    vec![company_actor_from_companion(elias, false)]
}

fn sync_recruits(
    save: &mut pb_content::schema::SaveFileData,
    content: &pb_content::schema::Content,
) -> Result<(), String> {
    let mut recruits = save
        .completed_nodes
        .iter()
        .filter_map(|node_id| content.campaign_nodes.get(node_id))
        .flat_map(|node| node.recruits.iter().cloned())
        .collect::<Vec<_>>();
    recruits.sort();
    recruits.dedup();
    for recruit_id in recruits {
        if save.company.iter().any(|actor| actor.id == recruit_id) {
            continue;
        }
        let companion = content
            .companions
            .get(&recruit_id)
            .ok_or_else(|| format!("E-CONTENT-001: unknown authored recruit `{recruit_id}`"))?;
        save.company
            .push(company_actor_from_companion(companion, true));
    }
    save.company.sort_by(|left, right| {
        left.is_companion
            .cmp(&right.is_companion)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(())
}

fn company_actor_from_companion(
    companion: &pb_content::schema::CompanionData,
    is_companion: bool,
) -> pb_content::schema::ActorData {
    let attributes = companion.attributes;
    let level = 1;
    let hp_max = attributes.hit_points(level);
    let sand_max = attributes.sand();
    let ap_max = attributes.action_points().clamp(0, i32::from(i16::MAX)) as i16;
    pb_content::schema::ActorData {
        id: companion.id.clone(),
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
        pos: pb_content::schema::TileXYData { x: 0, y: 0 },
        facing: 2,
        stance: "Standing".to_string(),
        wounds: Vec::new(),
        inventory: Vec::new(),
        equipped_primary: companion.starting_weapons.first().cloned(),
        equipped_sidearm: None,
        marks: Vec::new(),
        ways: Vec::new(),
        is_companion,
        is_dead: false,
    }
}

fn apply_choice_alias(
    choice: Option<&str>,
    graph: &pb_content::campaign::CampaignGraph,
    flags: &mut Vec<String>,
    completed: &mut Vec<String>,
) -> Result<(), String> {
    let Some(choice) = choice else {
        return Ok(());
    };
    let (node_id, canonical, mutually_exclusive) = match choice {
        "warn_adobe_walls" => (
            "a2_choice_warn_adobe_walls",
            "a2_warned_adobe_walls",
            "a2_took_hide_contract",
        ),
        "take_hide_contract" => (
            "a2_choice_take_hide_contract",
            "a2_took_hide_contract",
            "a2_warned_adobe_walls",
        ),
        other => {
            return Err(format!("E-CONTENT-001: unknown campaign choice `{other}`"));
        }
    };
    if flags.iter().any(|flag| flag == mutually_exclusive) {
        return Err(format!(
            "E-CONTENT-001: campaign choice conflicts with `{mutually_exclusive}`"
        ));
    }
    if flags.iter().any(|flag| flag == canonical) {
        return Ok(());
    }

    let unlocked = graph.nodes.iter().any(|(predecessor_id, node)| {
        completed.iter().any(|done| done == predecessor_id)
            && node.unlocks.iter().any(|id| id == node_id)
    });
    if !unlocked {
        return Err(format!(
            "E-CONTENT-001: campaign choice `{choice}` is not reachable"
        ));
    }

    complete_node(graph, node_id, flags, completed).map(|_| ())
}

/// Hash a string to u32.
fn hash_string(s: &str) -> u32 {
    let h = pb_core::hash::hash_state(s.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn compatibility_hashes(content_root: &Path) -> Result<([u8; 32], [u8; 32]), String> {
    let ruleset_hash = pb_content::hash::ruleset_hash(content_root)
        .map_err(|error| format!("content hash error: {error}"))?;
    let content_hash = pb_content::hash::content_hash(content_root)
        .map_err(|error| format!("content hash error: {error}"))?;
    Ok((ruleset_hash, content_hash))
}

/// Convert actor data to numeric ID.
fn actor_data_id(actor: &pb_content::schema::ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod direct_flow_tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "powderburn-campaign-{label}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("create scratch");
            Self { root }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn content_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }

    fn parse(arguments: &[String]) -> Args {
        let mut raw = vec![OsString::from("pbcli")];
        raw.extend(arguments.iter().map(OsString::from));
        Args::parse(raw).expect("parse arguments")
    }

    #[test]
    fn new_autoplay_and_audit_execute_the_real_campaign_contract() {
        let scratch = Scratch::new("autoplay");
        let save = scratch.root.join("company.pbsave");
        let content = content_root();
        let common = vec![
            "--save".to_string(),
            save.display().to_string(),
            "--content-root".to_string(),
            content.display().to_string(),
        ];

        let mut new_args = vec!["campaign".to_string(), "new".to_string()];
        new_args.extend(common.clone());
        new_args.extend(["--company-seed".to_string(), "90210".to_string()]);
        run_campaign_new(&parse(&new_args)).expect("campaign new");

        let mut play_args = vec!["campaign".to_string(), "play".to_string()];
        play_args.extend(common.clone());
        play_args.extend([
            "--mission".to_string(),
            "m01_elk_creek".to_string(),
            "--autoplay".to_string(),
            "--emit-outcome".to_string(),
        ]);
        run_campaign_play(&parse(&play_args)).expect("autoplay opening");

        let mut audit_args = vec!["campaign".to_string(), "audit".to_string()];
        audit_args.extend(common);
        audit_args.push("--dangling-refs".to_string());
        run_campaign_audit(&parse(&audit_args)).expect("campaign audit");
        assert!(save.is_file());
    }

    #[test]
    fn manifest_and_illegal_mission_paths_are_explicit() {
        let scratch = Scratch::new("manifest");
        let save = scratch.root.join("company.pbsave");
        let content = content_root();
        let base = vec![
            "campaign".to_string(),
            "play".to_string(),
            "--save".to_string(),
            save.display().to_string(),
            "--content-root".to_string(),
            content.display().to_string(),
            "--company-seed".to_string(),
            "7".to_string(),
            "--from-new".to_string(),
        ];
        let mut manifest = base.clone();
        manifest.push("--emit-manifest".to_string());
        run_campaign_play(&parse(&manifest)).expect("new campaign manifest");

        let mut illegal = base;
        illegal.extend([
            "--mission".to_string(),
            "m24_ledger_reckoning".to_string(),
            "--autoplay".to_string(),
        ]);
        let error = run_campaign_play(&parse(&illegal)).expect_err("locked mission must fail");
        assert!(error.contains("E-CONTENT-001"));
    }
}
