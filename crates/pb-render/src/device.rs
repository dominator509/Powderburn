//! wgpu device and adapter management for headless rendering.
//!
//! On headless systems (no display), we use the Vulkan backend with
//! VK_EXT_headless_surface, falling back to any available adapter.
//! On systems with a display, we prefer a real GPU adapter.

use std::sync::Arc;

/// A wgpu device, queue, and adapter triplet.
#[allow(missing_debug_implementations)]
pub struct RenderDevice {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub headless: bool,
}

impl RenderDevice {
    /// Create a new device for headless rendering.
    ///
    /// Uses Vulkan or GL backend with headless surface support.
    pub async fn new_headless() -> Result<Arc<Self>, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::empty(),
            backend_options: wgpu::BackendOptions::default(),
        });

        // List adapters
        let adapters: Vec<wgpu::Adapter> =
            instance.enumerate_adapters(wgpu::Backends::VULKAN | wgpu::Backends::GL);

        // Pick a Vulkan adapter first (usually llvmpipe on headless), fall back to any
        let adapter = adapters
            .into_iter()
            .find(|a| {
                let info = a.get_info();
                info.backend == wgpu::Backend::Vulkan
            })
            .or_else(|| {
                instance
                    .enumerate_adapters(wgpu::Backends::all())
                    .into_iter()
                    .next()
            })
            .ok_or_else(|| "no wgpu adapter available".to_string())?;

        let adapter_info = adapter.get_info();
        eprintln!(
            "render: adapter = {} ({:?})",
            adapter_info.name, adapter_info.backend
        );

        let device_descriptor = wgpu::DeviceDescriptor {
            label: Some("powderburn device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
        };

        let (device, queue) = adapter
            .request_device(&device_descriptor, None)
            .await
            .map_err(|e| format!("device request failed: {}", e))?;

        Ok(Arc::new(Self {
            adapter,
            device,
            queue,
            headless: true,
        }))
    }

    /// Get the default limits for this device.
    pub fn limits(&self) -> wgpu::Limits {
        self.device.limits()
    }
}
