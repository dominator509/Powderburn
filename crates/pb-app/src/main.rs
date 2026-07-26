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

mod combat;
mod menu;
mod state;

use std::path::Path;
use std::sync::Arc;

use pb_render::device::RenderDevice;
use state::{GameScreen, GameState, InteractionPhase, PlayerAction};
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

fn main() -> Result<(), String> {
    // ── Event loop & window (Arc for 'static surface) ─────────────────
    let event_loop = EventLoop::new().map_err(|e| format!("event loop creation failed: {e}"))?;
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("POWDERBURN: The Ledger of Elk Creek")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0)),
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

    // ── Game state ────────────────────────────────────────────────────
    let mut game_state = GameState::new();
    println!("POWDERBURN — Press ENTER to begin");

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
                event:
                    WindowEvent::KeyboardInput {
                        event: kevent, ..
                    },
                ..
            } => {
                if kevent.state != ElementState::Pressed {
                    return;
                }

                // Escape always exits
                if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                    target.exit();
                    return;
                }

                // ── Title screen: Enter → init combat ────────────────
                if game_state.screen == GameScreen::Title {
                    if matches!(kevent.physical_key, PhysicalKey::Code(KeyCode::Enter)) {
                        game_state.message = "Initializing combat...".to_string();
                        println!("{}", game_state.message);
                        match combat::init_combat(&mut game_state, &content_root) {
                            Ok(()) => {
                                game_state.screen = GameScreen::Combat;
                                game_state.message =
                                    "Combat started — click an ally to select them".to_string();
                                println!("{}", game_state.message);
                            }
                            Err(e) => {
                                game_state.message = format!("Combat init failed: {e}");
                                eprintln!("{}", game_state.message);
                            }
                        }
                    }
                    return;
                }

                // ── Combat keyboard actions ──────────────────────────
                if game_state.screen == GameScreen::Combat {
                    // These keys only work when an actor is selected
                    if let InteractionPhase::SelectedActor(_) = game_state.phase {
                        match kevent.physical_key {
                            // 'f' → SnapShot (fire quick)
                            PhysicalKey::Code(KeyCode::KeyF) => {
                                let actor_id = match game_state.phase {
                                    InteractionPhase::SelectedActor(id) => id,
                                    _ => unreachable!(),
                                };
                                game_state.phase = InteractionPhase::Targeting {
                                    actor: actor_id,
                                    action: PlayerAction::SnapShot,
                                };
                                game_state.message =
                                    "Choose target — click on an enemy".to_string();
                            }

                            // 'a' → AimedShot
                            PhysicalKey::Code(KeyCode::KeyA) => {
                                let actor_id = match game_state.phase {
                                    InteractionPhase::SelectedActor(id) => id,
                                    _ => unreachable!(),
                                };
                                game_state.phase = InteractionPhase::Targeting {
                                    actor: actor_id,
                                    action: PlayerAction::AimedShot,
                                };
                                game_state.message =
                                    "Aimed shot — click on an enemy".to_string();
                            }

                            // 'h' → Hold (end turn, keep remaining AP)
                            PhysicalKey::Code(KeyCode::KeyH) => {
                                if let Err(e) = combat::execute_immediate_action(
                                    &mut game_state,
                                    PlayerAction::Hold,
                                ) {
                                    game_state.message = format!("Hold failed: {e}");
                                }
                            }

                            // 'r' → Reload
                            PhysicalKey::Code(KeyCode::KeyR) => {
                                if let Err(e) = combat::execute_immediate_action(
                                    &mut game_state,
                                    PlayerAction::Reload,
                                ) {
                                    game_state.message = format!("Reload failed: {e}");
                                }
                            }

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
                                game_state.message = format!(
                                    "Called shot to {:?} — click on an enemy",
                                    location
                                );
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

                // Update hovered tile for combat
                if game_state.screen == GameScreen::Combat {
                    let tile = combat::screen_to_tile(
                        position.x,
                        position.y,
                        game_state.camera_x,
                        game_state.camera_y,
                        viewport_width,
                        viewport_height,
                    );
                    game_state.hovered_tile_x = tile.x;
                    game_state.hovered_tile_y = tile.y;
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

                // Combat click handler
                if game_state.screen == GameScreen::Combat {
                    if let Err(e) = combat::handle_combat_click(&mut game_state) {
                        eprintln!("combat click error: {e}");
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
                match game_state.screen {
                    GameScreen::Title => {
                        menu::render_title(
                            &render_device,
                            &view,
                            surface_format,
                            size.width.max(1),
                            size.height.max(1),
                        );
                    }
                    GameScreen::Combat | GameScreen::AfterAction => {
                        combat::render_combat_frame(
                            &game_state,
                            &render_device,
                            &view,
                            surface_format,
                            size.width.max(1),
                            size.height.max(1),
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
