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
        }
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}
