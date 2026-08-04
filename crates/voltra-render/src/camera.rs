//! 2D orthographic camera and its GPU binding.

use bytemuck::{Pod, Zeroable};
use glam::camera::rh;
use glam::{Mat4, Vec2, Vec3};

/// An orthographic camera looking straight down at the XY plane.
///
/// `zoom` is a scale factor, not a distance: at `zoom == 1.0` the camera shows
/// two world units vertically, matching the clip-space range geometry used
/// before there was a camera at all. Doubling `zoom` halves what fits on
/// screen, which is the direction people expect a zoom control to move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    /// Viewport width divided by height. Keeps squares square.
    pub aspect: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            aspect: 1.0,
        }
    }
}

impl Camera2D {
    pub fn new(position: Vec2, zoom: f32, aspect: f32) -> Self {
        Self {
            position,
            zoom,
            aspect,
        }
    }

    /// Half the world-space height covered by the viewport.
    pub fn half_extents(&self) -> Vec2 {
        let half_height = 1.0 / self.zoom;
        Vec2::new(half_height * self.aspect, half_height)
    }

    /// The matrix handed to the shader.
    ///
    /// The `directx` projection and not the `opengl` or `vulkan` one: glam
    /// names these after the clip space they emit, and WebGPU shares DirectX's
    /// — Z in [0, 1] with Y up. The OpenGL variant maps Z to [-1, 1] and
    /// throws half the depth range behind the near plane; the Vulkan variant
    /// is Y-down and renders everything upside down.
    pub fn view_projection(&self) -> Mat4 {
        let half = self.half_extents();
        let projection =
            rh::proj::directx::orthographic(-half.x, half.x, -half.y, half.y, -1.0, 1.0);
        let view = Mat4::from_translation(-Vec3::new(self.position.x, self.position.y, 0.0));
        projection * view
    }
}

/// The camera matrix as the GPU sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl From<&Camera2D> for CameraUniform {
    fn from(camera: &Camera2D) -> Self {
        Self {
            view_proj: camera.view_projection().to_cols_array_2d(),
        }
    }
}

/// Uniform buffer, layout and bind group for a [`Camera2D`].
///
/// The layout is created here rather than derived from the shader so the
/// pipeline can be built before any camera exists.
pub struct CameraBinding {
    buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl CameraBinding {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniform"),
            size: size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            layout,
            bind_group,
        }
    }

    /// Writes the camera matrix to the GPU. Cheap enough to call every frame.
    pub fn upload(&self, queue: &wgpu::Queue, camera: &Camera2D) {
        let uniform = CameraUniform::from(camera);
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projecting a world point and dividing by w, as the GPU does.
    fn project(camera: &Camera2D, point: Vec2) -> Vec2 {
        let clip = camera.view_projection() * glam::Vec4::new(point.x, point.y, 0.0, 1.0);
        Vec2::new(clip.x / clip.w, clip.y / clip.w)
    }

    #[test]
    fn default_camera_is_identity_over_clip_space() {
        let camera = Camera2D::default();
        // At zoom 1 and aspect 1 the visible region is exactly clip space, so
        // geometry authored before the camera existed must not move.
        assert!((project(&camera, Vec2::new(0.0, 0.5)) - Vec2::new(0.0, 0.5)).length() < 1e-6);
        assert!((project(&camera, Vec2::new(-0.5, -0.5)) - Vec2::new(-0.5, -0.5)).length() < 1e-6);
    }

    #[test]
    fn moving_the_camera_shifts_geometry_the_other_way() {
        let camera = Camera2D::new(Vec2::new(1.0, 0.0), 1.0, 1.0);
        // The camera moved right, so the origin must appear to the left.
        let origin = project(&camera, Vec2::ZERO);
        assert!(
            origin.x < -0.9,
            "expected origin off to the left, got {origin}"
        );
    }

    #[test]
    fn zoom_shrinks_the_visible_region() {
        let camera = Camera2D::new(Vec2::ZERO, 2.0, 1.0);
        assert_eq!(camera.half_extents(), Vec2::new(0.5, 0.5));
        // A point at the old edge is now past the edge of the screen.
        assert!(project(&camera, Vec2::new(0.0, 1.0)).y > 1.0);
    }

    #[test]
    fn aspect_widens_the_visible_region_horizontally() {
        let camera = Camera2D::new(Vec2::ZERO, 1.0, 2.0);
        assert_eq!(camera.half_extents(), Vec2::new(2.0, 1.0));
        // Twice as wide means x is compressed by half in clip space.
        let p = project(&camera, Vec2::new(1.0, 0.0));
        assert!((p.x - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn uniform_is_the_matrix_in_column_major_order() {
        let camera = Camera2D::new(Vec2::new(3.0, 4.0), 1.5, 1.25);
        let uniform = CameraUniform::from(&camera);
        assert_eq!(
            uniform.view_proj,
            camera.view_projection().to_cols_array_2d()
        );
        assert_eq!(size_of::<CameraUniform>(), 16 * size_of::<f32>());
    }
}
