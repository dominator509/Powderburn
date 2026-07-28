//! Behavior-backed accessibility report for the shipped UI contract.
//!
//! The report executes the same screen graph, palette resolver, text layout,
//! overlay encodings, cue registry, and presentation clock used by the client.

#![allow(clippy::float_arithmetic)]

use std::collections::BTreeSet;

use pb_render::overlay::OverlayTileKind;
use pb_render::text::layout_wrapped_text;
use pb_render::ui_contract::{
    contrast_ratio, keyboard_traversal, palette_for_name, presentation_frame_interval, GameScreen,
    HUD_ACTION_SELECTED, HUD_ACTION_TARGETING, HUD_INSTRUCTION_IDLE, HUD_INSTRUCTION_SELECTED,
    HUD_INSTRUCTION_TARGETING, PALETTE_NAMES, SHIPPED_FLASHING_EFFECTS,
};

#[derive(Debug)]
struct A11yCheck {
    name: &'static str,
    passed: bool,
    detail: String,
}

/// Run the full accessibility report.
pub fn run_a11y_report() -> Result<(), String> {
    let checks = [
        check_color_only(),
        check_text_scale(),
        check_keyboard_traversal(),
        check_palette_contrast(),
        check_flashing_effects(),
        check_subtitles(),
        check_slow_clock(),
    ];

    let mut all_ok = true;
    for check in &checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        println!("a11y: {status} {}", check.name);
        if !check.passed {
            eprintln!("a11y detail: {}", check.detail);
            all_ok = false;
        }
    }
    if all_ok {
        println!("a11y: ok");
        Ok(())
    } else {
        let failed = checks
            .iter()
            .find(|check| !check.passed)
            .map_or("unknown", |check| check.name);
        Err(format!("a11y: FAIL {failed}"))
    }
}

fn check_color_only() -> A11yCheck {
    let samples = OverlayTileKind::ACCESSIBILITY_SAMPLES;
    let patterns: BTreeSet<_> = samples.iter().map(|kind| kind.pattern_id()).collect();
    let cues: BTreeSet<_> = samples.iter().map(|kind| kind.non_color_cue()).collect();
    let passed = patterns.len() == samples.len()
        && cues.len() == samples.len()
        && cues.iter().all(|cue| !cue.trim().is_empty());
    A11yCheck {
        name: "color-only",
        passed,
        detail: format!(
            "{} semantic overlays execute {} distinct shader patterns and {} text/glyph cues",
            samples.len(),
            patterns.len(),
            cues.len()
        ),
    }
}

fn check_text_scale() -> A11yCheck {
    let max_width = 1920.0 - 24.0;
    let scale = 4.0;
    let copy = [
        HUD_ACTION_SELECTED,
        HUD_ACTION_TARGETING,
        HUD_INSTRUCTION_IDLE,
        HUD_INSTRUCTION_SELECTED,
        HUD_INSTRUCTION_TARGETING,
    ];
    let lines: Vec<_> = copy
        .into_iter()
        .flat_map(|text| layout_wrapped_text(text, scale, max_width))
        .collect();
    let width_ok = lines.iter().all(|line| line.width <= max_width);
    let height: f32 = lines.iter().map(|line| line.height + 6.0).sum();
    let height_ok = height <= 1080.0 * 0.45;
    A11yCheck {
        name: "text-scale",
        passed: width_ok && height_ok && !lines.is_empty(),
        detail: format!(
            "production wrapper laid out {} lines at 200% within {:.0}x{:.0}px HUD bounds",
            lines.len(),
            max_width,
            1080.0 * 0.45
        ),
    }
}

fn check_keyboard_traversal() -> A11yCheck {
    let traversal = keyboard_traversal();
    let (passed, detail) = match traversal {
        Ok(path) => {
            let visited: BTreeSet<_> = path.iter().copied().collect();
            (
                visited == GameScreen::ALL.into_iter().collect(),
                format!(
                    "executed {} legal transitions and visited all {} runtime screens",
                    path.len().saturating_sub(1),
                    visited.len()
                ),
            )
        }
        Err(error) => (false, error),
    };
    A11yCheck {
        name: "keyboard",
        passed,
        detail,
    }
}

fn check_palette_contrast() -> A11yCheck {
    let ratios: Vec<_> = PALETTE_NAMES
        .into_iter()
        .map(|name| {
            let palette = palette_for_name(name);
            (name, contrast_ratio(palette.text, palette.background))
        })
        .collect();
    let passed = ratios.iter().all(|(_, ratio)| *ratio >= 4.5);
    A11yCheck {
        name: "palette",
        passed,
        detail: ratios
            .iter()
            .map(|(name, ratio)| format!("{name}={ratio:.1}:1"))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn check_flashing_effects() -> A11yCheck {
    let violations: Vec<_> = SHIPPED_FLASHING_EFFECTS
        .iter()
        .filter(|effect| effect.flashes_per_second > 3)
        .map(|effect| effect.name)
        .collect();
    A11yCheck {
        name: "flashing",
        passed: violations.is_empty(),
        detail: if SHIPPED_FLASHING_EFFECTS.is_empty() {
            "runtime effect registry contains no flashing effects".to_string()
        } else {
            format!(
                "{} registered effects checked; violations: {}",
                SHIPPED_FLASHING_EFFECTS.len(),
                violations.join(", ")
            )
        },
    }
}

fn check_subtitles() -> A11yCheck {
    let missing = pb_audio::Sfx::ALL
        .into_iter()
        .filter_map(|sfx| {
            let cue = sfx.cue();
            (cue.speaker.trim().is_empty() || cue.subtitle.trim().is_empty())
                .then_some(format!("{sfx:?}"))
        })
        .collect::<Vec<_>>();
    A11yCheck {
        name: "subtitles",
        passed: missing.is_empty(),
        detail: if missing.is_empty() {
            format!(
                "all {} runtime cues have speaker-labeled subtitles",
                pb_audio::Sfx::ALL.len()
            )
        } else {
            format!("cues missing speaker or subtitle: {}", missing.join(", "))
        },
    }
}

fn check_slow_clock() -> A11yCheck {
    let normal = presentation_frame_interval(false);
    let slow = presentation_frame_interval(true);
    A11yCheck {
        name: "slow-clock",
        passed: normal.as_millis() == 16 && slow.as_millis() == 32,
        detail: format!(
            "shared presentation clock changes redraw interval {}ms -> {}ms",
            normal.as_millis(),
            slow.as_millis()
        ),
    }
}
