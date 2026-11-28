//! One drawn copy of a mesh, as the GPU reads it: its placement, tint and
//! the lights that reach it.

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Mat3};

use super::lighting::LightList3D;
use super::winding::Winding;

/// Per-instance data of an instanced 3D draw: 124 bytes, eight attributes.
///
/// The model matrix as an affine `3×vec4` (rows: the 3×3 part and the
/// translation in `w`), its normal matrix — the cofactor matrix, which
/// keeps normals at right angles to a surface under non-uniform scale and
/// is defined for a flattened model too — as another `3×vec4`, a linear
/// RGBA tint, and its [`LightList3D`] as one `Uint32x3`: eight `u8`
/// point and spot light indices in two words, the directional lights'
/// bits in the third.
///
/// **The WebGL2 attribute budget** (16 attributes, a 255-byte stride per
/// buffer) is laid out for everything 0.5 adds: [`Vertex3D`]'s 4, these
/// 8 and skinning's two (155) — 14 of 16. Pinned by a test, so a field
/// added here that breaks the floor fails at once rather than on a
/// browser.
///
/// [`Vertex3D`]: super::Vertex3D
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Instance3D {
    /// The model matrix's rows: `x`, `y`, `z` and the translation in `w`.
    pub model: [[f32; 4]; 3],
    /// The normal matrix's rows. The first row's `w` is the sign the
    /// bitangent is turned by — −1 mirrored, 1 otherwise — the rest's
    /// unused.
    pub normal: [[f32; 4]; 3],
    /// Multiplies what is drawn; linear, alpha last.
    pub tint: [f32; 4],
    /// [`LightList3D::packed`]: which of the frame's lights reach it.
    pub lights: [u32; 3],
}

impl Instance3D {
    /// The shader locations, 4 to 11: after [`Vertex3D`](super::Vertex3D)'s.
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4,
        10 => Float32x4,
        11 => Uint32x3,
    ];

    /// The instance buffer's layout: buffer slot 1, stepped per instance.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// The instance of a mesh placed by `model` and tinted `tint`, lit by
    /// every directional light and no point or spot light
    /// ([`LightList3D::default`]) until [`set_lights`](Self::set_lights).
    ///
    /// Refused only when nothing can be drawn from it: a model that is not
    /// finite, or whose normal matrix overflows ([`InstanceRefused::NotFinite`]),
    /// or one that collapses space to a line or a point
    /// ([`InstanceRefused::Degenerate`], rank below 2) — it covers no pixel.
    /// A model flattened to a plane (`builtin:plane` at scale `(10, 0, 10)`,
    /// a cube with no height) still covers pixels and is drawn, as is a
    /// tiny one.
    ///
    /// The normal matrix is the cofactor matrix — the inverse transpose
    /// times the determinant, defined for a singular model too — signed so
    /// that a mirrored model's normals still point out. It is not unit
    /// length: the surface shader normalises after multiplying.
    pub fn new(model: Affine3A, tint: [f32; 4]) -> Result<Self, InstanceRefused> {
        if !model.is_finite() {
            return Err(InstanceRefused::NotFinite);
        }
        let linear = Mat3::from(model.matrix3);
        let (a, b, c) = (linear.x_axis, linear.y_axis, linear.z_axis);
        let cofactor = Mat3::from_cols(b.cross(c), c.cross(a), a.cross(b));
        if !cofactor.is_finite() {
            return Err(InstanceRefused::NotFinite);
        }
        if cofactor == Mat3::ZERO {
            return Err(InstanceRefused::Degenerate);
        }
        // Mirrored: the cofactor turned round so normals point out, and the
        // bitangent turned with it (Bevy's `sign_determinant_model_3x3m`).
        // A flat model's determinant is noise: both faces are drawn, and
        // the shader turns the normal to the side it is seen from.
        let (normal, sign) = if Self::winding(&model) == Winding::Clockwise {
            (-cofactor, -1.0)
        } else {
            (cofactor, 1.0)
        };
        let t = model.translation;
        let rows = |m: Mat3, w: [f32; 3]| {
            [0, 1, 2].map(|i| {
                let row = m.row(i);
                [row.x, row.y, row.z, w[i]]
            })
        };
        Ok(Self {
            model: rows(linear, [t.x, t.y, t.z]),
            normal: rows(normal, [sign, 0.0, 0.0]),
            tint,
            lights: LightList3D::default().packed(),
        })
    }

    /// Lights it by `lights`: which of the frame's lights reach it.
    pub fn set_lights(&mut self, lights: &LightList3D) {
        self.lights = lights.packed();
    }

    /// Which way `model` turns a mesh's triangles on screen, and so which
    /// pipeline variant draws it: see [`Winding::of`].
    pub fn winding(model: &Affine3A) -> Winding {
        Winding::of(model)
    }
}

/// Why [`Instance3D::new`] drew nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRefused {
    /// The model, or its normal matrix, is not finite: a broken transform,
    /// worth saying so once.
    NotFinite,
    /// The model collapses space to a line or a point: it covers no pixel.
    Degenerate,
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;
    use crate::render3d::Vertex3D;

    /// What WebGL2 guarantees (`Limits::downlevel_webgl2_defaults`).
    const WEBGL2_ATTRIBUTES: usize = 16;
    const WEBGL2_STRIDE: u64 = 255;
    /// Attributes later stages add to the same draws: the skin's joints
    /// and weights (155).
    const LATER: usize = 2;

    #[test]
    fn the_webgl2_attribute_budget_holds() {
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(limits.max_vertex_attributes as usize, WEBGL2_ATTRIBUTES);
        assert_eq!(
            u64::from(limits.max_vertex_buffer_array_stride),
            WEBGL2_STRIDE
        );
        let now = Vertex3D::ATTRIBUTES.len() + Instance3D::ATTRIBUTES.len();
        assert_eq!(now, 12);
        assert_eq!(now + LATER, 14, "the budget ARCHITECTURE records");
        assert!(now + LATER <= WEBGL2_ATTRIBUTES);
        for layout in [Vertex3D::layout(), Instance3D::layout()] {
            assert!(
                layout.array_stride <= WEBGL2_STRIDE,
                "{}",
                layout.array_stride
            );
        }
        assert_eq!(Vertex3D::layout().array_stride, 48);
        assert_eq!(Instance3D::layout().array_stride, 124);
        assert_eq!(
            Instance3D::ATTRIBUTES[7].format,
            wgpu::VertexFormat::Uint32x3,
            "the light list"
        );
        // Locations are unique across the two buffers, as one draw binds both.
        let mut locations: Vec<u32> = Vertex3D::ATTRIBUTES
            .iter()
            .chain(&Instance3D::ATTRIBUTES)
            .map(|a| a.shader_location)
            .collect();
        locations.sort_unstable();
        locations.dedup();
        assert_eq!(locations.len(), now);
        assert!(locations.iter().all(|&l| (l as usize) < WEBGL2_ATTRIBUTES));
    }

    #[test]
    fn the_rows_rebuild_the_model_and_the_normal_stays_perpendicular() {
        let model = Affine3A::from_scale_rotation_translation(
            Vec3::new(4.0, 1.0, 1.0),
            Quat::from_rotation_z(0.6),
            Vec3::new(1.0, -2.0, 3.0),
        );
        let instance = Instance3D::new(model, [1.0; 4]).expect("invertible");
        let p = Vec3::new(0.3, -0.7, 2.0);
        let rows = instance.model.map(glam::Vec4::from);
        let moved = Vec3::new(
            rows[0].dot(p.extend(1.0)),
            rows[1].dot(p.extend(1.0)),
            rows[2].dot(p.extend(1.0)),
        );
        assert!((moved - model.transform_point3(p)).length() < 1e-5);
        // A surface along (1, 1, 0) with normal (1, −1, 0), stretched.
        let along = model.transform_vector3(Vec3::new(1.0, 1.0, 0.0));
        let n = instance.normal.map(|r| glam::Vec4::from(r).truncate());
        let normal = Vec3::new(1.0, -1.0, 0.0);
        let normal = Vec3::new(n[0].dot(normal), n[1].dot(normal), n[2].dot(normal));
        assert!(along.dot(normal).abs() < 1e-5);
    }

    #[test]
    fn a_flat_or_tiny_model_is_drawn_and_a_line_or_a_broken_one_is_not() {
        for drawn in [
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::splat(1.0e-4),
            Vec3::new(-1.0, 1.0, 1.0),
        ] {
            let model = Affine3A::from_scale(drawn);
            assert!(Instance3D::new(model, [1.0; 4]).is_ok(), "{drawn}");
        }
        // A plane flattened along Y keeps its normal along Y.
        let flat = Instance3D::new(Affine3A::from_scale(Vec3::new(10.0, 0.0, 10.0)), [1.0; 4])
            .expect("a plane");
        let n = flat.normal.map(|r| glam::Vec4::from(r).truncate());
        let up = Vec3::new(n[0].dot(Vec3::Y), n[1].dot(Vec3::Y), n[2].dot(Vec3::Y));
        assert!(up.normalize().abs_diff_eq(Vec3::Y, 1e-6), "{up}");
        for line in [Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO] {
            let model = Affine3A::from_scale(line);
            assert_eq!(
                Instance3D::new(model, [1.0; 4]),
                Err(InstanceRefused::Degenerate)
            );
        }
        let broken = Affine3A::from_translation(Vec3::splat(f32::NAN));
        assert_eq!(
            Instance3D::new(broken, [1.0; 4]),
            Err(InstanceRefused::NotFinite)
        );
        let overflowing = Affine3A::from_scale(Vec3::splat(1.0e20));
        assert_eq!(
            Instance3D::new(overflowing, [1.0; 4]),
            Err(InstanceRefused::NotFinite)
        );
    }

    #[test]
    fn a_mirrored_model_keeps_its_normals_pointing_out() {
        let model = Affine3A::from_scale(Vec3::new(-2.0, 1.0, 1.0));
        assert_eq!(Instance3D::winding(&model), Winding::Clockwise);
        assert_eq!(
            Instance3D::winding(&Affine3A::from_scale(Vec3::splat(2.0))),
            Winding::CounterClockwise
        );
        let instance = Instance3D::new(model, [1.0; 4]).expect("invertible");
        // The +X face is now at −X; its normal must point to −X.
        let n = instance.normal.map(|r| glam::Vec4::from(r).truncate());
        let out = Vec3::new(n[0].dot(Vec3::X), n[1].dot(Vec3::X), n[2].dot(Vec3::X));
        assert!(out.x < 0.0, "{out}");
    }
}
