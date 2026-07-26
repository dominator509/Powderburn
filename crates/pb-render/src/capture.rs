//! Headless frame capture.
//!
//! Renders a frame to an offscreen texture and writes it as a PNG file.
//! This is the basis for LF-08 and for all deterministic visual proofs.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;

use super::device::RenderDevice;
use crate::{CaptureMeta, RenderConfig};

/// Render a single frame headlessly and capture it to a PNG file.
///
/// For M1, this simply clears the frame to a solid color and writes it.
/// Later milestones add the actual isometric scene rendering.
pub async fn capture_frame(
    device: Arc<RenderDevice>,
    config: &RenderConfig,
    output_path: &Path,
) -> Result<CaptureMeta, String> {
    // Create the offscreen texture
    let texture_size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };

    let texture = device.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capture target"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Create a buffer to read the texture back
    let buffer_size = (config.width * config.height * 4) as u64;
    let buffer = device.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Render: clear to a dark green-blue (frontier sky)
    let mut encoder = device
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture encoder"),
        });

    {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.15,
                        g: 0.25,
                        b: 0.20,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        // No draw calls yet - M1 is just a clear
    }

    // Copy texture to buffer
    let block_size = 4; // RGBA8 = 4 bytes per pixel
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(config.width * block_size),
                rows_per_image: Some(config.height),
            },
        },
        texture_size,
    );

    device.queue.submit(std::iter::once(encoder.finish()));

    // Map the buffer and read pixels
    let buffer_slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.device.poll(wgpu::Maintain::Wait);

    let result = rx
        .recv()
        .map_err(|_| "channel receive failed".to_string())?;
    result.map_err(|e| format!("buffer map failed: {}", e))?;

    let data = buffer_slice.get_mapped_range();
    let pixels: Vec<u8> = data.to_vec();
    drop(data);
    buffer.unmap();

    // Write PNG using the image crate
    image::save_buffer(
        output_path,
        &pixels,
        config.width,
        config.height,
        image::ColorType::Rgba8,
    )
    .map_err(|e| format!("failed to write PNG: {}", e))?;

    // Compute SHA256 of the file
    let checksum = file_sha256(output_path).unwrap_or_else(|| "unknown".to_string());

    Ok(CaptureMeta {
        width: config.width,
        height: config.height,
        checksum,
    })
}

fn file_sha256(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Some(format!("{:x}", hash))
}
