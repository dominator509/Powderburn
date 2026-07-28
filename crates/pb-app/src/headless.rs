//! Headless mode for the powderburn binary.
//!
//! Runs the simulation and optionally captures frames without a window.

use std::path::PathBuf;

use pb_render::device::RenderDevice;
use pb_render::{CaptureMeta, RenderConfig};
use pb_sim::action::{Action, Command};
use pb_sim::clock::advance_to_next_actor;

use crate::state::GameScreen;

/// Configuration for headless mode.
#[derive(Debug)]
pub struct HeadlessConfig {
    pub scenario: String,
    pub tick: Option<u64>,
    pub capture: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            scenario: String::from("prov_full_battle"),
            tick: None,
            capture: None,
            width: 1920,
            height: 1080,
        }
    }
}

/// Validate a campaign script and walk every required UI screen using only
/// declared transitions. This is the machine-readable keyboard-reachability
/// proof used by EP-005 M4.
pub fn scripted_screen_traversal(script: &std::path::Path) -> Result<Vec<GameScreen>, String> {
    let text = std::fs::read_to_string(script)
        .map_err(|error| format!("E-UI-SCRIPT: cannot read {}: {error}", script.display()))?;
    let command_count = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| {
            let mut fields = line.split_whitespace();
            fields
                .next()
                .is_some_and(|tick| tick.parse::<u64>().is_ok())
                && fields
                    .next()
                    .is_some_and(|actor| actor.parse::<u32>().is_ok())
                && fields.next().is_some()
        })
        .count();
    if command_count == 0 {
        return Err("E-UI-SCRIPT: script contains no valid commands".to_string());
    }

    let path = [
        GameScreen::Settings,
        GameScreen::Title,
        GameScreen::LedgerView,
        GameScreen::Title,
        GameScreen::Bibliography,
        GameScreen::Title,
        GameScreen::NewCompany,
        GameScreen::Camp,
        GameScreen::LedgerView,
        GameScreen::Camp,
        GameScreen::Settings,
        GameScreen::Camp,
        GameScreen::MapTravel,
        GameScreen::Briefing,
        GameScreen::Battle,
        GameScreen::SaveSlot,
        GameScreen::Battle,
        GameScreen::LoadSlot,
        GameScreen::Battle,
        GameScreen::AfterAction,
    ];
    let mut screens = vec![GameScreen::Title];
    let mut current = GameScreen::Title;
    for target in path {
        current = current.transition(target)?;
        screens.push(current);
    }
    Ok(screens)
}

/// Run a headless capture of the given scenario at the given tick.
///
/// Returns the capture metadata on success.
pub async fn run_headless_capture(config: &HeadlessConfig) -> Result<CaptureMeta, String> {
    // Initialize the render device
    let device = RenderDevice::new_headless().await?;

    // Build render config
    let render_config = RenderConfig {
        width: config.width,
        height: config.height,
        adapter_name: None,
    };

    let capture_path = config
        .capture
        .as_ref()
        .ok_or_else(|| "no capture path specified".to_string())?;

    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let content_root = workspace.join("content");
    let journal = workspace
        .join("tests/journals")
        .join(format!("{}.jrnl", config.scenario));
    let requested_tick = config.tick.unwrap_or(0);
    let mut state = if requested_tick > 0 && journal.is_file() {
        pb_cli::cmd_sim::capture_reproduction(
            &content_root,
            &config.scenario,
            42,
            &journal,
            requested_tick,
        )?
        .state
    } else {
        pb_cli::cmd_sim::construct_scenario_state(&content_root, &config.scenario, 42)?
    };
    let mut steps = 0_u16;
    while state.tick.0 < requested_tick {
        steps = steps.saturating_add(1);
        if steps > 1_024 {
            return Err("E-CAPTURE-TICK: could not reach requested tick".to_string());
        }
        let Some(actor_id) = advance_to_next_actor(&mut state) else {
            break;
        };
        if state.tick.0 > requested_tick {
            break;
        }
        pb_sim::action::step(
            &mut state,
            Command {
                actor_id,
                action: Action::Hold,
            },
        )
        .map_err(|error| format!("E-CAPTURE-TICK: {error:?}"))?;
    }

    let meta =
        pb_render::capture::capture_state_frame(device, &render_config, &state, capture_path)
            .await?;

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_traversal_visits_every_required_screen() {
        let path = std::env::temp_dir().join(format!(
            "powderburn-screen-script-{}.script",
            std::process::id()
        ));
        assert!(std::fs::write(&path, "0 1 Hold\n").is_ok());
        let Ok(screens) = scripted_screen_traversal(&path) else {
            panic!("valid traversal must succeed");
        };
        let _ = std::fs::remove_file(path);
        for required in [
            GameScreen::Title,
            GameScreen::NewCompany,
            GameScreen::Camp,
            GameScreen::MapTravel,
            GameScreen::Briefing,
            GameScreen::Battle,
            GameScreen::AfterAction,
            GameScreen::LedgerView,
            GameScreen::Settings,
            GameScreen::Bibliography,
            GameScreen::SaveSlot,
            GameScreen::LoadSlot,
        ] {
            assert!(screens.contains(&required), "missing {required:?}");
        }
        assert_eq!(screens.last(), Some(&GameScreen::AfterAction));
    }
}
