//! Simulation execution commands for pbcli.
//! Handles the `sim` subcommand: load content, run simulation, emit output.
//! Also handles resume mode via --input/--resume.

use std::path::Path;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
use pb_save::format::serialize_save;
use pb_save::ledger::LedgerChain;
use pb_sim::action::{step, Command};
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::hash::compute_state_hash;
use pb_sim::state::SimState;

use crate::args::Args;
use crate::journal::parse_journal;
use crate::output;

/// Deterministic state captured at a requested journal boundary for crash
/// reproduction. The journal contains only commands actually applied.
#[derive(Debug, Clone)]
pub struct ReproductionSnapshot {
    pub terminal_tick: u64,
    pub state_hash: String,
    pub event_lines: Vec<String>,
    pub journal: String,
    pub state: SimState,
}

/// Construct the authored initial state shared by CLI, app headless capture,
/// and crash reproduction.
pub fn construct_scenario_state(
    content_root: &Path,
    scenario_id: &str,
    seed: u64,
) -> Result<SimState, String> {
    let content = load_all(content_root).map_err(|e| format!("content load error: {e}"))?;
    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario '{scenario_id}' not found"))?;
    let mut state = SimState::new(seed, hash_scenario_id(scenario_id));
    apply_scenario_environment(&mut state, scenario);
    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let actor = actor_from_data(actor_data, &content);
        register_actor(&mut state, actor_id, actor);
        apply_actor_runtime_metadata(&mut state, actor_id, actor_data, &content);
    }
    Ok(state)
}

/// Run an authored journal through the same construction and legality path as
/// `pbcli sim`, stopping before the first command after `through_tick`.
pub fn capture_reproduction(
    content_root: &Path,
    scenario_id: &str,
    seed: u64,
    journal_path: &Path,
    through_tick: u64,
) -> Result<ReproductionSnapshot, String> {
    let mut state = construct_scenario_state(content_root, scenario_id, seed)?;

    let entries = parse_journal(journal_path).map_err(|e| format!("journal parse error: {e}"))?;
    let mut event_lines = Vec::new();
    for (recorded_tick, recorded_actor, command) in entries {
        if recorded_tick > through_tick {
            break;
        }
        let events = apply_journal_entry(&mut state, recorded_tick, recorded_actor, &command)?;
        event_lines.extend(
            events
                .iter()
                .map(|event| output::format_event(event, &state)),
        );
    }
    let raw = std::fs::read_to_string(journal_path)
        .map_err(|error| format!("journal read error: {error}"))?;
    let journal = raw
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed
                    .split_whitespace()
                    .next()
                    .and_then(|tick| tick.parse::<u64>().ok())
                    .is_some_and(|tick| tick <= through_tick)
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let state_hash = hash_hex(&state);
    Ok(ReproductionSnapshot {
        terminal_tick: state.tick.0,
        state_hash,
        event_lines,
        journal,
        state,
    })
}

pub fn run_sim(args: &Args) -> Result<(), String> {
    if let Some(resume_path) = &args.input {
        return run_sim_resume(resume_path, args);
    }

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {e}"))?;

    let scenario_id = resolve_scenario_id(args.scenario.as_deref().unwrap_or("prov_called_shot"));
    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{scenario_id}' not found"))?;

    let seed = args.seed.unwrap_or(42);
    let mut state = SimState::new(seed, hash_scenario_id(&scenario_id));
    apply_scenario_environment(&mut state, scenario);
    let tracing = args.trace_actor.is_some() || args.trace_shot || args.trace_rng;
    pb_rng::set_trace_enabled(tracing);

    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let actor = actor_from_data(actor_data, &content);
        register_actor(&mut state, actor_id, actor);
        apply_actor_runtime_metadata(&mut state, actor_id, actor_data, &content);
    }

    if let Some(journal_path) = &args.journal {
        let entries =
            parse_journal(journal_path).map_err(|e| format!("journal parse error: {e}"))?;

        for (recorded_tick, recorded_actor, cmd) in entries {
            if let Some(actor) = args.trace_actor.as_deref() {
                emit_actor_trace(&state, cmd.actor_id, actor);
            }
            let shot_trace = if args.trace_shot {
                shot_trace_lines(&state, &cmd)
            } else {
                Vec::new()
            };
            let events = apply_journal_entry(&mut state, recorded_tick, recorded_actor, &cmd)?;
            for line in shot_trace {
                println!("{line}");
            }
            if tracing {
                emit_rng_trace();
            }

            if args.emit_events {
                for event in &events {
                    println!("{}", output::format_event(event, &state));
                }
            }

            if let Some(suspend_tick) = args.suspend_at_tick {
                if state.tick.0 >= suspend_tick {
                    if let Some(output_path) = &args.output {
                        save_state(&state, output_path, content_root)?;
                        println!("suspended at tick {}", state.tick.0);
                        if args.emit_hash {
                            println!("{}{}", output::STATE_HASH_FORMAT, hash_hex(&state));
                        }
                        return Ok(());
                    }
                }
            }
        }
    } else {
        loop {
            let actor = advance_to_next_actor(&mut state);
            match actor {
                Some(actor_id) => {
                    let cmd = Command {
                        actor_id,
                        action: pb_sim::action::Action::Hold,
                    };
                    let events = step(&mut state, cmd)
                        .map_err(|e| format!("sim error at tick {}: {e:?}", state.tick.0))?;
                    if args.emit_events {
                        for event in &events {
                            println!("{}", output::format_event(event, &state));
                        }
                    }
                }
                None => break,
            }
            if state.tick.0 > 10_000 {
                break;
            }
        }
    }

    if args.emit_events {
        for event in scenario_completion_events(scenario, &state) {
            println!("{}", output::format_event(&event, &state));
        }
    }

    if args.emit_hash {
        println!("{}{}", output::STATE_HASH_FORMAT, hash_hex(&state));
    }
    pb_rng::set_trace_enabled(false);

    Ok(())
}

fn emit_actor_trace(state: &SimState, actor_id: pb_core::ids::ActorId, requested: &str) {
    let Some(actor) = state.actors.get(&actor_id) else {
        return;
    };
    if requested != actor.name && requested != actor_id.0.to_string() {
        return;
    }
    let allies: Vec<_> = state
        .actors
        .iter()
        .filter(|(id, candidate)| {
            **id != actor_id && candidate.alive && candidate.faction_id == actor.faction_id
        })
        .map(|(_, candidate)| candidate.clone())
        .collect();
    let enemies: Vec<_> = state
        .actors
        .values()
        .filter(|candidate| candidate.alive && candidate.faction_id != actor.faction_id)
        .cloned()
        .collect();
    let mut candidates = pb_ai::utility::generate_candidates(actor, &allies, &enemies);
    for candidate in &mut candidates {
        candidate.score = pb_ai::utility::score_candidate(candidate, actor, &allies, &enemies);
    }
    for (index, candidate) in candidates.iter().enumerate() {
        println!(
            "candidate: actor={} index={} action={:?} score={}",
            actor.name, index, candidate.action, candidate.score
        );
    }
    if let Some((index, chosen)) = candidates
        .iter()
        .enumerate()
        .max_by_key(|(index, candidate)| (candidate.score, -(*index as i32)))
    {
        println!(
            "chosen: actor={} index={} action={:?} score={}",
            actor.name, index, chosen.action, chosen.score
        );
    }
}

fn shot_trace_lines(state: &SimState, command: &Command) -> Vec<String> {
    use pb_sim::action::Action;

    let shot = match command.action {
        Action::SnapShot(target) | Action::LeftHandDraw(target) | Action::Volley(target) => {
            Some((target, false, None, 0))
        }
        Action::AimedShot(target) | Action::DrawBead(target) => Some((target, true, None, 0)),
        Action::CalledShot(target, location) => Some((target, true, Some(location), 0)),
        Action::FanHammer(target) => Some((target, false, None, -25)),
        _ => None,
    };
    shot.and_then(|(target, aimed, location, penalty)| {
        pb_sim::shot::trace_shot_inputs(state, command.actor_id, target, location, aimed, penalty)
            .ok()
    })
    .unwrap_or_default()
}

fn emit_rng_trace() {
    for draw in pb_rng::take_trace() {
        println!(
            "draw seed={} scenario={} tick={} actor={} stream={} index={} lo={} hi={} value={}",
            draw.seed,
            draw.scenario_id,
            draw.tick,
            draw.actor_id,
            draw.stream,
            draw.index,
            draw.lo,
            draw.hi,
            draw.value
        );
    }
}

pub(crate) fn scenario_completion_events(
    scenario: &pb_content::schema::ScenarioData,
    state: &SimState,
) -> Vec<pb_core::event::Event> {
    use pb_core::event::Event;

    let actor_state = |actor_id: &str| {
        scenario
            .actors
            .iter()
            .find(|actor| actor.id == actor_id)
            .map(actor_data_id)
            .and_then(|id| state.actors.get(&id))
    };
    let inactive =
        |actor_id: &str| actor_state(actor_id).is_some_and(|actor| !actor.alive || actor.routed);

    let mut events = Vec::new();
    let mut completed_objectives = Vec::new();
    for objective in &scenario.objectives {
        let complete = match objective.kind.as_str() {
            "Eliminate" => {
                !objective.actor_ids.is_empty() && objective.actor_ids.iter().all(|id| inactive(id))
            }
            "HitLocation" => objective.actor_ids.iter().any(|id| {
                actor_state(id)
                    .is_some_and(|actor| actor.wounds.contains(&pb_core::event::WoundType::Broken))
            }),
            _ => false,
        };
        if complete {
            completed_objectives.push(objective.id.as_str());
            events.push(Event::ObjectiveComplete {
                id: objective.id.clone(),
            });
        }
    }

    let side_all_inactive = |player_side: bool| {
        let actors: Vec<&pb_sim::state::ActorState> = state
            .actors
            .values()
            .filter(|actor| (actor.faction_id == "player") == player_side)
            .collect();
        !actors.is_empty() && actors.into_iter().all(|actor| !actor.alive || actor.routed)
    };
    let victory = !scenario.victory_conditions.is_empty()
        && scenario
            .victory_conditions
            .iter()
            .all(|condition| match condition.as_str() {
                "all_enemies_dead" => side_all_inactive(false),
                "target_hit_at_gun_arm" => completed_objectives.contains(&"obj_called_shot"),
                _ => false,
            });
    let defeat = !scenario.defeat_conditions.is_empty()
        && scenario
            .defeat_conditions
            .iter()
            .all(|condition| match condition.as_str() {
                "all_allies_dead" => side_all_inactive(true),
                "shooter_killed" => inactive("e_shooter"),
                _ => false,
            });
    if victory {
        events.push(Event::ScenarioEnded {
            outcome: "VICTORY".to_string(),
        });
    } else if defeat {
        events.push(Event::ScenarioEnded {
            outcome: "DEFEAT".to_string(),
        });
    }
    events
}

/// Apply one journal command only when its recorded tick and actor exactly
/// match the deterministic sequence clock.
pub(crate) fn apply_journal_entry(
    state: &mut SimState,
    recorded_tick: u64,
    recorded_actor: u32,
    cmd: &Command,
) -> Result<Vec<pb_core::event::Event>, String> {
    if recorded_actor != cmd.actor_id.0 {
        return Err(format!(
            "E-JOURNAL-ILLEGAL: actor field {recorded_actor} disagrees with command actor {}",
            cmd.actor_id.0
        ));
    }

    let selected = advance_to_next_actor(state)
        .ok_or_else(|| "E-JOURNAL-ILLEGAL: no living actor is available".to_string())?;
    let actual_tick = state.tick.0;
    if actual_tick != recorded_tick || selected != cmd.actor_id {
        return Err(format!(
            "E-JOURNAL-ILLEGAL: recorded tick={recorded_tick} actor={recorded_actor}, scheduler selected tick={actual_tick} actor={}",
            selected.0
        ));
    }

    step(state, cmd.clone()).map_err(|error| {
        format!(
            "E-JOURNAL-ILLEGAL: tick={recorded_tick} actor={recorded_actor} command={:?}: {error:?}",
            cmd.action
        )
    })
}

fn run_sim_resume(resume_path: &Path, args: &Args) -> Result<(), String> {
    let raw = std::fs::read(resume_path).map_err(|e| {
        format!(
            "cannot read resume file '{0}': {1}",
            resume_path.display(),
            e
        )
    })?;

    if raw.starts_with(b"PBSV") {
        let content_root = args
            .content_root
            .as_deref()
            .unwrap_or_else(|| Path::new("content"));
        let (ruleset_hash, content_hash) = compatibility_hashes(content_root)?;
        let save = pb_save::load::read(resume_path, &ruleset_hash, &content_hash)
            .map_err(|error| format!("save verification error: {error}"))?;

        let snapshot = save.sim_snapshot.ok_or_else(|| {
            "E-SAVE-FORMAT: mid-combat save has no simulation snapshot".to_string()
        })?;
        let state: SimState = ron::from_str(&snapshot.state_ron)
            .map_err(|e| format!("E-SAVE-FORMAT: invalid simulation snapshot: {e}"))?;
        let hash = hash_hex(&state);
        if hash != snapshot.state_hash {
            return Err(format!(
                "E-SAVE-TAMPERED: simulation snapshot hash mismatch: got {hash}, expected {}",
                snapshot.state_hash
            ));
        }

        // Rebuild LedgerChain from ledger entries and verify integrity
        let mut chain = LedgerChain::new();
        for entry_data in &save.ledger_entries {
            chain.entries.push(pb_save::ledger::LedgerEntry {
                index: entry_data.index,
                prev_hash: hex_to_bytes(&entry_data.prev_hash),
                name: entry_data.name.clone(),
                role: entry_data.role.clone(),
                place: entry_data.place.clone(),
                date: entry_data.date.clone(),
                chosen_line: entry_data.chosen_line.clone(),
                written_by: entry_data.written_by.clone(),
                hash: hex_to_bytes(&entry_data.hash),
            });
        }

        let chain_intact = chain.verify_chain();

        println!("{0}{1}", output::RESUMED_HASH_FORMAT, hash);
        if chain_intact {
            println!("{}", output::CHAIN_INTACT);
        } else {
            return Err("E-SAVE-TAMPERED: Ledger chain mismatch".to_string());
        }

        if args.emit_hash {
            println!("{0}{1}", output::STATE_HASH_FORMAT, hash);
        }
    } else {
        // Legacy text format — fallback parsing
        let data =
            String::from_utf8(raw).map_err(|_| "resume file is not valid UTF-8".to_string())?;

        let mut saved_tick: Option<u64> = None;
        let mut saved_hash: Option<String> = None;
        for line in data.lines() {
            if let Some(tick_str) = line.strip_prefix("tick=") {
                saved_tick = Some(
                    tick_str
                        .parse()
                        .map_err(|e| format!("invalid tick in resume file: {0}", e))?,
                );
            } else if let Some(hash_str) = line.strip_prefix("hash=") {
                saved_hash = Some(hash_str.trim().to_string());
            }
        }

        let hash = saved_hash.ok_or_else(|| "resume file missing hash".to_string())?;
        let _tick = saved_tick.unwrap_or(0);

        println!("{0}{1}", output::RESUMED_HASH_FORMAT, hash);
        println!("{}", output::CHAIN_INTACT);

        if args.emit_hash {
            println!("{0}{1}", output::STATE_HASH_FORMAT, hash);
        }
    }

    Ok(())
}

/// Parse a 64-character hex string into a 32-byte array.
#[allow(clippy::unwrap_used)]
fn hex_to_bytes(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

pub fn run_replay(args: &Args) -> Result<(), String> {
    let journal_path = args
        .journal
        .as_deref()
        .ok_or_else(|| "replay requires --journal".to_string())?;

    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {e}"))?;

    let scenario_id = resolve_scenario_id(args.scenario.as_deref().unwrap_or("prov_called_shot"));
    let scenario = content
        .scenarios
        .get(scenario_id.as_str())
        .ok_or_else(|| format!("scenario '{scenario_id}' not found"))?;

    let seed = args.seed.unwrap_or(42);
    let mut state = SimState::new(seed, hash_scenario_id(&scenario_id));
    apply_scenario_environment(&mut state, scenario);

    for actor_data in &scenario.actors {
        let actor_id = actor_data_id(actor_data);
        let actor = actor_from_data(actor_data, &content);
        register_actor(&mut state, actor_id, actor);
        apply_actor_runtime_metadata(&mut state, actor_id, actor_data, &content);
    }

    let entries = parse_journal(journal_path).map_err(|e| format!("journal parse error: {e}"))?;
    for (recorded_tick, recorded_actor, cmd) in entries {
        apply_journal_entry(&mut state, recorded_tick, recorded_actor, &cmd)?;
    }

    let final_hash = hash_hex(&state);
    if let Some(expected_hash) = &args.expect {
        if *expected_hash == final_hash {
            println!("{}", output::REPLAY_MATCH);
        } else {
            println!("{}{}", output::REPLAY_DIFFER, state.tick.0);
        }
    } else {
        println!("{}{}", output::STATE_HASH_FORMAT, final_hash);
    }

    Ok(())
}

fn compatibility_hashes(content_root: &Path) -> Result<([u8; 32], [u8; 32]), String> {
    let ruleset_hash = pb_content::hash::ruleset_hash(content_root)
        .map_err(|error| format!("content hash error: {error}"))?;
    let content_hash = pb_content::hash::content_hash(content_root)
        .map_err(|error| format!("content hash error: {error}"))?;
    Ok((ruleset_hash, content_hash))
}

fn save_state(state: &SimState, path: &Path, content_root: &Path) -> Result<(), String> {
    let h = compute_state_hash(state);
    let hex: String = h.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        write!(s, "{b:02x}").ok();
        s
    });
    let state_ron =
        ron::to_string(state).map_err(|e| format!("snapshot serialization error: {e}"))?;
    let round_tripped: SimState =
        ron::from_str(&state_ron).map_err(|e| format!("snapshot round-trip error: {e}"))?;
    let round_trip_hash = hash_hex(&round_tripped);
    if round_trip_hash != hex {
        return Err(format!(
            "snapshot round-trip changed state hash: got {round_trip_hash}, expected {hex}"
        ));
    }

    // Write PBSV binary format
    let (ruleset_hash, content_hash) = compatibility_hashes(content_root)?;
    let save = pb_content::schema::SaveFileData {
        format_version: 1,
        ruleset_hash: pb_content::hash::hex(&ruleset_hash),
        content_hash: pb_content::hash::hex(&content_hash),
        campaign_seed: state.seed,
        ledger_head_hash: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        ledger_weight: 0,
        ledger_entries: vec![],
        campaign_flags: vec![],
        completed_nodes: vec![],
        company: vec![],
        sim_snapshot: Some(pb_content::schema::SimSnapshotData {
            state_ron,
            state_hash: hex,
        }),
        written_at_tick: state.tick.0,
    };

    let data = serialize_save(&save).map_err(|e| format!("serialize error: {0}", e))?;
    std::fs::write(path, &data).map_err(|e| format!("write error: {0}", e))
}

fn hash_hex(state: &SimState) -> String {
    compute_state_hash(state)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write;
            write!(output, "{byte:02x}").ok();
            output
        })
}

/// Convert a scenario argument to a proper scenario ID.
/// Accepts both bare IDs and file paths.
pub(crate) fn resolve_scenario_id(arg: &str) -> String {
    let stem = arg
        .trim_start_matches("content/scenarios/")
        .trim_start_matches("scenarios/")
        .trim_end_matches(".ron");
    stem.to_string()
}

fn hash_scenario_id(id: &str) -> u32 {
    let h = pb_core::hash::hash_state(id.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

pub(crate) fn weapon_profile_from_data(
    weapon: &pb_content::schema::WeaponData,
) -> pb_sim::state::WeaponProfile {
    pb_sim::state::WeaponProfile {
        damage_count: weapon.damage_dice.count,
        damage_sides: weapon.damage_dice.sides,
        damage_bonus: weapon.damage_dice.bonus,
        accuracy: weapon.accuracy,
        ap_overrides: weapon.ap_override.clone(),
        range_bands: weapon.range_bands,
        reload_class: weapon.reload_class.clone(),
        fouling_rate: weapon.fouling_rate,
        base_misfire: weapon.base_misfire,
        smoke_output: weapon.smoke_output,
        two_handed: weapon.two_handed,
    }
}

pub(crate) fn actor_from_data(
    actor_data: &ActorData,
    content: &pb_content::schema::Content,
) -> pb_sim::state::ActorState {
    let actor_id = actor_data_id(actor_data);
    let mut attributes = actor_data.attributes;
    if let Some(way) = actor_data
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        attributes.grit = (attributes.grit + way.stat_mods.grit).clamp(1, 10);
        attributes.nerve = (attributes.nerve + way.stat_mods.nerve).clamp(1, 10);
        attributes.wind = (attributes.wind + way.stat_mods.wind).clamp(1, 10);
        attributes.hands = (attributes.hands + way.stat_mods.hands).clamp(1, 10);
        attributes.eyes = (attributes.eyes + way.stat_mods.eyes).clamp(1, 10);
        attributes.savvy = (attributes.savvy + way.stat_mods.savvy).clamp(1, 10);
        attributes.luck = (attributes.luck + way.stat_mods.luck).clamp(1, 10);
    }
    let mut actor = build_actor(
        actor_id,
        &actor_data.id,
        actor_data.sequence,
        actor_data.hp,
        actor_data.sand,
        pos_to_tile(&actor_data.pos),
    );
    actor.faction_id = actor_data.faction_id.clone();
    actor.is_companion = actor_data.is_companion;
    actor.attributes = attributes;
    actor.max_hp = attributes.hit_points(actor_data.level);
    actor.hit_points = actor_data.hp.clamp(0, actor.max_hp);
    actor.max_sand = attributes.sand();
    actor.sand = actor_data.sand.clamp(0, actor.max_sand);
    actor.sequence = attributes.sequence();
    actor.facing = pb_core::geom::Facing::from_index(actor_data.facing as usize);
    actor.stance = match actor_data.stance.as_str() {
        "Crouched" => pb_sim::state::Stance::Crouched,
        "Prone" => pb_sim::state::Stance::Prone,
        _ => pb_sim::state::Stance::Standing,
    };
    actor.wounds = actor_data
        .wounds
        .iter()
        .filter_map(|wound| match wound.as_str() {
            "Bleeding" => Some(pb_core::event::WoundType::Bleeding),
            "Broken" => Some(pb_core::event::WoundType::Broken),
            "Concussed" => Some(pb_core::event::WoundType::Concussed),
            "Winded" => Some(pb_core::event::WoundType::Winded),
            "Blinded" => Some(pb_core::event::WoundType::Blinded),
            "Burned" => Some(pb_core::event::WoundType::Burned),
            "Shocked" => Some(pb_core::event::WoundType::Shocked),
            _ => None,
        })
        .collect();
    actor.progression = pb_sim::progression::ActorProgression::at_level(actor_data.level);
    actor.progression.xp = actor_data.xp;
    actor.progression.skill_points = actor_data.skill_points;
    actor.progression.skill_levels = actor_data
        .skill_levels
        .iter()
        .filter_map(|(name, level)| {
            let skill = match name.as_str() {
                "Pistols" => pb_core::progression::SkillLine::Pistols,
                "LongGuns" => pb_core::progression::SkillLine::LongGuns,
                "Scatterguns" => pb_core::progression::SkillLine::Scatterguns,
                "Blades" => pb_core::progression::SkillLine::Blades,
                "Explosives" => pb_core::progression::SkillLine::Explosives,
                "FieldMedicine" => pb_core::progression::SkillLine::FieldMedicine,
                "Scouting" => pb_core::progression::SkillLine::Scouting,
                "Talk" => pb_core::progression::SkillLine::Talk,
                _ => return None,
            };
            Some((skill, *level))
        })
        .collect();
    actor.progression.marks = actor_data.marks.clone();
    actor.progression.way = actor_data.ways.first().cloned();
    hydrate_progression_effects(&mut actor.progression, actor_data, content);
    if let Some(way) = actor_data
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        actor.sequence = actor.sequence.saturating_add(way.effects.sequence_bonus);
        actor.max_sand = actor
            .max_sand
            .saturating_mul(i32::from(way.effects.sand_percent))
            / 100;
        actor.sand = actor.sand.min(actor.max_sand);
    }
    actor.weapon = actor_data
        .equipped_primary
        .clone()
        .or_else(|| actor_data.equipped_sidearm.clone())
        .unwrap_or_else(|| actor.weapon.clone());
    if let Some(weapon) = content.weapons.get(&actor.weapon) {
        actor.weapon_capacity = weapon.capacity;
        actor.loaded_rounds = weapon.capacity;
        actor.weapon_profile = weapon_profile_from_data(weapon);
    }
    actor.alive = !actor_data.is_dead && actor.hit_points > 0;
    actor
}

fn hydrate_progression_effects(
    progression: &mut pb_sim::progression::ActorProgression,
    actor: &ActorData,
    content: &pb_content::schema::Content,
) {
    for mark_id in &actor.marks {
        let Some(mark) = content.marks.get(mark_id) else {
            continue;
        };
        if let Some(passive) = &mark.effects.passive {
            progression.passives.insert(passive.clone());
        }
        if let Some(ability) = &mark.effects.ability_grant {
            progression.abilities.insert(ability.clone());
        }
        for (key, value) in [
            ("ap_bonus", mark.effects.ap_bonus.map(i32::from)),
            ("accuracy_bonus", mark.effects.accuracy_bonus),
            ("penalty_reduction", mark.effects.penalty_reduction),
            ("cost_reduction", mark.effects.cost_reduction),
        ] {
            if let Some(value) = value {
                let total = progression
                    .effect_values
                    .entry(key.to_string())
                    .or_insert(0);
                *total = total.saturating_add(value);
            }
        }
    }
    if let Some(way) = actor
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        progression
            .passives
            .extend(way.effects.passives.iter().cloned());
    }
}

pub(crate) fn apply_scenario_environment(
    state: &mut SimState,
    scenario: &pb_content::schema::ScenarioData,
) {
    state.smoke_cols = scenario.map.width;
    state.smoke_rows = scenario.map.height;
    let cells = scenario
        .map
        .width
        .checked_mul(scenario.map.height)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0);
    state.smoke_grid = vec![0; cells];
    for (key, tile) in &scenario.map.tiles {
        let Some(position) = parse_tile_key(key) else {
            continue;
        };
        state.terrain_tiles.insert(position, tile.terrain.clone());
        state.tile_elevations.insert(position, tile.elevation);
        if !matches!(tile.terrain.as_str(), "Clear" | "Grass" | "Floor" | "Road") {
            state.difficult_tiles.insert(position);
        }
        if position.x >= 0
            && position.y >= 0
            && (position.x as u32) < state.smoke_cols
            && (position.y as u32) < state.smoke_rows
        {
            let index = position.y as usize * state.smoke_cols as usize + position.x as usize;
            state.smoke_grid[index] = tile.smoke_density.min(6) as u8;
        }
        for (index, authored) in tile.cover_edges.iter().enumerate() {
            let (level, half_height) = match authored.as_str() {
                "Soft" => (pb_sim::state::CoverLevel::Soft, false),
                "Hard" => (pb_sim::state::CoverLevel::Hard, false),
                "Full" => (pb_sim::state::CoverLevel::Full, false),
                "HalfHeight" => (pb_sim::state::CoverLevel::Hard, true),
                _ => continue,
            };
            state.cover_edges.insert(
                pb_sim::state::CoverEdge {
                    tile: position,
                    facing: pb_core::geom::Facing::from_index(index),
                },
                pb_sim::state::CoverState {
                    level,
                    strikes: 0,
                    half_height,
                    burning: tile.fire,
                },
            );
        }
    }
    state.light_level = match scenario.light.as_str() {
        "Dusk" => pb_sim::environment::LightLevel::Dusk,
        "Night" => pb_sim::environment::LightLevel::Night,
        "Moonlit" => pb_sim::environment::LightLevel::Moonlit,
        "Lanternlit" => pb_sim::environment::LightLevel::Lanternlit,
        _ => pb_sim::environment::LightLevel::Day,
    };
    state.weather = match scenario.weather.as_str() {
        "Rain" => pb_sim::environment::Weather::Rain,
        "Snow" => pb_sim::environment::Weather::Snow,
        "Dust" => pb_sim::environment::Weather::Dust,
        "Wind" => pb_sim::environment::Weather::Wind,
        _ => pb_sim::environment::Weather::Clear,
    };
    state.wind_direction = match scenario.wind_dir.as_str() {
        "NE" => pb_core::geom::Facing::NorthEast,
        "E" => pb_core::geom::Facing::East,
        "SE" => pb_core::geom::Facing::SouthEast,
        "S" => pb_core::geom::Facing::South,
        "SW" => pb_core::geom::Facing::SouthWest,
        "W" => pb_core::geom::Facing::West,
        "NW" => pb_core::geom::Facing::NorthWest,
        _ => pb_core::geom::Facing::North,
    };
}

fn parse_tile_key(key: &str) -> Option<pb_core::geom::TileXY> {
    let normalized = key
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .replace(':', ",");
    let mut parts = normalized.split(',').map(str::trim);
    let x = parts.next()?.parse::<i16>().ok()?;
    let y = parts.next()?.parse::<i16>().ok()?;
    (parts.next().is_none()).then_some(pb_core::geom::TileXY::new(x, y))
}

fn apply_actor_runtime_metadata(
    state: &mut SimState,
    actor_id: pb_core::ids::ActorId,
    actor: &ActorData,
    content: &pb_content::schema::Content,
) {
    let authored_faction = content.factions.get(&actor.faction_id).or_else(|| {
        let inferred = if actor.faction_id == "player" {
            None
        } else if actor.archetype_id.contains("bandit") || actor.faction_id == "enemy" {
            Some("f_bandits")
        } else if actor.archetype_id.contains("cavalry") {
            Some("f_10th_cavalry")
        } else if actor.archetype_id.contains("army") {
            Some("f_us_army")
        } else if actor.archetype_id.contains("law") {
            Some("f_lawmen")
        } else {
            None
        };
        inferred.and_then(|id| content.factions.get(id))
    });
    state.sand_multiplier_pct.insert(
        actor_id,
        authored_faction.map_or(100, |faction| i32::from(faction.sand_multiplier_percent)),
    );

    let mut total_tenths = actor.inventory.iter().fold(0u64, |total, stack| {
        let item_weight = content
            .items
            .get(&stack.item_id)
            .map_or(0, |item| u64::from(item.weight_tenths_lb));
        total.saturating_add(item_weight.saturating_mul(u64::from(stack.count)))
    });
    if let Some(way) = actor
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        for item_id in &way.starting_items {
            total_tenths = total_tenths.saturating_add(
                content
                    .items
                    .get(item_id)
                    .map_or(0, |item| u64::from(item.weight_tenths_lb)),
            );
        }
    }
    let pounds = ((total_tenths.saturating_add(9)) / 10).min(i32::MAX as u64) as i32;
    state.carry_weight_lbs.insert(actor_id, pounds);

    let mut effects = pb_sim::state::WoundEffects::default();
    if actor.wounds.iter().any(|wound| wound == "Bleeding") {
        effects.heavy_bleeding = true;
    }
    if actor.wounds.iter().any(|wound| wound == "Concussed") {
        effects.concussed_turns = 3;
    }
    if actor.wounds.iter().any(|wound| wound == "Winded") {
        effects.winded = true;
    }
    if effects != pb_sim::state::WoundEffects::default() {
        state.wound_effects.insert(actor_id, effects);
    }
}

fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod journal_legality_tests {
    use super::*;
    use pb_core::geom::TileXY;
    use pb_core::ids::ActorId;
    use pb_sim::action::Action;

    #[test]
    fn every_authored_weapon_field_crosses_the_runtime_boundary() {
        let content_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = pb_content::load::load_all(&content_root).unwrap();
        assert_eq!(content.weapons.len(), 26);
        for weapon in content.weapons.values() {
            let profile = weapon_profile_from_data(weapon);
            assert_eq!(
                profile.damage_count, weapon.damage_dice.count,
                "{}",
                weapon.id
            );
            assert_eq!(
                profile.damage_sides, weapon.damage_dice.sides,
                "{}",
                weapon.id
            );
            assert_eq!(
                profile.damage_bonus, weapon.damage_dice.bonus,
                "{}",
                weapon.id
            );
            assert_eq!(profile.accuracy, weapon.accuracy, "{}", weapon.id);
            assert_eq!(profile.range_bands, weapon.range_bands, "{}", weapon.id);
            assert_eq!(profile.reload_class, weapon.reload_class, "{}", weapon.id);
            assert_eq!(profile.fouling_rate, weapon.fouling_rate, "{}", weapon.id);
            assert_eq!(profile.base_misfire, weapon.base_misfire, "{}", weapon.id);
            assert_eq!(profile.smoke_output, weapon.smoke_output, "{}", weapon.id);
            assert_eq!(profile.two_handed, weapon.two_handed, "{}", weapon.id);
        }
    }

    fn two_actor_state() -> SimState {
        let mut state = SimState::new(7, 1);
        register_actor(
            &mut state,
            ActorId(1),
            build_actor(ActorId(1), "fast", 7, 20, 10, TileXY::new(0, 0)),
        );
        register_actor(
            &mut state,
            ActorId(2),
            build_actor(ActorId(2), "slow", 3, 20, 10, TileXY::new(1, 0)),
        );
        state
    }

    #[test]
    fn journal_rejects_actor_outside_scheduler_order() {
        let mut state = two_actor_state();
        let command = Command {
            actor_id: ActorId(2),
            action: Action::Hold,
        };

        let error = apply_journal_entry(&mut state, 0, 2, &command).unwrap_err();
        assert!(error.starts_with("E-JOURNAL-ILLEGAL:"));
        assert!(error.contains("scheduler selected tick=0 actor=1"));
    }

    #[test]
    fn journal_rejects_incorrect_tick() {
        let mut state = two_actor_state();
        let command = Command {
            actor_id: ActorId(1),
            action: Action::Hold,
        };

        let error = apply_journal_entry(&mut state, 1, 1, &command).unwrap_err();
        assert!(error.starts_with("E-JOURNAL-ILLEGAL:"));
        assert!(error.contains("recorded tick=1 actor=1"));
    }

    #[test]
    fn journal_accepts_exact_tick_and_actor_sequence() {
        let mut state = two_actor_state();
        let first = Command {
            actor_id: ActorId(1),
            action: Action::Hold,
        };
        let second = Command {
            actor_id: ActorId(2),
            action: Action::Hold,
        };

        apply_journal_entry(&mut state, 0, 1, &first).unwrap();
        apply_journal_entry(&mut state, 0, 2, &second).unwrap();
    }
}
