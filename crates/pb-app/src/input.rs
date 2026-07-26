//! Input mapping system.
//!
//! Keyboard and mouse bindings, stored in a remappable map.
//! Persisted to $PB_CONFIG_DIR/input.ron.

use std::collections::BTreeMap;

/// A key binding action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    RotateLeft,
    RotateRight,
    SelectNext,
    SelectPrev,
    EndTurn,
    Fire,
    CalledShot,
    Reload,
    Crouch,
    Prone,
    DrawBead,
    TabTarget,
    Menu,
    Confirm,
    Cancel,
}

/// Default keybindings.
pub fn default_bindings() -> BTreeMap<String, Action> {
    let mut map = BTreeMap::new();
    map.insert("w".to_string(), Action::MoveUp);
    map.insert("s".to_string(), Action::MoveDown);
    map.insert("a".to_string(), Action::MoveLeft);
    map.insert("d".to_string(), Action::MoveRight);
    map.insert("q".to_string(), Action::RotateLeft);
    map.insert("e".to_string(), Action::RotateRight);
    map.insert("Tab".to_string(), Action::TabTarget);
    map.insert("Space".to_string(), Action::EndTurn);
    map.insert("f".to_string(), Action::Fire);
    map.insert("g".to_string(), Action::CalledShot);
    map.insert("r".to_string(), Action::Reload);
    map.insert("c".to_string(), Action::Crouch);
    map.insert("x".to_string(), Action::Prone);
    map.insert("o".to_string(), Action::DrawBead);
    map.insert("Escape".to_string(), Action::Menu);
    map.insert("Return".to_string(), Action::Confirm);
    map
}
