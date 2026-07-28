//! Camera system for the isometric viewport.
//!
//! Manages pan, zoom, and the projection from world space to screen space.

/// Camera configuration for the isometric battlefield view.
#[allow(missing_debug_implementations)]
pub struct IsoCamera {
    /// World-space center point (isometric x, isometric y)
    pub center_x: f32,
    pub center_y: f32,
    /// Zoom level: 1.0 = default, >1 = zoomed in, <1 = zoomed out
    pub zoom: f32,
    /// Viewport dimensions in pixels
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Tile footprint in pixels (width, height) at zoom 1.0
    pub tile_w: f32,
    pub tile_h: f32,
}

impl IsoCamera {
    /// Create a default camera centered on the battlefield origin.
    pub fn new(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
            viewport_width: viewport_width as f32,
            viewport_height: viewport_height as f32,
            tile_w: 64.0,
            tile_h: 32.0,
        }
    }

    /// Project a world-space isometric tile coordinate to screen-space pixel.
    ///
    /// Isometric projection: screen_x = (x - y) * tile_w/2
    ///                      screen_y = (x + y) * tile_h/2
    pub fn world_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        let half_w = self.tile_w * 0.5 * self.zoom;
        let half_h = self.tile_h * 0.5 * self.zoom;

        let sx = (wx - wy) * half_w + self.viewport_width * 0.5 - self.center_x * self.zoom;
        let sy = (wx + wy) * half_h + self.viewport_height * 0.5 - self.center_y * self.zoom;
        (sx, sy)
    }

    /// Get the orthographic projection matrix as 16 f32s (column-major).
    pub fn ortho_matrix(&self) -> [f32; 16] {
        let half_width = self.viewport_width * 0.5 / self.zoom;
        let half_height = self.viewport_height * 0.5 / self.zoom;
        let left = self.center_x - half_width;
        let right = self.center_x + half_width;
        let bottom = self.center_y - half_height;
        let top = self.center_y + half_height;
        let near = -1000.0;
        let far = 1000.0;

        let rcp_w = 1.0 / (right - left);
        let rcp_h = 1.0 / (top - bottom);
        // WebGPU's clip-space depth is 0..1, not OpenGL's -1..1.
        let rcp_d = 1.0 / (near - far);

        [
            2.0 * rcp_w,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 * rcp_h,
            0.0,
            0.0,
            0.0,
            0.0,
            rcp_d,
            0.0,
            -(right + left) * rcp_w,
            -(top + bottom) * rcp_h,
            near * rcp_d,
            1.0,
        ]
    }

    /// Serialize the ortho matrix as bytes for the uniform buffer.
    pub fn ortho_matrix_bytes(&self) -> [u8; 64] {
        let m = self.ortho_matrix();
        let mut bytes = [0u8; 64];
        for (i, &val) in m.iter().enumerate() {
            let f_bytes = val.to_le_bytes();
            bytes[i * 4..(i + 1) * 4].copy_from_slice(&f_bytes);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::IsoCamera;

    #[test]
    fn ortho_depth_keeps_all_render_layers_in_webgpu_clip_range() {
        let matrix = IsoCamera::new(1920, 1080).ortho_matrix();
        for z in [0.0_f32, 5.0, 10.0, 120.0] {
            let clip_z = matrix[10] * z + matrix[14];
            assert!((0.0..=1.0).contains(&clip_z), "z={z} clipped at {clip_z}");
        }
    }

    #[test]
    fn ortho_matrix_applies_camera_center() {
        let mut camera = IsoCamera::new(1920, 1080);
        camera.center_x = 128.0;
        camera.center_y = 240.0;
        let matrix = camera.ortho_matrix();
        let clip_x = matrix[0] * camera.center_x + matrix[12];
        let clip_y = matrix[5] * camera.center_y + matrix[13];
        assert!(clip_x.abs() < 0.0001);
        assert!(clip_y.abs() < 0.0001);
    }
}
