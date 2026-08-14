//! Title screen rendering.
//!
//! Uses project-generated key art uploaded once at startup.

use std::sync::Arc;

use pb_render::backdrop::BackdropSystem;
use pb_render::device::RenderDevice;

/// One pointer-selectable menu command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuButton {
    pub label: String,
    pub selected: bool,
    pub enabled: bool,
}

impl MenuButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            enabled: true,
        }
    }

    pub fn selected(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected: true,
            enabled: true,
        }
    }

    pub fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            enabled: false,
        }
    }
}

/// Pixel-space hit target shared by rendering and pointer input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuButtonBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl MenuButtonBounds {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x <= f64::from(self.x + self.width)
            && y >= f64::from(self.y)
            && y <= f64::from(self.y + self.height)
    }
}

/// Lay menu commands out as a centered one- or two-column button grid.
pub fn button_layout(
    button_count: usize,
    screen_size: (u32, u32),
    text_scale_percent: u32,
) -> Vec<MenuButtonBounds> {
    if button_count == 0 {
        return Vec::new();
    }
    let width = screen_size.0.max(1) as f32;
    let height = screen_size.1.max(1) as f32;
    let columns = if button_count > 6 { 2 } else { 1 };
    let rows = button_count.div_ceil(columns);
    let margin = (width * 0.06).clamp(24.0, 72.0);
    let column_gap = (width * 0.018).clamp(12.0, 24.0);
    let scale = text_scale_percent as f32 / 100.0;
    let button_height = (42.0 * scale).clamp(40.0, 62.0);
    let row_gap = (10.0 * scale).clamp(8.0, 14.0);
    let button_width = if columns == 1 {
        (width * 0.40).clamp(280.0, 520.0).min(width - margin * 2.0)
    } else {
        ((width - margin * 2.0 - column_gap) / 2.0)
            .clamp(220.0, 430.0)
            .min((width - column_gap) / 2.0)
    };
    let grid_width = button_width * columns as f32 + column_gap * (columns - 1) as f32;
    let grid_height = button_height * rows as f32 + row_gap * rows.saturating_sub(1) as f32;
    let left = ((width - grid_width) * 0.5).max(8.0);
    let latest_top = (height - margin - grid_height).max(margin);
    let top = (height * 0.52).min(latest_top).max(margin);

    (0..button_count)
        .map(|index| {
            let column = index % columns;
            let row = index / columns;
            MenuButtonBounds {
                x: left + column as f32 * (button_width + column_gap),
                y: top + row as f32 * (button_height + row_gap),
                width: button_width,
                height: button_height,
            }
        })
        .collect()
}

pub fn button_at(
    buttons: &[MenuButton],
    screen_size: (u32, u32),
    text_scale_percent: u32,
    pointer: (f64, f64),
) -> Option<usize> {
    button_layout(buttons.len(), screen_size, text_scale_percent)
        .into_iter()
        .enumerate()
        .find_map(|(index, bounds)| {
            (buttons[index].enabled && bounds.contains(pointer.0, pointer.1)).then_some(index)
        })
}

#[allow(missing_debug_implementations)]
pub struct TitleRenderer {
    backdrop: BackdropSystem,
}

impl TitleRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        Self::new_with_png(
            device,
            queue,
            format,
            include_bytes!("../../../assets/art/title_backdrop.png"),
        )
    }

    /// Create a cinematic backdrop renderer from an embedded PNG.
    pub fn new_with_png(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        png_bytes: &[u8],
    ) -> Result<Self, String> {
        let backdrop = BackdropSystem::from_png_bytes(device, queue, format, png_bytes)?;
        Ok(Self { backdrop })
    }

    pub fn render(&self, render_device: &Arc<RenderDevice>, view: &wgpu::TextureView) {
        self.backdrop
            .render(&render_device.device, &render_device.queue, view);
    }
}

#[cfg(test)]
mod tests {
    use super::{button_at, button_layout, MenuButton};

    #[test]
    fn menu_buttons_are_centered_and_stay_inside_the_window() {
        let bounds = button_layout(8, (800, 600), 200);
        assert_eq!(bounds.len(), 8);
        for button in &bounds {
            assert!(button.x >= 0.0);
            assert!(button.y >= 0.0);
            assert!(button.x + button.width <= 800.0);
            assert!(button.y + button.height <= 600.0);
        }
        assert!(bounds[0].x < 400.0);
        assert!(bounds[1].x > 400.0);
    }

    #[test]
    fn hit_testing_uses_the_same_layout_and_ignores_disabled_buttons() {
        let buttons = vec![MenuButton::new("Play"), MenuButton::disabled("Unavailable")];
        let bounds = button_layout(buttons.len(), (1280, 720), 100);
        let center = |index: usize| {
            (
                f64::from(bounds[index].x + bounds[index].width * 0.5),
                f64::from(bounds[index].y + bounds[index].height * 0.5),
            )
        };
        assert_eq!(button_at(&buttons, (1280, 720), 100, center(0)), Some(0));
        assert_eq!(button_at(&buttons, (1280, 720), 100, center(1)), None);
    }
}
