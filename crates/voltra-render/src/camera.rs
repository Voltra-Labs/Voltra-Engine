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
    /// Private because `half_extents` divides by it. Write through
    /// [`Self::set_zoom`], which clamps.
    zoom: f32,
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
        let mut camera = Self {
            position,
            zoom: 1.0,
            aspect,
        };
        camera.set_zoom(zoom);
        camera
    }

    /// Current zoom. Always within [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Half the world-space height covered by the viewport.
    pub fn half_extents(&self) -> Vec2 {
        let half_height = 1.0 / self.zoom;
        Vec2::new(half_height * self.aspect, half_height)
    }

    /// Floor and ceiling for [`Self::set_zoom`].
    ///
    /// Unbounded zoom is not a theoretical problem: `half_extents` divides by
    /// `zoom`, so zero produces infinities and a large enough value produces
    /// denormals. Godot clamps its editor zoom for the same reason.
    pub const MIN_ZOOM: f32 = 1.0 / 128.0;
    pub const MAX_ZOOM: f32 = 128.0;

    /// The world point under a viewport pixel.
    ///
    /// `point` is viewport-local and **Y-down**, which is what every UI toolkit
    /// reports; world space is Y-up, hence the flip on that axis alone.
    ///
    /// Assumes `self.aspect` matches `viewport`'s. `App` keeps it so; if it
    /// ever did not, the image on screen would already be stretched.
    pub fn viewport_to_world(&self, point: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        // Normalised to 0..1 across the viewport, then to -1..1 about its
        // centre, then scaled by what the camera covers.
        let normalised = point / viewport;
        let centred = Vec2::new(normalised.x * 2.0 - 1.0, 1.0 - normalised.y * 2.0);
        self.position + centred * half
    }

    /// Inverse of [`Self::viewport_to_world`].
    pub fn world_to_viewport(&self, world: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        let centred = (world - self.position) / half;
        let normalised = Vec2::new(centred.x * 0.5 + 0.5, 0.5 - centred.y * 0.5);
        normalised * viewport
    }

    /// Sets `zoom`, clamped to [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    ///
    /// `NaN` leaves the previous value alone: it compares false against every
    /// bound, so `clamp` would panic and a bare assignment would poison the
    /// projection matrix for every later frame.
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_nan() {
            return;
        }
        self.zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    /// Scales the zoom by `factor`, keeping the world point under `anchor`
    /// under `anchor` afterwards.
    ///
    /// Measuring the same anchor before and after is what makes this correct at
    /// the clamp too: if `set_zoom` refused the change, the two samples are
    /// equal and the camera does not move.
    pub fn zoom_around(&mut self, anchor: Vec2, viewport: Vec2, factor: f32) {
        let before = self.viewport_to_world(anchor, viewport);
        self.set_zoom(self.zoom * factor);
        let after = self.viewport_to_world(anchor, viewport);
        self.position += before - after;
    }

    /// A viewport no smaller than one point per axis.
    ///
    /// A panel dragged shut reports zero, and both conversions divide by this.
    fn guard(viewport: Vec2) -> Vec2 {
        viewport.max(Vec2::ONE)
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

    /// A viewport that is not square and not a round number, so an axis mix-up
    /// cannot pass by coincidence.
    fn viewport() -> Vec2 {
        Vec2::new(800.0, 600.0)
    }

    fn camera_for(viewport: Vec2) -> Camera2D {
        Camera2D::new(Vec2::new(3.0, -2.0), 1.5, viewport.x / viewport.y)
    }

    #[test]
    fn viewport_centre_is_the_camera_position() {
        let viewport = viewport();
        let camera = camera_for(viewport);
        let world = camera.viewport_to_world(viewport * 0.5, viewport);
        assert!(
            (world - camera.position).length() < 1e-5,
            "centre mapped to {world}, expected {}",
            camera.position
        );
    }

    #[test]
    fn viewport_top_left_is_up_and_to_the_left() {
        let viewport = viewport();
        let camera = camera_for(viewport);
        let half = camera.half_extents();
        // Top-left in a Y-down viewport is minimum x and *maximum* y in world
        // space. Getting this flip wrong is the classic silent bug.
        let expected = camera.position + Vec2::new(-half.x, half.y);
        let world = camera.viewport_to_world(Vec2::ZERO, viewport);
        assert!(
            (world - expected).length() < 1e-5,
            "got {world}, want {expected}"
        );
    }

    #[test]
    fn viewport_and_world_round_trip() {
        let viewport = viewport();
        for zoom in [Camera2D::MIN_ZOOM, 0.25, 1.0, 7.5, Camera2D::MAX_ZOOM] {
            let camera = Camera2D::new(Vec2::new(-4.0, 9.0), zoom, viewport.x / viewport.y);
            for point in [
                Vec2::ZERO,
                viewport,
                viewport * 0.5,
                Vec2::new(123.0, 456.0),
            ] {
                let back =
                    camera.world_to_viewport(camera.viewport_to_world(point, viewport), viewport);
                // A twentieth of a point, not a thousandth. `world_to_viewport`
                // subtracts two world coordinates of magnitude ~9 to get a
                // difference of ~0.008 at MAX_ZOOM, then divides by an equally
                // small half-extent. f32 carries about seven digits, so that
                // cancellation costs real precision — the observed error at the
                // ceiling is ~0.006 points. This is inherent to f32 world
                // coordinates, not to the algorithm, and a twentieth of a pixel
                // is far below anything visible.
                assert!(
                    (back - point).length() < 0.05,
                    "zoom {zoom}: {point} round-tripped to {back}"
                );
            }
        }
    }

    #[test]
    fn zoom_around_keeps_the_anchor_world_point_still() {
        let viewport = viewport();
        let mut camera = camera_for(viewport);
        // Off-centre on both axes: a centred zoom would pass a centred anchor.
        let anchor = Vec2::new(150.0, 500.0);
        let before = camera.viewport_to_world(anchor, viewport);

        camera.zoom_around(anchor, viewport, 1.7);

        let after = camera.viewport_to_world(anchor, viewport);
        assert!(
            (after - before).length() < 1e-4,
            "anchor drifted from {before} to {after}"
        );
        assert!(
            camera.zoom() > 1.5,
            "zoom did not increase: {}",
            camera.zoom()
        );
    }

    #[test]
    fn set_zoom_clamps_both_ends() {
        let mut camera = Camera2D::default();
        camera.set_zoom(f32::INFINITY);
        assert_eq!(camera.zoom(), Camera2D::MAX_ZOOM);
        camera.set_zoom(0.0);
        assert_eq!(camera.zoom(), Camera2D::MIN_ZOOM);
        camera.set_zoom(-5.0);
        assert_eq!(camera.zoom(), Camera2D::MIN_ZOOM);
    }

    #[test]
    fn set_zoom_ignores_nan() {
        let mut camera = Camera2D::default();
        camera.set_zoom(2.0);
        camera.set_zoom(f32::NAN);
        assert_eq!(camera.zoom(), 2.0, "NaN must leave the previous zoom alone");
    }

    #[test]
    fn zoom_around_at_the_clamp_does_not_drift() {
        let viewport = viewport();
        let mut camera = Camera2D::new(Vec2::ZERO, Camera2D::MAX_ZOOM, viewport.x / viewport.y);
        let anchor = Vec2::new(10.0, 20.0);

        camera.zoom_around(anchor, viewport, 2.0);

        // Zoom was already at the ceiling, so nothing may move. Without this
        // the camera slides sideways every notch once the user hits the limit.
        assert_eq!(camera.zoom(), Camera2D::MAX_ZOOM);
        assert!(
            camera.position.length() < 1e-6,
            "drifted to {}",
            camera.position
        );
    }

    #[test]
    fn new_clamps_its_zoom() {
        // The constructor is a second door onto the same invariant. Leaving it
        // unclamped means `Camera2D::new(pos, 0.0, 1.0)` produces infinities in
        // half_extents with no warning anywhere.
        assert_eq!(
            Camera2D::new(Vec2::ZERO, 0.0, 1.0).zoom(),
            Camera2D::MIN_ZOOM
        );
        assert_eq!(
            Camera2D::new(Vec2::ZERO, 1e9, 1.0).zoom(),
            Camera2D::MAX_ZOOM
        );
    }

    #[test]
    fn a_degenerate_viewport_stays_finite() {
        let camera = Camera2D::default();
        let world = camera.viewport_to_world(Vec2::ZERO, Vec2::ZERO);
        assert!(world.is_finite(), "got {world}");
    }
}
