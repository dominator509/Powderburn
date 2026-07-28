//! Camp / after-action progression screen.
//!
//! Provides the camp menu interface where the player can:
//! - View squad XP, levels, and skill lines
//! - Spend skill points on skill lines
//! - View acquired Marks (perks)
//! - View chosen Way (trait)
//! - Select new Marks when a mark-granting level is reached
//!
//! This module produces structured data for the renderer to display;
//! it does not perform any wgpu rendering itself.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use pb_core::progression::{
    marks_earned_by_level, xp_for_level, SkillLine, BASE_SCENARIO_XP, BONUS_OBJECTIVE_XP,
    HEADSHOT_BONUS_XP, MAX_CHARACTER_LEVEL, NO_CASUALTIES_BONUS_XP, OBJECTIVE_BONUS_XP,
};

use pb_content::schema::SaveFileData;
use pb_sim::progression::{compute_skill_bonuses, ActorProgression, SkillBonuses};

/// A label and value pair for display.
#[derive(Debug, Clone)]
pub struct DisplayRow {
    pub label: String,
    pub value: String,
}

/// A skill line display entry.
#[derive(Debug, Clone)]
pub struct SkillDisplay {
    pub name: &'static str,
    pub level: u32,
    pub max_level: u32,
    pub effect: String,
    pub can_upgrade: bool,
}

/// Progression view for a single squad member.
#[derive(Debug, Clone)]
pub struct ProgressionView {
    /// Character name / ID.
    pub name: String,
    /// Current level.
    pub level: u32,
    /// Total XP earned.
    pub xp: u64,
    /// XP needed for next level (None if at max).
    pub xp_to_next: Option<u64>,
    /// Unspent skill points.
    pub skill_points: u32,
    /// Skill line details.
    pub skills: Vec<SkillDisplay>,
    /// Marks (perks) acquired.
    pub marks: Vec<String>,
    /// Way (trait) if chosen.
    pub way: Option<String>,
    /// Combat bonuses derived from skills.
    pub bonuses: SkillBonuses,
}

/// Build a progression display view from an actor's progression data.
pub fn build_progression_view(name: &str, prog: &ActorProgression) -> ProgressionView {
    let xp_to_next = if prog.level < MAX_CHARACTER_LEVEL {
        Some(xp_for_level(prog.level + 1).saturating_sub(prog.xp))
    } else {
        None
    };

    let skills: Vec<SkillDisplay> = SkillLine::ALL
        .iter()
        .map(|&skill| {
            let level = prog.skill_level(skill);
            let effect = match skill {
                SkillLine::Pistols => format!("+{} pistol accuracy", level),
                SkillLine::LongGuns => format!("+{} rifle accuracy", level),
                SkillLine::Scatterguns => format!("+{} shotgun accuracy", level),
                SkillLine::Blades => format!("+{} melee accuracy", level),
                SkillLine::Explosives => format!("+{} explosive accuracy/misfire reduction", level),
                SkillLine::FieldMedicine => format!("+{} HP on heal", level),
                SkillLine::Scouting => format!("+{} sight radius", level),
                SkillLine::Talk => format!("+{} dialogue/rally bonus", level),
            };
            SkillDisplay {
                name: skill.display_name(),
                level,
                max_level: SkillLine::MAX_LEVEL,
                effect,
                can_upgrade: prog.skill_points > 0 && level < SkillLine::MAX_LEVEL,
            }
        })
        .collect();

    let bonuses = compute_skill_bonuses(prog);

    ProgressionView {
        name: name.to_string(),
        level: prog.level,
        xp: prog.xp,
        xp_to_next,
        skill_points: prog.skill_points,
        skills,
        marks: prog.marks.clone(),
        way: prog.way.clone(),
        bonuses,
    }
}

/// Spend a skill point and return the result as a display string.
pub fn spend_skill_point(prog: &mut ActorProgression, skill: SkillLine) -> Result<String, String> {
    let skill_name = skill.display_name();
    match prog.spend_skill_point(skill) {
        Ok(new_level) => Ok(format!("{} increased to level {}!", skill_name, new_level)),
        Err(msg) => Err(format!("Cannot upgrade {}: {}", skill_name, msg)),
    }
}

/// Check whether the player has pending mark choices to make.
/// Returns the number of unassigned mark slots.
pub fn pending_mark_choices(prog: &ActorProgression) -> u32 {
    let earned = marks_earned_by_level(prog.level);
    let owned = prog.marks.len() as u32;
    earned.saturating_sub(owned)
}

/// The result of an after-action XP phase.
#[derive(Debug, Clone)]
pub struct AfterActionXpSummary {
    /// Total XP awarded.
    pub total_xp: u64,
    /// XP award breakdown.
    pub base_xp: u64,
    pub primary_objective_xp: u64,
    pub bonus_objective_xp: u64,
    pub headshot_xp: u64,
    pub no_casualties_xp: u64,
    /// Whether any squad member levelled up.
    pub any_level_up: bool,
    /// Number of squad members who gained levels.
    pub squad_members_leveled: u32,
    /// Members who need to choose marks.
    pub members_needing_marks: Vec<String>,
}

/// Persistent changes made when the company reaches a camp interlude.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CampRecoverySummary {
    pub actors_treated: u32,
    pub hp_restored: u32,
    pub sand_restored: u32,
    pub wounds_healed: u32,
}

fn sand_recovery_amount(max_sand: i32, ledger_weight: u32) -> i32 {
    if max_sand <= 0 {
        return 0;
    }
    let weight_penalty = i32::try_from(ledger_weight / 2)
        .unwrap_or(i32::MAX)
        .min(max_sand.saturating_sub(1));
    max_sand.saturating_sub(weight_penalty).max(1)
}

/// Heal the living company once after a resolved mission.
///
/// The best living SAVVY score determines ordinary treatment. Tomas provides
/// one additional wound treatment and five additional HP while alive. Ledger
/// Weight reduces Sand recovered, but can never reduce it below one point.
pub fn recover_company(campaign: &mut SaveFileData) -> CampRecoverySummary {
    let surgeon_savvy = campaign
        .company
        .iter()
        .filter(|actor| !actor.is_dead)
        .map(|actor| actor.attributes.savvy)
        .max()
        .unwrap_or(1)
        .clamp(1, 10);
    let tomas_available = campaign
        .company
        .iter()
        .any(|actor| actor.id == "c_alcantara" && !actor.is_dead);
    let wound_slots = 1usize
        .saturating_add(usize::try_from(surgeon_savvy / 4).unwrap_or(0))
        .saturating_add(usize::from(tomas_available));
    let hp_recovery = 5i32
        .saturating_add(surgeon_savvy)
        .saturating_add(if tomas_available { 5 } else { 0 });
    let mut summary = CampRecoverySummary::default();

    for actor in campaign.company.iter_mut().filter(|actor| !actor.is_dead) {
        summary.actors_treated = summary.actors_treated.saturating_add(1);

        let old_hp = actor.hp;
        actor.hp = actor.hp.saturating_add(hp_recovery).min(actor.hp_max);
        summary.hp_restored = summary
            .hp_restored
            .saturating_add(u32::try_from(actor.hp.saturating_sub(old_hp)).unwrap_or(u32::MAX));

        let old_sand = actor.sand;
        actor.sand = actor
            .sand
            .saturating_add(sand_recovery_amount(actor.sand_max, campaign.ledger_weight))
            .min(actor.sand_max);
        summary.sand_restored = summary
            .sand_restored
            .saturating_add(u32::try_from(actor.sand.saturating_sub(old_sand)).unwrap_or(u32::MAX));

        let healed = actor.wounds.len().min(wound_slots);
        actor.wounds.drain(0..healed);
        summary.wounds_healed = summary
            .wounds_healed
            .saturating_add(u32::try_from(healed).unwrap_or(u32::MAX));
        actor.ap = actor.ap_max;
    }

    summary
}

/// Calculate the after-action XP summary for the squad.
pub fn compute_after_action_xp(
    primary_objectives: u32,
    bonus_objectives: u32,
    headshots: u32,
    no_casualties: bool,
) -> AfterActionXpSummary {
    let base_xp = BASE_SCENARIO_XP;
    let primary_objective_xp = primary_objectives as u64 * OBJECTIVE_BONUS_XP;
    let bonus_objective_xp = bonus_objectives as u64 * BONUS_OBJECTIVE_XP;
    let headshot_xp = headshots as u64 * HEADSHOT_BONUS_XP;
    let no_casualties_xp = if no_casualties {
        NO_CASUALTIES_BONUS_XP
    } else {
        0
    };

    let total_xp =
        base_xp + primary_objective_xp + bonus_objective_xp + headshot_xp + no_casualties_xp;

    AfterActionXpSummary {
        total_xp,
        base_xp,
        primary_objective_xp,
        bonus_objective_xp,
        headshot_xp,
        no_casualties_xp,
        any_level_up: false,
        squad_members_leveled: 0,
        members_needing_marks: Vec::new(),
    }
}

/// Render the progression screen as a list of text rows (for CLI debug or
/// overlay display).
pub fn render_progression_text(view: &ProgressionView) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!("=== {} ===", view.name));
    lines.push(format!("Level: {}   XP: {}", view.level, view.xp));
    if let Some(xp_needed) = view.xp_to_next {
        lines.push(format!("XP to next level: {}", xp_needed));
    } else {
        lines.push("XP to next level: MAX LEVEL".to_string());
    }
    lines.push(format!("Skill points: {}", view.skill_points));

    if let Some(ref way) = view.way {
        lines.push(format!("Way: {}", way));
    }

    lines.push(String::new());
    lines.push("--- Skills ---".to_string());
    for skill in &view.skills {
        lines.push(format!(
            "  {}: {} / {} ({}) {}",
            skill.name,
            skill.level,
            skill.max_level,
            skill.effect,
            if skill.can_upgrade { "[SPEND]" } else { "" }
        ));
    }

    if !view.marks.is_empty() {
        lines.push(String::new());
        lines.push("--- Marks ---".to_string());
        for mark in &view.marks {
            lines.push(format!("  {}", mark));
        }
    }

    lines.push(String::new());
    lines.push("--- Bonuses ---".to_string());
    let b = &view.bonuses;
    if b.pistols_accuracy > 0 {
        lines.push(format!("  Pistols accuracy: +{}", b.pistols_accuracy));
    }
    if b.long_guns_accuracy > 0 {
        lines.push(format!("  Long guns accuracy: +{}", b.long_guns_accuracy));
    }
    if b.scatterguns_accuracy > 0 {
        lines.push(format!(
            "  Scatterguns accuracy: +{}",
            b.scatterguns_accuracy
        ));
    }
    if b.blades_accuracy > 0 {
        lines.push(format!("  Blades accuracy: +{}", b.blades_accuracy));
    }
    if b.explosives_accuracy > 0 {
        lines.push(format!("  Explosives accuracy: +{}", b.explosives_accuracy));
    }
    if b.medicine_heal_bonus > 0 {
        lines.push(format!("  Heal bonus: +{} HP", b.medicine_heal_bonus));
    }
    if b.scouting_sight_bonus > 0 {
        lines.push(format!("  Sight bonus: +{} tiles", b.scouting_sight_bonus));
    }
    if b.talk_bonus > 0 {
        lines.push(format!("  Talk bonus: +{}", b.talk_bonus));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use pb_sim::progression::ActorProgression;

    #[test]
    fn build_progression_view_shows_correct_data() {
        let mut prog = ActorProgression::at_level(5);
        prog.skill_points = 3;
        prog.marks.push("mk_steady_hands".to_string());
        prog.way = Some("wy_old_wound".to_string());

        let _ = prog.spend_skill_point(SkillLine::Pistols);
        let _ = prog.spend_skill_point(SkillLine::Pistols);

        let view = build_progression_view("TestGuy", &prog);
        assert_eq!(view.name, "TestGuy");
        assert_eq!(view.level, 5);
        assert_eq!(view.xp, 1000);
        assert_eq!(view.skill_points, 1);
        assert_eq!(view.marks.len(), 1);
        assert_eq!(view.way, Some("wy_old_wound".to_string()));
        assert_eq!(view.skills.len(), 8);
    }

    #[test]
    fn skill_display_can_upgrade_true() {
        let mut progression = ActorProgression::at_level(5);
        progression.skill_points = 1;
        let view = build_progression_view("X", &progression);
        assert!(view
            .skills
            .iter()
            .any(|skill| skill.name == "Pistols" && skill.can_upgrade));
    }

    #[test]
    fn skill_display_can_upgrade_false_no_points() {
        let mut progression = ActorProgression::at_level(5);
        progression.skill_points = 0;
        let view = build_progression_view("X", &progression);
        assert!(view
            .skills
            .iter()
            .any(|skill| skill.name == "Pistols" && !skill.can_upgrade));
    }

    #[test]
    fn skills_list_has_eight_entries() {
        let prog = ActorProgression::new();
        let view = build_progression_view("X", &prog);
        assert_eq!(view.skills.len(), 8);
    }

    #[test]
    fn spend_skill_via_camp_success() {
        let mut prog = ActorProgression::at_level(5);
        prog.skill_points = 2;
        let result = spend_skill_point(&mut prog, SkillLine::Pistols);
        assert!(result.is_ok());
        assert_eq!(prog.skill_level(SkillLine::Pistols), 1);
    }

    #[test]
    fn spend_skill_via_camp_no_points() {
        let mut prog = ActorProgression::new();
        let result = spend_skill_point(&mut prog, SkillLine::Pistols);
        assert!(result.is_err());
    }

    #[test]
    fn pending_mark_choices_at_level_1() {
        let prog = ActorProgression::new();
        assert_eq!(pending_mark_choices(&prog), 0);
    }

    #[test]
    fn pending_mark_choices_at_level_3_no_marks() {
        let prog = ActorProgression::at_level(3);
        assert_eq!(pending_mark_choices(&prog), 1);
    }

    #[test]
    fn pending_mark_choices_at_level_3_with_mark() {
        let mut prog = ActorProgression::at_level(3);
        prog.marks.push("mk_steady_hands".to_string());
        assert_eq!(pending_mark_choices(&prog), 0);
    }

    #[test]
    fn after_action_xp_basic() {
        let summary = compute_after_action_xp(0, 0, 0, false);
        assert_eq!(summary.total_xp, BASE_SCENARIO_XP);
    }

    #[test]
    fn after_action_xp_with_bonuses() {
        let summary = compute_after_action_xp(1, 1, 2, true);
        let expected = BASE_SCENARIO_XP
            + OBJECTIVE_BONUS_XP
            + BONUS_OBJECTIVE_XP
            + 2 * HEADSHOT_BONUS_XP
            + NO_CASUALTIES_BONUS_XP;
        assert_eq!(summary.total_xp, expected);
    }

    #[test]
    fn render_progression_text_produces_lines() {
        let prog = ActorProgression::new();
        let view = build_progression_view("Test", &prog);
        let lines = render_progression_text(&view);
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.contains("Test")));
        assert!(lines.iter().any(|l| l.contains("Level:")));
        assert!(lines.iter().any(|l| l.contains("Pistols")));
    }

    #[test]
    fn xp_to_next_level_at_max_level() {
        let mut prog = ActorProgression::at_level(MAX_CHARACTER_LEVEL);
        prog.xp = xp_for_level(MAX_CHARACTER_LEVEL);
        let view = build_progression_view("Max", &prog);
        assert!(view.xp_to_next.is_none());
    }

    #[test]
    fn ledger_weight_slows_but_never_eliminates_sand_recovery() {
        assert_eq!(sand_recovery_amount(20, 0), 20);
        assert_eq!(sand_recovery_amount(20, 20), 10);
        assert_eq!(sand_recovery_amount(20, u32::MAX), 1);
    }

    #[test]
    fn living_tomas_treats_one_additional_wound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let mut state = crate::state::GameState::new();
        assert!(crate::campaign::start_new(&mut state, &root, 90210).is_ok());
        let Some(mut with_tomas) = state.campaign else {
            panic!("new campaign must exist");
        };
        let Some(elias) = with_tomas
            .company
            .iter_mut()
            .find(|actor| actor.id == "c_elias")
        else {
            panic!("Elias must exist");
        };
        elias.wounds = vec![
            "Bleeding".to_string(),
            "Burned".to_string(),
            "Concussed".to_string(),
            "Winded".to_string(),
            "Broken".to_string(),
        ];
        let mut tomas = with_tomas.company[0].clone();
        tomas.id = "c_alcantara".to_string();
        tomas.is_companion = true;
        with_tomas.company.push(tomas);
        let mut without_tomas = with_tomas.clone();
        let Some(tomas) = without_tomas
            .company
            .iter_mut()
            .find(|actor| actor.id == "c_alcantara")
        else {
            panic!("Tomas must exist");
        };
        tomas.is_dead = true;

        let with_summary = recover_company(&mut with_tomas);
        let without_summary = recover_company(&mut without_tomas);
        assert!(with_summary.wounds_healed > without_summary.wounds_healed);
        let Some(with_elias) = with_tomas
            .company
            .iter()
            .find(|actor| actor.id == "c_elias")
        else {
            panic!("Elias must remain");
        };
        let with_remaining = with_elias.wounds.len();
        let Some(without_elias) = without_tomas
            .company
            .iter()
            .find(|actor| actor.id == "c_elias")
        else {
            panic!("Elias must remain");
        };
        let without_remaining = without_elias.wounds.len();
        assert!(with_remaining < without_remaining);
    }
}
