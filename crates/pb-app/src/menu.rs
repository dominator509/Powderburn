//! Title screen rendering.
//!
//! Uses project-generated key art uploaded once at startup.

use std::sync::Arc;

use pb_render::backdrop::BackdropSystem;
use pb_render::device::RenderDevice;

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
        let backdrop = BackdropSystem::from_png_bytes(
            device,
            queue,
            format,
            include_bytes!("../../../assets/art/title_backdrop.png"),
        )?;
        Ok(Self { backdrop })
    }

    pub fn render(&self, render_device: &Arc<RenderDevice>, view: &wgpu::TextureView) {
        self.backdrop
            .render(&render_device.device, &render_device.queue, view);
    }
}
