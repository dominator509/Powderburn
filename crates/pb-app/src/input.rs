//! Input mapping system.
//!
//! Keyboard and mouse bindings, stored in a remappable map.
//! Persisted to $PB_CONFIG_DIR/input.ron.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::settings::MAX_SETTINGS_BYTES;

/// A key binding action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    Aim,
    CalledShot,
    Reload,
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
    map.insert("ArrowUp".to_string(), Action::MoveUp);
    map.insert("ArrowDown".to_string(), Action::MoveDown);
    map.insert("ArrowLeft".to_string(), Action::MoveLeft);
    map.insert("ArrowRight".to_string(), Action::MoveRight);
    map.insert("q".to_string(), Action::RotateLeft);
    map.insert("e".to_string(), Action::RotateRight);
    map.insert("Tab".to_string(), Action::TabTarget);
    map.insert("Space".to_string(), Action::EndTurn);
    map.insert("f".to_string(), Action::Fire);
    map.insert("h".to_string(), Action::Aim);
    map.insert("g".to_string(), Action::CalledShot);
    map.insert("r".to_string(), Action::Reload);
    map.insert("c".to_string(), Action::Crouch);
    map.insert("x".to_string(), Action::Prone);
    map.insert("o".to_string(), Action::DrawBead);
    map.insert("v".to_string(), Action::FanHammer);
    map.insert("y".to_string(), Action::Volley);
    map.insert("z".to_string(), Action::LeftHandDraw);
    map.insert("m".to_string(), Action::Melee);
    map.insert("b".to_string(), Action::Bandage);
    map.insert("t".to_string(), Action::Rally);
    map.insert("l".to_string(), Action::Loot);
    map.insert("i".to_string(), Action::ThrowDynamite);
    map.insert("j".to_string(), Action::CatchDynamite);
    map.insert("k".to_string(), Action::RethrowDynamite);
    map.insert("u".to_string(), Action::UseItem);
    map.insert("n".to_string(), Action::CapAndBallReload);
    map.insert("p".to_string(), Action::ClearJam);
    map.insert("Escape".to_string(), Action::Menu);
    map.insert("Return".to_string(), Action::Confirm);
    map.insert("Backspace".to_string(), Action::Cancel);
    map
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBindings {
    pub bindings: BTreeMap<String, Action>,
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            bindings: default_bindings(),
        }
    }
}

impl InputBindings {
    pub fn action_for(&self, key: &str) -> Option<Action> {
        self.bindings.get(key).copied()
    }

    /// Remap an action, keeping one deterministic binding per key and
    /// preventing a required action from becoming unreachable.
    pub fn bind(&mut self, key: String, action: Action) -> Result<(), String> {
        if key.is_empty()
            || key.len() > 32
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'-'))
        {
            return Err("E-CONFIG-RANGE: invalid input key name".to_string());
        }
        self.bindings.retain(|_, existing| *existing != action);
        self.bindings.insert(key, action);
        Ok(())
    }
}

pub fn load_from(path: &Path) -> Result<InputBindings, String> {
    if !path.exists() {
        return Ok(InputBindings::default());
    }
    let metadata = std::fs::metadata(path).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    if metadata.len() > MAX_SETTINGS_BYTES {
        return Err("E-CONFIG-OVERSIZE: input.ron exceeds 64 KiB".to_string());
    }
    let text = std::fs::read_to_string(path).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    let bindings: InputBindings =
        ron::from_str(&text).map_err(|error| format!("E-CONFIG-PARSE: {error}"))?;
    if bindings.bindings.is_empty() {
        return Err("E-CONFIG-RANGE: input map may not be empty".to_string());
    }
    Ok(bindings)
}

pub fn load() -> Result<InputBindings, String> {
    load_from(&crate::settings::config_dir()?.join("input.ron"))
}

pub fn save_to(path: &Path, bindings: &InputBindings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "E-CONFIG-PATH: input path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    let text = ron::ser::to_string_pretty(bindings, ron::ser::PrettyConfig::default())
        .map_err(|error| format!("E-CONFIG-FORMAT: {error}"))?;
    std::fs::write(path, text).map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
            .map_err(|error| format!("E-CONFIG-IO: {error}"))?;
    }
    Ok(())
}

pub fn save(bindings: &InputBindings) -> Result<(), String> {
    save_to(&crate::settings::config_dir()?.join("input.ron"), bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remapped_binding_roundtrips() {
        let path =
            std::env::temp_dir().join(format!("powderburn-input-{}.ron", std::process::id()));
        let mut bindings = InputBindings::default();
        assert!(bindings.bind("z".to_string(), Action::Fire).is_ok());
        assert!(save_to(&path, &bindings).is_ok());
        let Ok(loaded) = load_from(&path) else {
            panic!("bindings must load");
        };
        let _ = std::fs::remove_file(path);
        assert_eq!(loaded.action_for("z"), Some(Action::Fire));
        assert_eq!(loaded.action_for("f"), None);
    }
}
