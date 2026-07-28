//! Shared runtime UI and accessibility contract.
//!
//! The client and command-line release gate both execute these functions.
//! Keeping screen legality, palettes, copy layout, and presentation timing
//! here prevents an audit command from proving a second, fabricated model.

use std::time::Duration;

/// Current game screen. This is the single runtime UI state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameScreen {
    Title,
    NewCompany,
    Camp,
    MapTravel,
    Briefing,
    Battle,
    AfterAction,
    LedgerView,
    Settings,
    Bibliography,
    SaveSlot,
    LoadSlot,
}

impl GameScreen {
    pub const ALL: [Self; 12] = [
        Self::Title,
        Self::NewCompany,
        Self::Camp,
        Self::MapTravel,
        Self::Briefing,
        Self::Battle,
        Self::AfterAction,
        Self::LedgerView,
        Self::Settings,
        Self::Bibliography,
        Self::SaveSlot,
        Self::LoadSlot,
    ];

    /// Apply one declared screen transition.
    pub fn transition(self, target: Self) -> Result<Self, String> {
        let legal = matches!(
            (self, target),
            (
                Self::Title,
                Self::NewCompany | Self::Settings | Self::LedgerView | Self::Bibliography
            ) | (Self::NewCompany, Self::Camp | Self::Title)
                | (
                    Self::Camp,
                    Self::MapTravel | Self::LedgerView | Self::Settings | Self::Title
                )
                | (Self::MapTravel, Self::Camp | Self::Briefing)
                | (Self::Briefing, Self::MapTravel | Self::Battle)
                | (
                    Self::Battle,
                    Self::AfterAction | Self::SaveSlot | Self::LoadSlot | Self::Title
                )
                | (
                    Self::AfterAction,
                    Self::Camp | Self::LedgerView | Self::Title
                )
                | (Self::LedgerView, Self::Camp | Self::Title)
                | (Self::Settings, Self::Camp | Self::Title)
                | (Self::Bibliography, Self::Title)
                | (Self::SaveSlot, Self::Battle)
                | (Self::LoadSlot, Self::Battle)
        );
        if legal {
            Ok(target)
        } else {
            Err(format!("E-SCREEN-TRANSITION: {self:?} -> {target:?}"))
        }
    }
}

/// Execute a legal keyboard-only route that visits every shipped screen.
pub fn keyboard_traversal() -> Result<Vec<GameScreen>, String> {
    use GameScreen as S;
    let targets = [
        S::Settings,
        S::Title,
        S::LedgerView,
        S::Title,
        S::Bibliography,
        S::Title,
        S::NewCompany,
        S::Camp,
        S::LedgerView,
        S::Camp,
        S::MapTravel,
        S::Briefing,
        S::Battle,
        S::SaveSlot,
        S::Battle,
        S::LoadSlot,
        S::Battle,
        S::AfterAction,
        S::Camp,
    ];
    let mut current = S::Title;
    let mut visited = vec![current];
    for target in targets {
        current = current.transition(target)?;
        visited.push(current);
    }
    Ok(visited)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPalette {
    pub background: [f32; 4],
    pub text: [f32; 4],
    pub accent: [f32; 4],
    pub ally: [f32; 4],
    pub enemy: [f32; 4],
    pub warning: [f32; 4],
}

pub const PALETTE_NAMES: [&str; 3] = ["default", "deuteranopia", "tritanopia"];

/// Resolve the exact palette used by the shipped renderer.
pub fn palette_for_name(name: &str) -> UiPalette {
    match name {
        "deuteranopia" => UiPalette {
            background: [0.06, 0.08, 0.12, 0.88],
            text: [0.96, 0.96, 0.88, 1.0],
            accent: [0.42, 0.72, 1.0, 1.0],
            ally: [0.38, 0.78, 1.0, 1.0],
            enemy: [1.0, 0.68, 0.24, 1.0],
            warning: [1.0, 0.86, 0.32, 1.0],
        },
        "tritanopia" => UiPalette {
            background: [0.05, 0.10, 0.08, 0.88],
            text: [0.98, 0.93, 0.88, 1.0],
            accent: [1.0, 0.45, 0.42, 1.0],
            ally: [0.95, 0.65, 0.30, 1.0],
            enemy: [0.95, 0.38, 0.65, 1.0],
            warning: [1.0, 0.82, 0.38, 1.0],
        },
        _ => UiPalette {
            background: [0.08, 0.06, 0.04, 0.88],
            text: [0.96, 0.94, 0.86, 1.0],
            accent: [0.95, 0.76, 0.30, 1.0],
            ally: [0.48, 0.82, 1.0, 1.0],
            enemy: [1.0, 0.48, 0.38, 1.0],
            warning: [1.0, 0.84, 0.32, 1.0],
        },
    }
}

pub fn contrast_ratio(foreground: [f32; 4], background: [f32; 4]) -> f32 {
    let luminance = |color: [f32; 4]| {
        let channel = |value: f32| {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color[0]) + 0.7152 * channel(color[1]) + 0.0722 * channel(color[2])
    };
    let a = luminance(foreground);
    let b = luminance(background);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Slow clock changes presentation cadence only; simulation ticks are not
/// accepted by this API.
pub fn presentation_frame_interval(slow_clock: bool) -> Duration {
    Duration::from_millis(if slow_clock { 32 } else { 16 })
}

/// All flashing effects must be registered here with their maximum cadence.
#[derive(Debug, Clone, Copy)]
pub struct FlashingEffect {
    pub name: &'static str,
    pub flashes_per_second: u8,
}

/// No shipped effect flashes. Future effects fail the gate above 3 Hz.
pub const SHIPPED_FLASHING_EFFECTS: &[FlashingEffect] = &[];

pub const HUD_ACTION_SELECTED: &str = "Actions: [F]ire  [A]imed  [G]Called  [H]old  [R]eload";
pub const HUD_ACTION_TARGETING: &str = "Click an enemy to target | Right-click to cancel";
pub const HUD_INSTRUCTION_IDLE: &str = "Click an ally to select | Press key for action";
pub const HUD_INSTRUCTION_SELECTED: &str =
    "Press F: fire  A: aimed  G: called shot wheel  1-7: called shot  H: hold  R: reload";
pub const HUD_INSTRUCTION_TARGETING: &str = "Click on an enemy to execute the action";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn traversal_visits_every_screen_through_legal_edges() {
        let Ok(path) = keyboard_traversal() else {
            panic!("declared accessibility traversal must stay legal");
        };
        let visited: BTreeSet<_> = path.into_iter().collect();
        assert_eq!(visited, GameScreen::ALL.into_iter().collect());
    }

    #[test]
    fn palettes_meet_text_contrast_floor() {
        for name in PALETTE_NAMES {
            let palette = palette_for_name(name);
            assert!(contrast_ratio(palette.text, palette.background) >= 4.5);
        }
    }
}
