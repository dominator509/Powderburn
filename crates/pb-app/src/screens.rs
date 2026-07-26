//! Screen state machine for the powderburn game.
//!
//! Explicit state machine with named transitions.
//! An undeclared transition is a compile error.

#![allow(dead_code, clippy::unwrap_used)]

/// The screen the player sees. Every transition is a named variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    NewCompany,
    Camp,
    MapTravel,
    Briefing,
    Battle,
    AfterAction,
    LedgerView,
    Settings,
    Quit,
}

impl Screen {
    /// Transition to a new screen. Returns Ok if the transition is legal.
    pub fn transition(self, target: Screen) -> Result<Screen, String> {
        match (self, target) {
            // From Title
            (Screen::Title, Screen::NewCompany) => Ok(target),
            (Screen::Title, Screen::Settings) => Ok(target),
            (Screen::Title, Screen::LedgerView) => Ok(target),
            (Screen::Title, Screen::Quit) => Ok(target),

            // From NewCompany
            (Screen::NewCompany, Screen::Camp) => Ok(target),

            // From Camp
            (Screen::Camp, Screen::MapTravel) => Ok(target),
            (Screen::Camp, Screen::LedgerView) => Ok(target),
            (Screen::Camp, Screen::Settings) => Ok(target),
            (Screen::Camp, Screen::Title) => Ok(target),

            // From MapTravel
            (Screen::MapTravel, Screen::Camp) => Ok(target),
            (Screen::MapTravel, Screen::Briefing) => Ok(target),

            // From Briefing
            (Screen::Briefing, Screen::Battle) => Ok(target),
            (Screen::Briefing, Screen::MapTravel) => Ok(target),

            // From Battle
            (Screen::Battle, Screen::AfterAction) => Ok(target),

            // From AfterAction
            (Screen::AfterAction, Screen::Camp) => Ok(target),
            (Screen::AfterAction, Screen::LedgerView) => Ok(target),
            (Screen::AfterAction, Screen::Title) => Ok(target),

            // From LedgerView
            (Screen::LedgerView, Screen::Camp) => Ok(target),
            (Screen::LedgerView, Screen::Title) => Ok(target),

            // From Settings
            (Screen::Settings, Screen::Title) => Ok(target),
            (Screen::Settings, Screen::Camp) => Ok(target),

            // Quit is final
            (Screen::Quit, _) => Err("cannot transition from Quit".to_string()),

            _ => Err(format!("illegal transition: {:?} -> {:?}", self, target)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_to_new_company() {
        assert_eq!(
            Screen::Title.transition(Screen::NewCompany),
            Ok(Screen::NewCompany)
        );
    }

    #[test]
    fn test_title_to_quit() {
        assert_eq!(Screen::Title.transition(Screen::Quit), Ok(Screen::Quit));
    }

    #[test]
    fn test_illegal_transition() {
        assert!(Screen::Title.transition(Screen::Battle).is_err());
    }

    #[test]
    fn test_full_path() {
        let mut s = Screen::Title;
        s = s.transition(Screen::NewCompany).unwrap();
        s = s.transition(Screen::Camp).unwrap();
        s = s.transition(Screen::MapTravel).unwrap();
        s = s.transition(Screen::Briefing).unwrap();
        s = s.transition(Screen::Battle).unwrap();
        s = s.transition(Screen::AfterAction).unwrap();
        s = s.transition(Screen::Camp).unwrap();
        assert_eq!(s, Screen::Camp);
    }

    #[test]
    fn test_quit_is_final() {
        assert!(Screen::Quit.transition(Screen::Title).is_err());
    }
}
