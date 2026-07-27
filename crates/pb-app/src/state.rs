//! Game state machine for POWDERBURN combat.
//!
//! Manages the interaction flow: IDLE → SELECTED_ACTOR → TARGETING → EXECUTING

use pb_core::event::HitLocationType;
use pb_core::ids::ActorId;
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
}

/// Current game screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameScreen {
    Title,
    Combat,
    AfterAction,
    /// Save slot selection screen.
    SaveSlot,
    /// Load slot selection screen.
    LoadSlot,
}

/// A single called shot location entry for the wheel overlay.
#[derive(Debug, Clone, Copy)]
pub struct CalledShotEntry {
    pub location: HitLocationType,
    pub key: u8,
    pub penalty: i32,
    pub crit_effect: &'static str,
}

/// Top-level game state.
#[derive(Debug)]
pub struct GameState {
    pub screen: GameScreen,
    pub sim: Option<SimState>,
    pub phase: InteractionPhase,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_down: bool,
    pub hovered_tile_x: i16,
    pub hovered_tile_y: i16,
    pub camera_x: f32,
    pub camera_y: f32,
    pub tick: u64,
    pub message: String,
    /// Whether the game is paused (pause menu overlay shown).
    pub paused: bool,
    /// Audio system for sound effect playback.
    pub audio: Option<pb_audio::AudioSystem>,
    /// Whether the called shot wheel overlay is shown.
    pub called_shot_active: bool,
    /// Currently selected called shot location index (0-6) when wheel is active.
    pub called_shot_index: u8,
    /// The called shot entries for the wheel display.
    pub called_shot_entries: Vec<CalledShotEntry>,
    /// Hovered tile for movement preview (ap_cost).
    pub move_preview_ap: Option<i32>,
    /// Whether movement preview info is visible.
    pub move_preview_active: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            screen: GameScreen::Title,
            sim: None,
            phase: InteractionPhase::Idle,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            hovered_tile_x: 0,
            hovered_tile_y: 0,
            camera_x: 0.0,
            camera_y: 0.0,
            tick: 0,
            message: String::new(),
            paused: false,
            audio: None,
            called_shot_active: false,
            called_shot_index: 0,
            called_shot_entries: vec![
                CalledShotEntry { location: HitLocationType::Head, key: 1, penalty: 25, crit_effect: "Concuss" },
                CalledShotEntry { location: HitLocationType::Eyes, key: 2, penalty: 40, crit_effect: "Blind" },
                CalledShotEntry { location: HitLocationType::Torso, key: 3, penalty: 0, crit_effect: "Bleed" },
                CalledShotEntry { location: HitLocationType::Vitals, key: 4, penalty: 30, crit_effect: "Bleed×2.5" },
                CalledShotEntry { location: HitLocationType::GunArm, key: 5, penalty: 15, crit_effect: "Broken" },
                CalledShotEntry { location: HitLocationType::OffArm, key: 6, penalty: 18, crit_effect: "Broken" },
                CalledShotEntry { location: HitLocationType::Legs, key: 7, penalty: 10, crit_effect: "Broken" },
            ],
            move_preview_ap: None,
            move_preview_active: false,
        }
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}
