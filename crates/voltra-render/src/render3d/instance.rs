//! One drawn copy of a mesh, as the GPU reads it: its placement and tint.

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Mat3};

/// Per-instance data of an instanced 3D draw: 112 bytes, seven attributes.
///
/// The model matrix as an affine `3×vec4` (rows: the 3×3 part and the
/// translation in `w`), its normal matrix — the inverse transpose, which
/// keeps normals at right angles to a surface under non-uniform scale — as
/// another `3×vec4`, and a linear RGBA tint.
///
/// **The WebGL2 attribute budget** (16 attributes, a 255-byte stride per
/// buffer) is laid out now for everything 0.5 adds: [`Vertex3D`]'s 4, these
/// 7, the light list's one `Uint32x2` (stage 139) and skinning's two (155)
/// — 14 of 16. Pinned by a test, so a field added here that breaks the
/// floor fails at once rather than on a browser.
///
/// [`Vertex3D`]: super::Vertex3D
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Instance3D {
    /// The model matrix's rows: `x`, `y`, `z` and the translation in `w`.
    pub model: [[f32; 4]; 3],
    /// The normal matrix's rows, `w` unused.
    pub normal: [[f32; 4]; 3],
    /// Multiplies what is drawn; linear, alpha last.
    pub tint: [f32; 4],
}

impl Instance3D {
    /// The shader locations, 4 to 10: after [`Vertex3D`](super::Vertex3D)'s.
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4,
        10 => Float32x4,
    ];

    /// The instance buffer's layout: buffer slot 1, stepped per instance.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// The instance of a mesh placed by `model` and tinted `tint`, or `None`
    /// when `model` is not finite or collapses space (a zero scale on some
    /// axis): such a mesh covers no pixel, and its normal matrix does not
    /// exist.
    pub fn new(model: Affine3A, tint: [f32; 4]) -> Option<Self> {
        let linear = Mat3::from(model.matrix3);
        let determinant = linear.determinant();
        if !model.is_finite() || !determinant.is_finite() || determinant.abs() < 1.0e-12 {
            return None;
        }
        let normal = linear.inverse().transpose();
        let t = model.translation;
        let rows = |m: Mat3, w: [f32; 3]| {
            [0, 1, 2].map(|i| {
                let row = m.row(i);
                [row.x, row.y, row.z, w[i]]
            })
        };
        Some(Self {
            model: rows(linear, [t.x, t.y, t.z]),
            normal: rows(normal, [0.0; 3]),
            tint,
        })
    }
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;
    use crate::render3d::Vertex3D;

    /// What WebGL2 guarantees (`Limits::downlevel_webgl2_defaults`).
    const WEBGL2_ATTRIBUTES: usize = 16;
    const WEBGL2_STRIDE: u64 = 255;
    /// Attributes later stages add to the same draws: the light list's one
    /// `Uint32x2` (139) and the skin's joints and weights (155).
    const LATER: usize = 1 + 2;

    #[test]
    fn the_webgl2_attribute_budget_holds() {
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(limits.max_vertex_attributes as usize, WEBGL2_ATTRIBUTES);
        assert_eq!(
            u64::from(limits.max_vertex_buffer_array_stride),
            WEBGL2_STRIDE
        );
        let now = Vertex3D::ATTRIBUTES.len() + Instance3D::ATTRIBUTES.len();
        assert_eq!(now, 11);
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
        assert_eq!(Instance3D::layout().array_stride, 112);
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
    fn a_collapsed_or_broken_model_has_no_instance() {
        let flat = Affine3A::from_scale(Vec3::new(1.0, 0.0, 1.0));
        assert!(Instance3D::new(flat, [1.0; 4]).is_none());
        let broken = Affine3A::from_translation(Vec3::splat(f32::NAN));
        assert!(Instance3D::new(broken, [1.0; 4]).is_none());
    }
}
