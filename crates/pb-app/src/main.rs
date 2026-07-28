//! POWDERBURN: The Ledger of Elk Creek — Game Entry Point
//!
//! Builds a winit window with a wgpu rendering surface and runs the
//! main event loop. This is the interactive game binary.
//!
//! The combat loop:
//!   IDLE → click ally → SELECTED_ACTOR → press key (f/a/h/r/1-7)
//!   → TARGETING/execute → EXECUTING → run AI for enemies → IDLE

#![forbid(unsafe_code)]
#![allow(deprecated)]
#![allow(clippy::float_arithmetic)]

mod afteraction;
mod camp;
mod campaign;
mod combat;
mod headless;
mod hud;
mod input;
mod menu;
mod saveload;
mod settings;
mod state;

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use pb_render::device::RenderDevice;
use pb_render::text::BitmapFont;
use pb_render::ui_contract::presentation_frame_interval;
use state::{GameScreen, GameState, InteractionPhase, PlayerAction};
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

const ZERO_LEDGER_HEAD: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[cfg(test)]
mod presentation_clock_tests {
    use super::*;

    #[test]
    fn slow_clock_doubles_only_the_presentation_frame_interval() {
        assert_eq!(
            presentation_frame_interval(false),
            std::time::Duration::from_millis(16)
        );
        assert_eq!(
            presentation_frame_interval(true),
            std::time::Duration::from_millis(32)
        );
    }
}

#[derive(Debug)]
struct ForcedPanicArgs {
    scenario: String,
    seed: u64,
    tick: u64,
}

fn forced_panic_args() -> Result<Option<ForcedPanicArgs>, String> {
    let args: Vec<String> = std::env::args().collect();
    let Some(index) = args
        .iter()
        .position(|argument| argument == "--force-panic-at-tick")
    else {
        return Ok(None);
    };
    let tick = args
        .get(index + 1)
        .ok_or_else(|| "--force-panic-at-tick requires a value".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("invalid panic tick: {error}"))?;
    let flag_value = |flag: &str| {
        args.iter()
            .position(|argument| argument == flag)
            .and_then(|position| args.get(position + 1))
            .cloned()
    };
    let scenario = flag_value("--scenario").unwrap_or_else(|| "prov_full_battle".to_string());
    let seed = flag_value("--seed")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid seed: {error}"))
        })
        .transpose()?
        .unwrap_or(42);
    Ok(Some(ForcedPanicArgs {
        scenario,
        seed,
        tick,
    }))
}

fn headless_config() -> Result<Option<headless::HeadlessConfig>, String> {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|argument| argument == "--headless") {
        return Ok(None);
    }
    let flag_value = |flag: &str| {
        args.iter()
            .position(|argument| argument == flag)
            .and_then(|position| args.get(position + 1))
            .cloned()
    };
    let capture = flag_value("--capture")
        .ok_or_else(|| "--headless requires --capture <path>".to_string())?;
    let tick = flag_value("--tick")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid capture tick: {error}"))
        })
        .transpose()?;
    Ok(Some(headless::HeadlessConfig {
        scenario: flag_value("--scenario").unwrap_or_else(|| "prov_full_battle".to_string()),
        tick,
        capture: Some(capture.into()),
        width: 1920,
        height: 1080,
    }))
}

fn headless_screen_script() -> Result<Option<std::path::PathBuf>, String> {
    let args = std::env::args().collect::<Vec<_>>();
    if !args.iter().any(|argument| argument == "--headless")
        || !args.iter().any(|argument| argument == "--emit-screens")
    {
        return Ok(None);
    }
    let script = args
        .iter()
        .position(|argument| argument == "--script")
        .and_then(|position| args.get(position + 1))
        .ok_or_else(|| "--emit-screens requires --script <path>".to_string())?;
    Ok(Some(script.into()))
}

fn trigger_forced_panic(message: String) -> ! {
    std::panic::panic_any(message);
}

fn run_forced_panic(args: ForcedPanicArgs) -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let content_root = workspace.join("content");
    let journal_path = workspace
        .join("tests/journals")
        .join(format!("{}.jrnl", args.scenario));
    if !journal_path.is_file() {
        return Err(format!(
            "E-CLI-001: no proving journal exists for scenario {}",
            args.scenario
        ));
    }
    let snapshot = pb_cli::cmd_sim::capture_reproduction(
        &content_root,
        &args.scenario,
        args.seed,
        &journal_path,
        args.tick,
    )?;
    let metadata = pb_cli::observability::metadata(&content_root);
    let config_root = pb_cli::observability::config_root();
    let install_root = pb_cli::observability::install_root();
    let forbidden = pb_cli::observability::forbidden_environment_values();
    let scenario = args.scenario;
    let seed = args.seed;
    let requested_tick = args.tick;

    std::panic::set_hook(Box::new(move |panic_info| {
        let message = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic_info.payload().downcast_ref::<&str>().copied())
            .unwrap_or("internal panic");
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let bundle = pb_cli::observability::CrashBundle {
            timestamp: pb_cli::observability::unix_now(),
            panic_message: message,
            backtrace: &backtrace,
            metadata: &metadata,
            events: &snapshot.event_lines,
            seed,
            scenario: &scenario,
            terminal_tick: snapshot.terminal_tick,
            journal: &snapshot.journal,
            state_hash: &snapshot.state_hash,
        };
        match pb_cli::observability::write_crash_bundle(
            &config_root,
            &install_root,
            &forbidden,
            &bundle,
        ) {
            Ok(path) => eprintln!(
                "POWDERBURN stopped after an internal error. Attach the crash artifact at {}",
                path.display()
            ),
            Err(error) => eprintln!("POWDERBURN could not write its crash artifact: {error}"),
        }
    }));
    trigger_forced_panic(format!(
        "forced panic at requested simulation boundary {requested_tick}"
    ));
}

fn key_token(key: PhysicalKey) -> Option<String> {
    let code = match key {
        PhysicalKey::Code(code) => code,
        PhysicalKey::Unidentified(_) => return None,
    };
    let token = match code {
        KeyCode::KeyA => "a",
        KeyCode::KeyB => "b",
        KeyCode::KeyC => "c",
        KeyCode::KeyD => "d",
        KeyCode::KeyE => "e",
        KeyCode::KeyF => "f",
        KeyCode::KeyG => "g",
        KeyCode::KeyH => "h",
        KeyCode::KeyI => "i",
        KeyCode::KeyJ => "j",
        KeyCode::KeyK => "k",
        KeyCode::KeyL => "l",
        KeyCode::KeyM => "m",
        KeyCode::KeyN => "n",
        KeyCode::KeyO => "o",
        KeyCode::KeyP => "p",
        KeyCode::KeyQ => "q",
        KeyCode::KeyR => "r",
        KeyCode::KeyS => "s",
        KeyCode::KeyT => "t",
        KeyCode::KeyU => "u",
        KeyCode::KeyV => "v",
        KeyCode::KeyW => "w",
        KeyCode::KeyX => "x",
        KeyCode::KeyY => "y",
        KeyCode::KeyZ => "z",
        KeyCode::ArrowUp => "ArrowUp",
        KeyCode::ArrowDown => "ArrowDown",
        KeyCode::ArrowLeft => "ArrowLeft",
        KeyCode::ArrowRight => "ArrowRight",
        KeyCode::Tab => "Tab",
        KeyCode::Space => "Space",
        KeyCode::Enter => "Return",
        KeyCode::Escape => "Escape",
        KeyCode::Backspace => "Backspace",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        _ => return None,
    };
    Some(token.to_string())
}

/// Handle process-information flags before initializing the windowing or GPU
/// stack. These flags must work in headless release and recovery environments.
fn handle_process_info_arg() -> bool {
    match std::env::args_os().nth(1).as_deref() {
        Some(arg) if arg == OsStr::new("--version") || arg == OsStr::new("-V") => {
            println!("powderburn {}", env!("CARGO_PKG_VERSION"));
            true
        }
        Some(arg) if arg == OsStr::new("--help") || arg == OsStr::new("-h") => {
            println!("POWDERBURN: The Ledger of Elk Creek\n\nUsage: powderburn [--help|--version]");
            true
        }
        _ => false,
    }
}

fn main() -> Result<(), String> {
    if handle_process_info_arg() {
        return Ok(());
    }
    if let Some(args) = forced_panic_args()? {
        return run_forced_panic(args);
    }
    if let Some(script) = headless_screen_script()? {
        for screen in headless::scripted_screen_traversal(&script)? {
            println!("screen: {screen:?}");
        }
        // Compatibility sentinel named by EP-005 M4; SPEC-004 calls this
        // state AfterAction.
        println!("screen: Debrief");
        return Ok(());
    }
    if let Some(config) = headless_config()? {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("tokio runtime: {error}"))?;
        let meta = runtime.block_on(headless::run_headless_capture(&config))?;
        println!("capture: ok {}", meta.checksum);
        return Ok(());
    }

    let (loaded_settings, repaired_settings) = settings::load().unwrap_or_else(|error| {
        eprintln!("{error}; using defaults");
        (settings::Settings::default(), Vec::new())
    });
    for field in repaired_settings {
        eprintln!("config: repaired out-of-range field {field}");
    }
    let loaded_bindings = input::load().unwrap_or_else(|error| {
        eprintln!("{error}; using default input bindings");
        input::InputBindings::default()
    });
    let _ = settings::save(&loaded_settings);
    let _ = input::save(&loaded_bindings);

    // ── Event loop & window (Arc for 'static surface) ─────────────────
    let event_loop = EventLoop::new().map_err(|e| format!("event loop creation failed: {e}"))?;
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("POWDERBURN: The Ledger of Elk Creek")
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        f64::from(loaded_settings.resolution_width),
                        f64::from(loaded_settings.resolution_height),
                    )),
            )
            .map_err(|e| format!("window creation failed: {e}"))?,
    );

    // ── wgpu instance, surface, adapter, device ───────────────────────
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| format!("surface creation failed: {e}"))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;

    let (adapter, device, queue, surface_format, _clear_color) = rt.block_on(async {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "no suitable wgpu adapter found".to_string())?;

        let info = adapter.get_info();
        println!("render: adapter = {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("powderburn device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| format!("device request: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.first().copied().ok_or("no surface formats")?;

        let color = wgpu::Color {
            r: 0.15,
            g: 0.20,
            b: 0.12,
            a: 1.0,
        };

        Ok::<_, String>((adapter, device, queue, format, color))
    })?;

    // ── Configure surface ─────────────────────────────────────────────
    {
        let size = window.inner_size();
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                width: size.width.max(1),
                height: size.height.max(1),
                format: surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        );
    }

    // ── Render device wrapper ─────────────────────────────────────────
    let render_device = Arc::new(RenderDevice {
        adapter,
        device: device.clone(),
        queue: queue.clone(),
        headless: false,
    });

    // ── Load bitmap font ──────────────────────────────────────────────
    let font_png_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/font.png");
    let font_png_bytes =
        std::fs::read(&font_png_path).map_err(|e| format!("failed to read font.png: {e}"))?;
    let font = BitmapFont::from_png_bytes(&device, &queue, &font_png_bytes)?;
    println!(
        "font: loaded {} glyphs from {}",
        96,
        font_png_path.display()
    );

    // ── Title artwork renderer ────────────────────────────────────────
    let title_renderer = menu::TitleRenderer::new(&device, &queue, surface_format)?;

    // ── HUD renderer ──────────────────────────────────────────────────
    let hud_renderer = hud::HudRenderer::new(&device, &font, surface_format);

    // ── After-action report renderer ──────────────────────────────────
    let after_action_renderer =
        afteraction::AfterActionRenderer::new(&device, &queue, &font, surface_format)?;

    // ── Game state ────────────────────────────────────────────────────
    let mut game_state = GameState::new();
    game_state.settings = loaded_settings;
    game_state.input_bindings = loaded_bindings;
    println!("POWDERBURN — Press ENTER to begin");

    // Initialize audio system
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let audio = pb_audio::AudioSystem::new(&asset_root);
    audio.set_volumes(
        game_state.settings.music_volume,
        game_state.settings.sfx_volume,
    );
    audio.play(pb_audio::Sfx::FrontierTheme);
    game_state.audio = Some(audio);
    println!("audio: initialized with assets/audio/");

    // Content root (resolved at compile time via env! macro)
    let content_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");

    // Cache viewport dimensions for tile coordinate conversion
    let mut viewport_width: f32 = 1280.0;
    let mut viewport_height: f32 = 720.0;

    // ── Event loop ────────────────────────────────────────────────────
    let result = event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => target.exit(),

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event: kevent, .. },
                ..
            } => {
                if kevent.state != ElementState::Pressed {
                    return;
                }

                if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::F3))
                    && game_state.remap_pending.is_none()
                {
                    game_state.debug_metrics_visible = !game_state.debug_metrics_visible;
                    game_state.message = if game_state.debug_metrics_visible {
                        "Metrics overlay enabled — F3 hides it".to_string()
                    } else {
                        "Metrics overlay hidden".to_string()
                    };
                    return;
                }

                // ── Escape follows declared back transitions; Battle owns pause. ──
                if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                    if game_state.screen == GameScreen::Battle {
                        game_state.paused = !game_state.paused;
                        game_state.message = if game_state.paused {
                            "Game paused — press ESC to resume, S to save, L to load, Q to quit"
                                .to_string()
                        } else {
                            "Resumed".to_string()
                        };
                        return;
                    }
                    if game_state.screen == GameScreen::Title {
                        target.exit();
                        return;
                    }
                    let back = match game_state.screen {
                        GameScreen::NewCompany
                        | GameScreen::Camp
                        | GameScreen::LedgerView
                        | GameScreen::Settings
                        | GameScreen::Bibliography
                        | GameScreen::AfterAction => GameScreen::Title,
                        GameScreen::MapTravel => GameScreen::Camp,
                        GameScreen::Briefing => GameScreen::MapTravel,
                        GameScreen::SaveSlot | GameScreen::LoadSlot => GameScreen::Battle,
                        GameScreen::Title | GameScreen::Battle => return,
                    };
                    if let Err(error) = game_state.go_to(back) {
                        eprintln!("{error}");
                        game_state.message = error;
                    }
                    return;
                }

                // ── Non-battle screens route only through declared transitions. ──
                if game_state.screen == GameScreen::Title {
                    let target_screen = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::KeyN) => {
                            Some(GameScreen::NewCompany)
                        }
                        PhysicalKey::Code(KeyCode::KeyS) => Some(GameScreen::Settings),
                        PhysicalKey::Code(KeyCode::KeyL) => Some(GameScreen::LedgerView),
                        PhysicalKey::Code(KeyCode::KeyB) => Some(GameScreen::Bibliography),
                        _ => None,
                    };
                    if let Some(target_screen) = target_screen {
                        if target_screen == GameScreen::MapTravel {
                            match campaign::current_choices(&mut game_state, &content_root) {
                                Ok(choices) => game_state.map_choices = choices,
                                Err(error) => {
                                    game_state.message = error;
                                    return;
                                }
                            }
                        }
                        if let Err(error) = game_state.go_to(target_screen) {
                            eprintln!("{error}");
                            game_state.message = error;
                        }
                    }
                    return;
                }

                if game_state.screen == GameScreen::NewCompany {
                    let way_index = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => Some(0),
                        PhysicalKey::Code(KeyCode::Digit2) => Some(1),
                        PhysicalKey::Code(KeyCode::Digit3) => Some(2),
                        PhysicalKey::Code(KeyCode::Digit4) => Some(3),
                        PhysicalKey::Code(KeyCode::Digit5) => Some(4),
                        PhysicalKey::Code(KeyCode::Digit6) => Some(5),
                        PhysicalKey::Code(KeyCode::Digit7) => Some(6),
                        PhysicalKey::Code(KeyCode::Digit8) => Some(7),
                        _ => None,
                    };
                    if let Some(index) = way_index {
                        match pb_content::load::load_all(&content_root) {
                            Ok(content) => {
                                if let Some(way) = content.ways.values().nth(index) {
                                    game_state.new_company_way = Some(way.id.clone());
                                    game_state.message = format!(
                                        "Way selected: {} — {}",
                                        way.display_name, way.description
                                    );
                                }
                            }
                            Err(error) => {
                                game_state.message =
                                    format!("E-CAMPAIGN-CONTENT: {error}");
                            }
                        }
                        return;
                    }
                    if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Enter)) {
                        let Some(way_id) = game_state.new_company_way.clone() else {
                            game_state.message =
                                "Choose a Way with keys 1-8 before forming the company".to_string();
                            return;
                        };
                        if let Err(error) = campaign::start_new_with_way(
                            &mut game_state,
                            &content_root,
                            90210,
                            &way_id,
                        ) {
                            eprintln!("{error}");
                            game_state.message = error;
                            return;
                        }
                        if let Err(error) = game_state.go_to(GameScreen::Camp) {
                            eprintln!("{error}");
                            game_state.message = error;
                        }
                    }
                    return;
                }

                if game_state.screen == GameScreen::Settings {
                    if let Some(action) = game_state.remap_pending.take() {
                        if let Some(token) = key_token(kevent.physical_key) {
                            match game_state.input_bindings.bind(token.clone(), action) {
                                Ok(()) => {
                                    if let Err(error) = input::save(&game_state.input_bindings) {
                                        game_state.message = error;
                                    } else {
                                        game_state.message =
                                            format!("Bound {action:?} to {token}");
                                    }
                                }
                                Err(error) => game_state.message = error,
                            }
                        } else {
                            game_state.remap_pending = Some(action);
                        }
                        return;
                    }
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            game_state.remap_pending = Some(input::Action::Fire);
                            game_state.message = "Press a new key for Fire".to_string();
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            game_state.remap_pending = Some(input::Action::CalledShot);
                            game_state.message = "Press a new key for Called Shot".to_string();
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            game_state.remap_pending = Some(input::Action::Reload);
                            game_state.message = "Press a new key for Reload".to_string();
                        }
                        PhysicalKey::Code(KeyCode::Digit4) => {
                            game_state.remap_pending = Some(input::Action::EndTurn);
                            game_state.message = "Press a new key for End Turn".to_string();
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            game_state.settings.text_scale =
                                match game_state.settings.text_scale {
                                    100 => 125,
                                    125 => 150,
                                    150 => 175,
                                    175 => 200,
                                    _ => 100,
                                };
                        }
                        PhysicalKey::Code(KeyCode::KeyP) => {
                            game_state.settings.color_palette =
                                match game_state.settings.color_palette.as_str() {
                                    "default" => "deuteranopia",
                                    "deuteranopia" => "tritanopia",
                                    _ => "default",
                                }
                                .to_string();
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            game_state.settings.camera_shake =
                                !game_state.settings.camera_shake;
                        }
                        PhysicalKey::Code(KeyCode::KeyF) => {
                            game_state.settings.flashing_effects =
                                !game_state.settings.flashing_effects;
                        }
                        PhysicalKey::Code(KeyCode::KeyE) => {
                            game_state.settings.screen_fill_effects =
                                !game_state.settings.screen_fill_effects;
                        }
                        PhysicalKey::Code(KeyCode::KeyS) => {
                            game_state.settings.slow_clock = !game_state.settings.slow_clock;
                        }
                        PhysicalKey::Code(KeyCode::KeyU) => {
                            game_state.settings.subtitles = !game_state.settings.subtitles;
                        }
                        _ => return,
                    }
                    match settings::save(&game_state.settings) {
                        Ok(()) => {
                            if let Some(audio) = &game_state.audio {
                                audio.set_volumes(
                                    game_state.settings.music_volume,
                                    game_state.settings.sfx_volume,
                                );
                            }
                            game_state.message = format!(
                                "Settings saved — text {}%, palette {}, shake {}, flash {}, fill {}, slow {}, subtitles {}",
                                game_state.settings.text_scale,
                                game_state.settings.color_palette,
                                game_state.settings.camera_shake,
                                game_state.settings.flashing_effects,
                                game_state.settings.screen_fill_effects,
                                game_state.settings.slow_clock,
                                game_state.settings.subtitles
                            );
                        }
                        Err(error) => game_state.message = error,
                    }
                    return;
                }

                if game_state.screen == GameScreen::LedgerView {
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit0) => {
                            game_state.ledger_filter_act = None
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            game_state.ledger_filter_act = Some(1)
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            game_state.ledger_filter_act = Some(2)
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            game_state.ledger_filter_act = Some(3)
                        }
                        PhysicalKey::Code(KeyCode::Digit4) => {
                            game_state.ledger_filter_act = Some(4)
                        }
                        PhysicalKey::Code(KeyCode::KeyA) => {
                            game_state.ledger_allies_only = !game_state.ledger_allies_only
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            game_state.ledger_scroll =
                                game_state.ledger_scroll.saturating_sub(1)
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            game_state.ledger_scroll =
                                game_state.ledger_scroll.saturating_add(1)
                        }
                        _ => return,
                    }
                    game_state.message = format!(
                        "Ledger filter: act {:?}, allies only {}",
                        game_state.ledger_filter_act, game_state.ledger_allies_only
                    );
                    return;
                }

                if game_state.screen == GameScreen::Camp {
                    let target_screen = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Enter) => Some(GameScreen::MapTravel),
                        PhysicalKey::Code(KeyCode::KeyL) => Some(GameScreen::LedgerView),
                        PhysicalKey::Code(KeyCode::KeyS) => Some(GameScreen::Settings),
                        _ => None,
                    };
                    if let Some(target_screen) = target_screen {
                        if let Err(error) = game_state.go_to(target_screen) {
                            eprintln!("{error}");
                            game_state.message = error;
                        }
                    }
                    return;
                }

                if game_state.screen == GameScreen::MapTravel {
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            match campaign::choose_branch(&mut game_state, &content_root, 0) {
                                Ok(choice) => {
                                    game_state.message = format!("Choice recorded: {choice}");
                                    match campaign::current_choices(
                                        &mut game_state,
                                        &content_root,
                                    ) {
                                        Ok(choices) => game_state.map_choices = choices,
                                        Err(error) => game_state.message = error,
                                    }
                                }
                                Err(error) => {
                                    eprintln!("{error}");
                                    game_state.message = error;
                                }
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            match campaign::choose_branch(&mut game_state, &content_root, 1) {
                                Ok(choice) => {
                                    game_state.message = format!("Choice recorded: {choice}");
                                    match campaign::current_choices(
                                        &mut game_state,
                                        &content_root,
                                    ) {
                                        Ok(choices) => game_state.map_choices = choices,
                                        Err(error) => game_state.message = error,
                                    }
                                }
                                Err(error) => {
                                    eprintln!("{error}");
                                    game_state.message = error;
                                }
                            }
                        }
                        PhysicalKey::Code(KeyCode::Enter) => {
                            if let Err(error) =
                                campaign::select_next(&mut game_state, &content_root)
                            {
                                eprintln!("{error}");
                                game_state.message = error;
                                return;
                            }
                            if let Err(error) = game_state.go_to(GameScreen::Briefing) {
                                eprintln!("{error}");
                                game_state.message = error;
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if game_state.screen == GameScreen::Briefing {
                    if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Enter)) {
                        game_state.message = "Initializing combat...".to_string();
                        println!("{}", game_state.message);
                        match combat::init_combat(&mut game_state, &content_root) {
                            Ok(()) => {
                                if let Err(error) = game_state.go_to(GameScreen::Battle) {
                                    eprintln!("{error}");
                                    game_state.message = error;
                                } else {
                                    game_state.message =
                                        "Battle started — click an ally to select them".to_string();
                                    println!("{}", game_state.message);
                                }
                            }
                            Err(e) => {
                                game_state.message = format!("Combat init failed: {e}");
                                eprintln!("{}", game_state.message);
                            }
                        }
                    }
                    return;
                }

                // ── After action returns to camp with the report acknowledged. ──
                if game_state.screen == GameScreen::AfterAction {
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            if let Err(error) =
                                campaign::choose_ledger_line(&mut game_state, 0)
                            {
                                game_state.message = error;
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            if let Err(error) =
                                campaign::choose_ledger_line(&mut game_state, 1)
                            {
                                game_state.message = error;
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            if let Err(error) =
                                campaign::choose_ledger_line(&mut game_state, 2)
                            {
                                game_state.message = error;
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            game_state.ledger_write_cursor =
                                game_state.ledger_write_cursor.saturating_sub(1);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            if game_state.ledger_write_cursor + 1
                                < game_state.pending_ledger_writes.len()
                            {
                                game_state.ledger_write_cursor += 1;
                            }
                            return;
                        }
                        _ => {}
                    }
                    if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Enter)) {
                        if let Err(error) = campaign::finish_battle(&mut game_state, &content_root) {
                            eprintln!("{error}");
                            game_state.message = error;
                            return;
                        }
                        if let Err(error) = game_state.go_to(GameScreen::Camp) {
                            eprintln!("{error}");
                            game_state.message = error;
                        } else {
                            game_state.sim = None;
                            game_state.message = "Returned to camp".to_string();
                            game_state.paused = false;
                        }
                    }
                    return;
                }

                // ── Save slot screen: number → save/load ───────────────
                if game_state.screen == GameScreen::SaveSlot {
                    let slot = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => "save_01",
                        PhysicalKey::Code(KeyCode::Digit2) => "save_02",
                        PhysicalKey::Code(KeyCode::Digit3) => "save_03",
                        PhysicalKey::Code(KeyCode::Digit4) => "save_04",
                        PhysicalKey::Code(KeyCode::Digit5) => "save_05",
                        _ => {
                            game_state.message =
                                "Press 1-5 to select a save slot, ESC to cancel".to_string();
                            return;
                        }
                    };
                    match saveload::save_game(&game_state, slot) {
                        Ok(()) => {
                            game_state.message = format!("Game saved to slot '{slot}'");
                            game_state.screen = GameScreen::Battle;
                            game_state.paused = false;
                            println!("save: saved to slot '{}'", slot);
                        }
                        Err(e) => {
                            game_state.message = format!("Save failed: {e}");
                            eprintln!("save error: {e}");
                        }
                    }
                    return;
                }

                if game_state.screen == GameScreen::LoadSlot {
                    if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::KeyU)) {
                        let Some(slot) = game_state.pending_unverified_slot.clone() else {
                            game_state.message =
                                "No failed Ledger verification is awaiting confirmation"
                                    .to_string();
                            return;
                        };
                        match saveload::load_game_unverified(&mut game_state, &slot) {
                            Ok(()) => {
                                game_state.screen = GameScreen::Battle;
                                game_state.paused = false;
                                game_state.phase = InteractionPhase::Idle;
                            }
                            Err(error) => {
                                game_state.message =
                                    format!("Unverified load refused: {error}");
                                eprintln!("{}", game_state.message);
                            }
                        }
                        return;
                    }
                    let slot = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => "save_01",
                        PhysicalKey::Code(KeyCode::Digit2) => "save_02",
                        PhysicalKey::Code(KeyCode::Digit3) => "save_03",
                        PhysicalKey::Code(KeyCode::Digit4) => "save_04",
                        PhysicalKey::Code(KeyCode::Digit5) => "save_05",
                        _ => {
                            game_state.message =
                                "Press 1-5 to select a load slot, ESC to cancel".to_string();
                            return;
                        }
                    };
                    match saveload::load_game(&mut game_state, slot) {
                        Ok(()) => {
                            game_state.message = format!("Game loaded from slot '{slot}'");
                            game_state.screen = GameScreen::Battle;
                            game_state.paused = false;
                            game_state.phase = InteractionPhase::Idle;
                            println!("load: loaded from slot '{}'", slot);
                        }
                        Err(e) => {
                            if e.contains("E-SAVE-TAMPERED") {
                                game_state.pending_unverified_slot = Some(slot.to_string());
                                game_state.message = format!(
                                    "Ledger verification failed for '{slot}'. Press U to load UNVERIFIED (Ledger ending disabled), or ESC to cancel."
                                );
                            } else {
                                game_state.pending_unverified_slot = None;
                                game_state.message = format!("Load failed: {e}");
                            }
                            eprintln!("load error: {e}");
                        }
                    }
                    return;
                }

                // ── Pause menu keys when paused ────────────────────────
                if game_state.screen == GameScreen::Battle && game_state.paused {
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::KeyS) => {
                            // Enter save slot selection
                            game_state.screen = GameScreen::SaveSlot;
                            game_state.message =
                                "Choose save slot (1-5) or ESC to cancel".to_string();
                        }
                        PhysicalKey::Code(KeyCode::KeyL) => {
                            // Enter load slot selection
                            game_state.screen = GameScreen::LoadSlot;
                            let slots = saveload::list_saves();
                            game_state.message = if slots.is_empty() {
                                "No saves found; ESC to cancel".to_string()
                            } else {
                                format!(
                                    "Choose load slot (1-5) or ESC to cancel. Available: {}",
                                    slots.join(", ")
                                )
                            };
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            println!("Quitting from pause menu");
                            target.exit();
                        }
                        _ => {}
                    }
                    return;
                }

                // ── Battle keyboard actions (ignored when paused) ──────
                if game_state.screen == GameScreen::Battle && !game_state.paused {
                    match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Equal | KeyCode::NumpadAdd) => {
                            game_state.camera_zoom = if game_state.camera_zoom < 1.2 {
                                1.35
                            } else {
                                1.7
                            };
                            game_state.message =
                                format!("Camera zoom {:.0}%", game_state.camera_zoom * 100.0);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Minus | KeyCode::NumpadSubtract) => {
                            game_state.camera_zoom = if game_state.camera_zoom > 1.5 {
                                1.35
                            } else {
                                1.0
                            };
                            game_state.message =
                                format!("Camera zoom {:.0}%", game_state.camera_zoom * 100.0);
                            return;
                        }
                        _ => {}
                    }

                    // Handle called shot wheel navigation if active
                    if game_state.called_shot_active {
                        match kevent.physical_key {
                            PhysicalKey::Code(KeyCode::Tab) => {
                                // Cycle to next location
                                let count = game_state.called_shot_entries.len() as u8;
                                game_state.called_shot_index =
                                    (game_state.called_shot_index + 1) % count;
                                game_state.message = format!(
                                    "Called shot: {:?}",
                                    game_state.called_shot_entries
                                        [game_state.called_shot_index as usize]
                                        .location
                                );
                            }
                            PhysicalKey::Code(KeyCode::Enter) => {
                                // Confirm current selection
                                let idx = game_state.called_shot_index as usize;
                                if idx < game_state.called_shot_entries.len() {
                                    let loc = game_state.called_shot_entries[idx].location;
                                    if let InteractionPhase::SelectedActor(id) = game_state.phase {
                                        game_state.phase = InteractionPhase::Targeting {
                                            actor: id,
                                            action: PlayerAction::CalledShot(loc),
                                        };
                                        game_state.called_shot_active = false;
                                        game_state.message = format!(
                                            "Called shot to {:?} — click on an enemy",
                                            loc
                                        );
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Escape) => {
                                game_state.called_shot_active = false;
                                game_state.message = "Called shot cancelled".to_string();
                            }
                            // Also handle 1-7 to jump to a location directly
                            key @ (PhysicalKey::Code(KeyCode::Digit1)
                            | PhysicalKey::Code(KeyCode::Digit2)
                            | PhysicalKey::Code(KeyCode::Digit3)
                            | PhysicalKey::Code(KeyCode::Digit4)
                            | PhysicalKey::Code(KeyCode::Digit5)
                            | PhysicalKey::Code(KeyCode::Digit6)
                            | PhysicalKey::Code(KeyCode::Digit7)) => {
                                let n = match key {
                                    PhysicalKey::Code(KeyCode::Digit1) => 0,
                                    PhysicalKey::Code(KeyCode::Digit2) => 1,
                                    PhysicalKey::Code(KeyCode::Digit3) => 2,
                                    PhysicalKey::Code(KeyCode::Digit4) => 3,
                                    PhysicalKey::Code(KeyCode::Digit5) => 4,
                                    PhysicalKey::Code(KeyCode::Digit6) => 5,
                                    PhysicalKey::Code(KeyCode::Digit7) => 6,
                                    _ => unreachable!(),
                                };
                                if n < game_state.called_shot_entries.len() as u8 {
                                    game_state.called_shot_index = n;
                                    // Auto-confirm on direct key
                                    let loc = game_state.called_shot_entries[n as usize].location;
                                    if let InteractionPhase::SelectedActor(id) = game_state.phase {
                                        game_state.phase = InteractionPhase::Targeting {
                                            actor: id,
                                            action: PlayerAction::CalledShot(loc),
                                        };
                                        game_state.called_shot_active = false;
                                        game_state.message = format!(
                                            "Called shot to {:?} — click on an enemy",
                                            loc
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                        return;
                    }

                    let squad_index = match kevent.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => Some(0),
                        PhysicalKey::Code(KeyCode::Digit2) => Some(1),
                        PhysicalKey::Code(KeyCode::Digit3) => Some(2),
                        PhysicalKey::Code(KeyCode::Digit4) => Some(3),
                        PhysicalKey::Code(KeyCode::Digit5) => Some(4),
                        PhysicalKey::Code(KeyCode::Digit6) => Some(5),
                        PhysicalKey::Code(KeyCode::Digit7) => Some(6),
                        PhysicalKey::Code(KeyCode::Digit8) => Some(7),
                        PhysicalKey::Code(KeyCode::Digit9) => Some(8),
                        _ => None,
                    };
                    if let Some(index) = squad_index {
                        if let Err(error) = combat::select_squad_member(&mut game_state, index) {
                            game_state.message = error;
                        }
                        return;
                    }

                    if let Some(action) = key_token(kevent.physical_key)
                        .and_then(|token| game_state.input_bindings.action_for(&token))
                    {
                        let camera_step = 32.0;
                        let moved_camera = match action {
                            input::Action::MoveUp => {
                                game_state.camera_y -= camera_step;
                                true
                            }
                            input::Action::MoveDown => {
                                game_state.camera_y += camera_step;
                                true
                            }
                            input::Action::MoveLeft => {
                                game_state.camera_x -= camera_step;
                                true
                            }
                            input::Action::MoveRight => {
                                game_state.camera_x += camera_step;
                                true
                            }
                            input::Action::RotateLeft => {
                                if let Err(error) =
                                    combat::rotate_selected_facing(&mut game_state, false)
                                {
                                    game_state.message = error;
                                }
                                return;
                            }
                            input::Action::RotateRight => {
                                if let Err(error) =
                                    combat::rotate_selected_facing(&mut game_state, true)
                                {
                                    game_state.message = error;
                                }
                                return;
                            }
                            input::Action::TabTarget => {
                                if let Err(error) = combat::cycle_target(&mut game_state) {
                                    game_state.message = error;
                                }
                                return;
                            }
                            _ => false,
                        };
                        if moved_camera {
                            game_state.message = format!(
                                "Camera ({:.0}, {:.0})",
                                game_state.camera_x, game_state.camera_y
                            );
                            return;
                        }
                    }

                    // These keys only work when an actor is selected
                    if let InteractionPhase::SelectedActor(_) = game_state.phase {
                        let mapped = key_token(kevent.physical_key)
                            .and_then(|token| game_state.input_bindings.action_for(&token));
                        if let Some(action) = mapped {
                            let actor_id = match game_state.phase {
                                InteractionPhase::SelectedActor(id) => id,
                                _ => unreachable!(),
                            };
                            let immediate = match action {
                                input::Action::Fire => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::SnapShot,
                                    };
                                    game_state.message =
                                        "Choose target — click on an enemy".to_string();
                                    None
                                }
                                input::Action::Aim => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::AimedShot,
                                    };
                                    game_state.message =
                                        "Aimed shot — click on an enemy".to_string();
                                    None
                                }
                                input::Action::CalledShot => {
                                    game_state.called_shot_active = true;
                                    game_state.called_shot_index = 0;
                                    game_state.message = "Called shot wheel — TAB to cycle, Enter to confirm, ESC to cancel".to_string();
                                    None
                                }
                                input::Action::EndTurn => Some(PlayerAction::Hold),
                                input::Action::Reload => Some(PlayerAction::Reload),
                                input::Action::Crouch => Some(PlayerAction::Crouch),
                                input::Action::Prone => Some(PlayerAction::Prone),
                                input::Action::DrawBead => Some(PlayerAction::DrawBead),
                                input::Action::FanHammer => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::FanHammer,
                                    };
                                    game_state.message =
                                        "Fan the hammer — click a living enemy".to_string();
                                    None
                                }
                                input::Action::Volley => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::Volley,
                                    };
                                    game_state.message =
                                        "Volley — click a living enemy".to_string();
                                    None
                                }
                                input::Action::LeftHandDraw => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::LeftHandDraw,
                                    };
                                    game_state.message =
                                        "Left-Hand Draw — click a living enemy".to_string();
                                    None
                                }
                                input::Action::Melee => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::Melee,
                                    };
                                    game_state.message =
                                        "Melee — click an adjacent enemy".to_string();
                                    None
                                }
                                input::Action::Bandage => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::Bandage,
                                    };
                                    game_state.message =
                                        "Bandage — click an adjacent ally".to_string();
                                    None
                                }
                                input::Action::Rally => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::Rally,
                                    };
                                    game_state.message =
                                        "Rally — click an adjacent ally".to_string();
                                    None
                                }
                                input::Action::Loot => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::Loot,
                                    };
                                    game_state.message =
                                        "Loot — click an adjacent actor".to_string();
                                    None
                                }
                                input::Action::ThrowDynamite => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::ThrowDynamite,
                                    };
                                    game_state.message =
                                        "Throw dynamite — click a destination tile".to_string();
                                    None
                                }
                                input::Action::CatchDynamite => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::CatchDynamite,
                                    };
                                    game_state.message =
                                        "Catch dynamite — click the adjacent lit stick".to_string();
                                    None
                                }
                                input::Action::RethrowDynamite => {
                                    game_state.phase = InteractionPhase::Targeting {
                                        actor: actor_id,
                                        action: PlayerAction::RethrowDynamite,
                                    };
                                    game_state.message =
                                        "Rethrow dynamite — click a destination tile".to_string();
                                    None
                                }
                                input::Action::UseItem => Some(PlayerAction::UseItem),
                                input::Action::CapAndBallReload => {
                                    Some(PlayerAction::CapAndBallReload)
                                }
                                input::Action::ClearJam => Some(PlayerAction::ClearJam),
                                _ => None,
                            };
                            if let Some(player_action) = immediate {
                                if let Err(error) =
                                    combat::execute_immediate_action(&mut game_state, player_action)
                                {
                                    game_state.message = format!("Action failed: {error}");
                                }
                                if game_state.screen == GameScreen::AfterAction {
                                    if let Err(error) = campaign::prepare_ledger_writes(
                                        &mut game_state,
                                        &content_root,
                                    ) {
                                        game_state.message = error;
                                    }
                                }
                            }
                            return;
                        }
                        match kevent.physical_key {
                            // '1'-'7' → CalledShot to a hit location
                            key @ (PhysicalKey::Code(KeyCode::Digit1)
                            | PhysicalKey::Code(KeyCode::Digit2)
                            | PhysicalKey::Code(KeyCode::Digit3)
                            | PhysicalKey::Code(KeyCode::Digit4)
                            | PhysicalKey::Code(KeyCode::Digit5)
                            | PhysicalKey::Code(KeyCode::Digit6)
                            | PhysicalKey::Code(KeyCode::Digit7)) => {
                                let location = match key {
                                    PhysicalKey::Code(KeyCode::Digit1) => {
                                        pb_core::event::HitLocationType::Head
                                    }
                                    PhysicalKey::Code(KeyCode::Digit2) => {
                                        pb_core::event::HitLocationType::Eyes
                                    }
                                    PhysicalKey::Code(KeyCode::Digit3) => {
                                        pb_core::event::HitLocationType::Torso
                                    }
                                    PhysicalKey::Code(KeyCode::Digit4) => {
                                        pb_core::event::HitLocationType::Vitals
                                    }
                                    PhysicalKey::Code(KeyCode::Digit5) => {
                                        pb_core::event::HitLocationType::GunArm
                                    }
                                    PhysicalKey::Code(KeyCode::Digit6) => {
                                        pb_core::event::HitLocationType::OffArm
                                    }
                                    PhysicalKey::Code(KeyCode::Digit7) => {
                                        pb_core::event::HitLocationType::Legs
                                    }
                                    _ => unreachable!(),
                                };

                                let actor_id = match game_state.phase {
                                    InteractionPhase::SelectedActor(id) => id,
                                    _ => unreachable!(),
                                };
                                game_state.phase = InteractionPhase::Targeting {
                                    actor: actor_id,
                                    action: PlayerAction::CalledShot(location),
                                };
                                game_state.message =
                                    format!("Called shot to {:?} — click on an enemy", location);
                            }

                            _ => {}
                        }
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                game_state.mouse_x = position.x;
                game_state.mouse_y = position.y;

                // Update hovered tile for battle
                if game_state.screen == GameScreen::Battle {
                    const EDGE_SCROLL_ZONE: f64 = 16.0;
                    const EDGE_SCROLL_STEP: f32 = 12.0;
                    if position.x <= EDGE_SCROLL_ZONE {
                        game_state.camera_x -= EDGE_SCROLL_STEP;
                    } else if position.x >= f64::from(viewport_width) - EDGE_SCROLL_ZONE {
                        game_state.camera_x += EDGE_SCROLL_STEP;
                    }
                    if position.y <= EDGE_SCROLL_ZONE {
                        game_state.camera_y -= EDGE_SCROLL_STEP;
                    } else if position.y >= f64::from(viewport_height) - EDGE_SCROLL_ZONE {
                        game_state.camera_y += EDGE_SCROLL_STEP;
                    }
                    let tile = combat::screen_to_tile(
                        position.x,
                        position.y,
                        game_state.camera_x,
                        game_state.camera_y,
                        game_state.camera_zoom,
                        viewport_width,
                        viewport_height,
                    );
                    game_state.hovered_tile_x = tile.x;
                    game_state.hovered_tile_y = tile.y;
                    if let Some(preview) = combat::movement_preview(&game_state) {
                        let mut consequences = vec![format!("Move: {} AP", preview.ap_cost)];
                        if preview.leaves_cover {
                            consequences.push("LEAVES COVER [crosshatch]".to_string());
                        }
                        if preview.crosses_overwatch {
                            consequences.push("ENEMY OVERWATCH [hatch]".to_string());
                        }
                        game_state.message = consequences.join(" | ");
                    }
                }
            }

            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    },
                ..
            } => {
                game_state.mouse_down = true;
            }

            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                        ..
                    },
                ..
            } => {
                game_state.mouse_down = false;

                // Battle click handler
                if game_state.screen == GameScreen::Battle {
                    if let Err(e) = combat::handle_combat_click(&mut game_state) {
                        eprintln!("combat click error: {e}");
                    }
                    if game_state.screen == GameScreen::AfterAction {
                        if let Err(error) =
                            campaign::prepare_ledger_writes(&mut game_state, &content_root)
                        {
                            game_state.message = error;
                        }
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                if new_size.width > 0 && new_size.height > 0 {
                    viewport_width = new_size.width as f32;
                    viewport_height = new_size.height as f32;

                    surface.configure(
                        &device,
                        &wgpu::SurfaceConfiguration {
                            width: new_size.width,
                            height: new_size.height,
                            format: surface_format,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            present_mode: wgpu::PresentMode::Fifo,
                            desired_maximum_frame_latency: 2,
                            alpha_mode: wgpu::CompositeAlphaMode::Auto,
                            view_formats: vec![],
                        },
                    );
                }
            }

            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let frame = match surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let sz = window.inner_size();
                        if sz.width > 0 && sz.height > 0 {
                            surface.configure(
                                &device,
                                &wgpu::SurfaceConfiguration {
                                    width: sz.width,
                                    height: sz.height,
                                    format: surface_format,
                                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                    present_mode: wgpu::PresentMode::Fifo,
                                    desired_maximum_frame_latency: 2,
                                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                                    view_formats: vec![],
                                },
                            );
                        }
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("wgpu: out of memory, shutting down");
                        target.exit();
                        return;
                    }
                    Err(e) => {
                        eprintln!("wgpu: surface error: {e:?}");
                        return;
                    }
                };

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let size = window.inner_size();

                // Render the appropriate screen
                let sw = size.width.max(1);
                let sh = size.height.max(1);
                match game_state.screen {
                    GameScreen::Title => {
                        title_renderer.render(&render_device, &view);
                        hud_renderer.render_title_overlay(
                            &font,
                            &render_device,
                            &view,
                            sw,
                            sh,
                            &game_state.settings,
                        );
                    }
                    GameScreen::Camp => {
                        title_renderer.render(&render_device, &view);
                        let mut lines = Vec::new();
                        if let Some(campaign_state) = &game_state.campaign {
                            lines.push(format!(
                                "Ledger Weight: {}   Living company: {}/{}",
                                campaign_state.ledger_weight,
                                campaign_state
                                    .company
                                    .iter()
                                    .filter(|actor| !actor.is_dead)
                                    .count(),
                                campaign_state.company.len()
                            ));
                            match pb_content::load::load_all(&content_root) {
                                Ok(content) => {
                                    let dialogue =
                                        campaign::available_camp_dialogue(&content, campaign_state);
                                    for scene in dialogue.iter().take(4) {
                                        for line in scene.lines.iter().take(1) {
                                            lines.push(format!(
                                                "{}: {}",
                                                line.speaker_id, line.subtitle
                                            ));
                                        }
                                    }
                                    if dialogue.is_empty() {
                                        lines.push(
                                            "The fire settles. No one has more to say tonight."
                                                .to_string(),
                                        );
                                    }
                                }
                                Err(error) => lines.push(format!(
                                    "E-CONTENT-001: Camp dialogue could not be loaded: {error}"
                                )),
                            }
                        } else {
                            lines.push(
                                "E-CAMPAIGN-STATE: No company is encamped here.".to_string(),
                            );
                        }
                        lines.push(
                            "ENTER: map  L: Ledger  S: settings  ESC: title".to_string(),
                        );
                        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            "CAMP",
                            &line_refs,
                            &game_state.settings,
                        );
                    }
                    GameScreen::LedgerView => {
                        title_renderer.render(&render_device, &view);
                        let mut lines = Vec::new();
                        if let Some(campaign_state) = &game_state.campaign {
                            let act_for_date = |date: &str| match date.get(0..4) {
                                Some("1867" | "1868") => 1,
                                Some("1869" | "1870" | "1871" | "1872") => 2,
                                Some("1873" | "1874" | "1875" | "1876") => 3,
                                Some("1877" | "1878") => 4,
                                _ => 0,
                            };
                            let filtered = campaign_state
                                .ledger_entries
                                .iter()
                                .filter(|entry| {
                                    game_state
                                        .ledger_filter_act
                                        .is_none_or(|act| act_for_date(&entry.date) == act)
                                })
                                .filter(|entry| {
                                    !game_state.ledger_allies_only
                                        || entry.role == "Companion"
                                })
                                .collect::<Vec<_>>();
                            let max_scroll = filtered.len().saturating_sub(1);
                            let start = game_state.ledger_scroll.min(max_scroll);
                            for entry in filtered.iter().skip(start).take(8) {
                                lines.push(format!(
                                    "#{:03} {} — {} — {}",
                                    entry.index, entry.name, entry.place, entry.date
                                ));
                                lines.push(format!("  \"{}\"", entry.chosen_line));
                            }
                            if filtered.is_empty() {
                                lines.push(
                                    "Elias's first page is untouched. No name meets this filter."
                                        .to_string(),
                                );
                            }
                            lines.push(format!(
                                "Chain head: {}",
                                campaign_state.ledger_head_hash
                            ));
                        } else {
                            lines.push(
                                "Elias's first page is untouched. No campaign is open."
                                    .to_string(),
                            );
                            lines.push(format!("Chain head: {ZERO_LEDGER_HEAD}"));
                        }
                        lines.push(
                            "0: all acts  1-4: act  A: allies  arrows: scroll  ESC: return"
                                .to_string(),
                        );
                        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            "THE LEDGER",
                            &refs,
                            &game_state.settings,
                        );
                    }
                    GameScreen::NewCompany => {
                        title_renderer.render(&render_device, &view);
                        let mut lines = vec![
                            "Elias Ward opens the Ledger at Elk Creek.".to_string(),
                            "Choose one lasting Way; every choice carries a trade.".to_string(),
                        ];
                        match pb_content::load::load_all(&content_root) {
                            Ok(content) => {
                                for (index, way) in content.ways.values().enumerate() {
                                    let marker = if game_state.new_company_way.as_deref()
                                        == Some(way.id.as_str())
                                    {
                                        ">"
                                    } else {
                                        " "
                                    };
                                    lines.push(format!(
                                        "{marker} {}. {} | G{:+} N{:+} W{:+} H{:+} E{:+} S{:+} L{:+} | Seq{:+} Sand{}%",
                                        index + 1,
                                        way.display_name,
                                        way.stat_mods.grit,
                                        way.stat_mods.nerve,
                                        way.stat_mods.wind,
                                        way.stat_mods.hands,
                                        way.stat_mods.eyes,
                                        way.stat_mods.savvy,
                                        way.stat_mods.luck,
                                        way.effects.sequence_bonus,
                                        way.effects.sand_percent,
                                    ));
                                }
                            }
                            Err(error) => {
                                lines.push(format!("E-CAMPAIGN-CONTENT: {error}"));
                            }
                        }
                        lines.push("1-8: choose Way  ENTER: form company  ESC: return".to_string());
                        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            "NEW COMPANY",
                            &refs,
                            &game_state.settings,
                        );
                    }
                    GameScreen::MapTravel => {
                        title_renderer.render(&render_device, &view);
                        let mut lines = Vec::new();
                        if game_state.map_choices.is_empty() {
                            lines.push(
                                "The next reachable mission is marked from the campaign Ledger."
                                    .to_string(),
                            );
                            lines.push("ENTER: prepare its briefing  ESC: return to camp".to_string());
                        } else {
                            lines.push("The road divides. Choose one authored course:".to_string());
                            if let Ok(content) = pb_content::load::load_all(&content_root) {
                                for (index, choice_id) in
                                    game_state.map_choices.iter().enumerate()
                                {
                                    let date = content
                                        .campaign_nodes
                                        .get(choice_id)
                                        .map_or("date unknown", |node| node.date.as_str());
                                    lines.push(format!("{}. {} — {date}", index + 1, choice_id));
                                }
                            }
                            lines.push("1-2: record choice  ENTER: continue when resolved".to_string());
                        }
                        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            "MAP & TRAVEL",
                            &refs,
                            &game_state.settings,
                        );
                    }
                    GameScreen::Briefing => {
                        title_renderer.render(&render_device, &view);
                        let mut lines = Vec::new();
                        match game_state
                            .current_mission
                            .as_deref()
                            .ok_or_else(|| "E-CAMPAIGN-STATE: no mission selected".to_string())
                            .and_then(|mission| {
                                pb_content::load::load_all(&content_root)
                                    .map_err(|error| format!("E-CAMPAIGN-CONTENT: {error}"))
                                    .and_then(|content| {
                                        content
                                            .scenarios
                                            .get(mission)
                                            .cloned()
                                            .ok_or_else(|| {
                                                format!(
                                                    "E-CAMPAIGN-CONTENT: missing scenario {mission}"
                                                )
                                            })
                                    })
                            }) {
                            Ok(scenario) => {
                                lines.push(format!(
                                    "{} — {} | {} | {}",
                                    scenario.display_name,
                                    scenario.date,
                                    scenario.weather,
                                    scenario.light
                                ));
                                for objective in &scenario.objectives {
                                    lines.push(format!(
                                        "[{}] {}",
                                        objective.kind, objective.description
                                    ));
                                }
                                lines.push("ENTER: deploy  ESC: return to map".to_string());
                            }
                            Err(error) => lines.push(error),
                        }
                        let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            "MISSION BRIEFING",
                            &refs,
                            &game_state.settings,
                        );
                    }
                    screen @ (GameScreen::Settings | GameScreen::Bibliography) => {
                        title_renderer.render(&render_device, &view);
                        let (title, lines): (&str, &[&str]) = match screen {
                            GameScreen::Settings => (
                                "SETTINGS",
                                &[
                                    "Keyboard control, subtitles, palettes, and text scale.",
                                    "Settings persistence is under PB_CONFIG_DIR. ESC: return",
                                ],
                            ),
                            GameScreen::Bibliography => (
                                "BIBLIOGRAPHY",
                                &[
                                    "Historical sources are listed in content/BIBLIOGRAPHY.md.",
                                    "Press ESC to return to the title.",
                                ],
                            ),
                            _ => (
                                "SCREEN ERROR",
                                &["E-SCREEN-TRANSITION: invalid presentation state."],
                            ),
                        };
                        hud_renderer.render_screen_overlay(
                            &font,
                            &render_device,
                            &view,
                            (sw, sh),
                            title,
                            lines,
                            &game_state.settings,
                        );
                    }
                    GameScreen::Battle => {
                        // Always render the combat frame
                        combat::render_combat_frame(
                            &game_state,
                            &render_device,
                            &view,
                            surface_format,
                            sw,
                            sh,
                        );

                        // Render HUD (unless paused — we dim instead)
                        if !game_state.paused {
                            hud_renderer.render(&font, &game_state, &render_device, &view, sw, sh);
                        }

                        // Pause overlay (semi-transparent dim + text)
                        if game_state.paused {
                            hud_renderer.render_pause_overlay(
                                &font,
                                &render_device,
                                &view,
                                sw,
                                sh,
                                &game_state.settings,
                            );
                        }
                    }
                    GameScreen::AfterAction => {
                        after_action_renderer.render(
                            &game_state,
                            &font,
                            &render_device,
                            &view,
                            surface_format,
                            sw,
                            sh,
                        );
                    }
                    GameScreen::SaveSlot => {
                        combat::render_combat_frame(
                            &game_state,
                            &render_device,
                            &view,
                            surface_format,
                            sw,
                            sh,
                        );
                        // Dim overlay + save slot text
                        hud_renderer.render_slot_overlay(
                            &font,
                            &render_device,
                            &view,
                            sw,
                            sh,
                            "SAVE SLOT",
                            &game_state.settings,
                        );
                    }
                    GameScreen::LoadSlot => {
                        combat::render_combat_frame(
                            &game_state,
                            &render_device,
                            &view,
                            surface_format,
                            sw,
                            sh,
                        );
                        hud_renderer.render_slot_overlay(
                            &font,
                            &render_device,
                            &view,
                            sw,
                            sh,
                            "LOAD SLOT",
                            &game_state.settings,
                        );
                    }
                }

                // Print game state message as a simple HUD to stdout
                if !game_state.message.is_empty() {
                    println!("{}", game_state.message);
                    game_state.message.clear();
                }

                frame.present();
            }

            Event::AboutToWait => {
                target.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + presentation_frame_interval(game_state.settings.slow_clock),
                ));
                window.request_redraw();
            }

            _ => {}
        }
    });

    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("event loop error: {e}")),
    }
}
