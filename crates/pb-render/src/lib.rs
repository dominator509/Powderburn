//! See ARCHITECTURE.md for this crate's place in the import law.
//! wgpu-based isometric renderer with headless capture support.
//! This crate uses floats freely because presentation is not simulation state.
#![forbid(unsafe_code)]

pub mod device;
pub mod capture;

/// A color with 8-bit channels.
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_wgpu(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64 / 255.0,
            g: self.g as f64 / 255.0,
            b: self.b as f64 / 255.0,
            a: self.a as f64 / 255.0,
        }
    }
}

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub adapter_name: Option<String>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            adapter_name: None,
        }
    }
}

/// Metadata about a captured frame.
#[derive(Debug, Clone)]
pub struct CaptureMeta {
    pub width: u32,
    pub height: u32,
    pub checksum: String,
}
