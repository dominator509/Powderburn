//! Game state machine for POWDERBURN combat.
//!
//! Manages the interaction flow: IDLE → SELECTED_ACTOR → TARGETING → EXECUTING

use pb_core::event::HitLocationType;
use pb_core::ids::ActorId;
pub use pb_render::ui_contract::GameScreen;
use pb_sim::state::SimState;

/// What the player is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionPhase {
    Idle,
    SelectedActor(ActorId),
    Targeting {
        actor: ActorId,
        action: PlayerAction,
    },
    Executing,
}

/// What the player wants the selected actor to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    SnapShot,
    AimedShot,
    CalledShot(HitLocationType),
    #[allow(dead_code)]
    Move,
    Reload,
    Hold,
    Crouch,
    Prone,
    DrawBead,
    FanHammer,
    Volley,
    LeftHandDraw,
    Melee,
    Bandage,
    Rally,
    Loot,
    ThrowDynamite,
    CatchDynamite,
    RethrowDynamite,
    UseItem,
    CapAndBallReload,
    ClearJam,
}

/// A single called shot location entry for the wheel overlay.
#[derive(Debug, Clone, Copy)]
pub struct CalledShotEntry {
    pub location: HitLocationType,
    pub key: u8,
    pub penalty: i32,
    pub crit_effect: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLedgerWrite {
    pub name: String,
    pub role: String,
    pub place: String,
    pub date: String,
    pub lines: [String; 3],
    pub selected_index: u8,
}

/// Top-level game state.
#[derive(Debug)]
pub struct GameState {
    pub screen: GameScreen,
    pub sim: Option<SimState>,
    /// Full campaign persistence model shared with the CLI and save verifier.
    pub campaign: Option<pb_content::schema::SaveFileData>,
    /// Integrity state of the currently loaded Ledger.
    pub ledger_verification: pb_save::load::LedgerVerification,
    /// Slot that failed strict verification and may be retried only by an
    /// explicit player command.
    pub pending_unverified_slot: Option<String>,
    pub settings: crate::settings::Settings,
    pub input_bindings: crate::input::InputBindings,
    pub remap_pending: Option<crate::input::Action>,
    /// Authored Way selected on the NewCompany screen.
    pub new_company_way: Option<String>,
    /// Reached authored choice nodes shown on the map screen.
    pub map_choices: Vec<String>,
    /// Campaign graph node selected for the active briefing/battle.
    pub current_mission: Option<String>,
    /// Outcome of the battle currently shown on the after-action screen.
    pub last_victory: Option<bool>,
    pub phase: InteractionPhase,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_down: bool,
    pub hovered_tile_x: i16,
    pub hovered_tile_y: i16,
    pub camera_x: f32,
    pub camera_y: f32,
    /// Three-step tactical camera zoom: 1.0, 1.35, or 1.7.
    pub camera_zoom: f32,
    pub tick: u64,
    pub message: String,
    /// Whether the game is paused (pause menu overlay shown).
    pub paused: bool,
    /// Whether the local-only observability metric panel is visible.
    pub debug_metrics_visible: bool,
    /// Audio system for sound effect playback.
    pub audio: Option<pb_audio::AudioSystem>,
    /// Whether the called shot wheel overlay is shown.
    pub called_shot_active: bool,
    /// Currently selected called shot location index (0-6) when wheel is active.
    pub called_shot_index: u8,
    /// The called shot entries for the wheel display.
    pub called_shot_entries: Vec<CalledShotEntry>,
    /// Complete battle event stream used by the after-action report.
    pub battle_events: Vec<pb_core::event::Event>,
    /// Authored Ledger choices awaiting player acknowledgement.
    pub pending_ledger_writes: Vec<PendingLedgerWrite>,
    pub ledger_write_cursor: usize,
    pub ledger_filter_act: Option<u8>,
    pub ledger_allies_only: bool,
    pub ledger_scroll: usize,
}

impl GameState {
    /// Apply a declared screen transition and update runtime state only on success.
    pub fn go_to(&mut self, target: GameScreen) -> Result<(), String> {
        match self.screen.transition(target) {
            Ok(next) => {
                self.screen = next;
                Ok(())
            }
            Err(error) => {
                debug_assert!(false, "{error}");
                Err(error)
            }
        }
    }

    pub fn new() -> Self {
        Self {
            screen: GameScreen::Title,
            sim: None,
            campaign: None,
            ledger_verification: pb_save::load::LedgerVerification::Verified,
            pending_unverified_slot: None,
            settings: crate::settings::Settings::default(),
            input_bindings: crate::input::InputBindings::default(),
            remap_pending: None,
            new_company_way: None,
            map_choices: Vec::new(),
            current_mission: None,
            last_victory: None,
            phase: InteractionPhase::Idle,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            hovered_tile_x: 0,
            hovered_tile_y: 0,
            camera_x: 0.0,
            camera_y: 0.0,
            camera_zoom: 1.35,
            tick: 0,
            message: String::new(),
            paused: false,
            debug_metrics_visible: false,
            audio: None,
            called_shot_active: false,
            called_shot_index: 0,
            called_shot_entries: vec![
                CalledShotEntry {
                    location: HitLocationType::Head,
                    key: 1,
                    penalty: 25,
                    crit_effect: "Concuss",
                },
                CalledShotEntry {
                    location: HitLocationType::Eyes,
                    key: 2,
                    penalty: 40,
                    crit_effect: "Blind",
                },
                CalledShotEntry {
                    location: HitLocationType::Torso,
                    key: 3,
                    penalty: 0,
                    crit_effect: "Bleed",
                },
                CalledShotEntry {
                    location: HitLocationType::Vitals,
                    key: 4,
                    penalty: 30,
                    crit_effect: "Bleed×2.5",
                },
                CalledShotEntry {
                    location: HitLocationType::GunArm,
                    key: 5,
                    penalty: 15,
                    crit_effect: "Broken",
                },
                CalledShotEntry {
                    location: HitLocationType::OffArm,
                    key: 6,
                    penalty: 18,
                    crit_effect: "Broken",
                },
                CalledShotEntry {
                    location: HitLocationType::Legs,
                    key: 7,
                    penalty: 10,
                    crit_effect: "Broken",
                },
            ],
            battle_events: Vec::new(),
            pending_ledger_writes: Vec::new(),
            ledger_write_cursor: 0,
            ledger_filter_act: None,
            ledger_allies_only: false,
            ledger_scroll: 0,
        }
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod screen_tests {
    use super::*;

    #[test]
    fn declared_campaign_path_reaches_battle_and_returns_to_camp() {
        let path = [
            GameScreen::NewCompany,
            GameScreen::Camp,
            GameScreen::MapTravel,
            GameScreen::Briefing,
            GameScreen::Battle,
            GameScreen::AfterAction,
            GameScreen::Camp,
        ];
        let mut screen = GameScreen::Title;
        for target in path {
            let transition = screen.transition(target);
            assert_eq!(transition, Ok(target));
            screen = target;
        }
    }

    #[test]
    fn every_required_auxiliary_screen_has_a_legal_entry_and_exit() {
        assert_eq!(
            GameScreen::Title.transition(GameScreen::Settings),
            Ok(GameScreen::Settings)
        );
        assert_eq!(
            GameScreen::Settings.transition(GameScreen::Title),
            Ok(GameScreen::Title)
        );
        assert_eq!(
            GameScreen::Title.transition(GameScreen::Bibliography),
            Ok(GameScreen::Bibliography)
        );
        assert_eq!(
            GameScreen::Bibliography.transition(GameScreen::Title),
            Ok(GameScreen::Title)
        );
        assert_eq!(
            GameScreen::Camp.transition(GameScreen::LedgerView),
            Ok(GameScreen::LedgerView)
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "E-SCREEN-TRANSITION")]
    fn undeclared_transition_panics_in_debug() {
        let mut state = GameState::new();
        let _ = state.go_to(GameScreen::Battle);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn undeclared_transition_is_refused_without_mutation_in_release() {
        let mut state = GameState::new();
        let result = state.go_to(GameScreen::Battle);
        assert!(matches!(result, Err(error) if error.starts_with("E-SCREEN-TRANSITION:")));
        assert_eq!(state.screen, GameScreen::Title);
    }
}
