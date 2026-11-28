//! What the 3D pass tests share: meshes made by hand, instances, a
//! headless renderer drawing a scene and reading it back.

#![allow(dead_code)]

use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    Instance3D, Mesh3D, MeshData3D, MeshDraw3D, Scene3D, Vertex3D, View3D, Winding,
};
use voltra_render::{read_rgba8, wgpu, Filter, Layer, RenderTarget, Renderer};

pub const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// A unit cube about the origin, counter-clockwise from outside: four
/// vertices a face so each carries its face's normal.
pub fn cube() -> MeshData3D {
    let mut mesh = MeshData3D::default();
    for (normal, u, v) in [
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ] {
        quad(&mut mesh, normal * 0.5, u * 0.5, v * 0.5, normal);
    }
    mesh
}

/// A square facing +Z, `half` metres from its centre to each side.
pub fn square(half: f32) -> MeshData3D {
    let mut mesh = MeshData3D::default();
    quad(
        &mut mesh,
        Vec3::ZERO,
        Vec3::X * half,
        Vec3::Y * half,
        Vec3::Z,
    );
    mesh
}

pub fn quad(mesh: &mut MeshData3D, center: Vec3, u: Vec3, v: Vec3, normal: Vec3) {
    let base = mesh.vertices.len() as u32;
    for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        mesh.vertices.push(Vertex3D {
            position: (center + u * su + v * sv).to_array(),
            normal: normal.to_array(),
            uv: [(su + 1.0) * 0.5, (1.0 - sv) * 0.5],
            tangent: u.normalize().extend(1.0).to_array(),
        });
    }
    mesh.indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

pub fn instance(at: Vec3, rotation: Quat, scale: f32, tint: [f32; 4]) -> Instance3D {
    scaled(at, rotation, Vec3::splat(scale), tint)
}

pub fn scaled(at: Vec3, rotation: Quat, scale: Vec3, tint: [f32; 4]) -> Instance3D {
    let model = Affine3A::from_scale_rotation_translation(scale, rotation, at);
    Instance3D::new(model, tint).expect("a drawable placement")
}

pub struct Gpu {
    pub renderer: Renderer,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

pub fn gpu() -> Option<Gpu> {
    let renderer = Renderer::headless(64, 64)?;
    let device = renderer.context().device().clone();
    let queue = renderer.context().queue().clone();
    Some(Gpu {
        renderer,
        device,
        queue,
    })
}

impl Gpu {
    pub fn draw(
        &mut self,
        (width, height): (u32, u32),
        view: View3D,
        mesh: &Mesh3D,
        instances: &[Instance3D],
        draws: &[(u32, u32)],
    ) -> Vec<[u8; 4]> {
        let draws: Vec<MeshDraw3D<'_>> = draws
            .iter()
            .map(|&(start, end)| MeshDraw3D {
                mesh,
                instances: start..end,
                winding: Winding::CounterClockwise,
            })
            .collect();
        self.draw_all((width, height), view, instances, &draws)
    }

    pub fn draw_all(
        &mut self,
        (width, height): (u32, u32),
        view: View3D,
        instances: &[Instance3D],
        draws: &[MeshDraw3D<'_>],
    ) -> Vec<[u8; 4]> {
        let format = self.renderer.context().format();
        let target = RenderTarget::new(&self.device, "3d", format, width, height, Filter::Linear);
        let scene = Scene3D {
            view,
            instances,
            draws,
            clear: CLEAR,
        };
        self.renderer.render_3d_into(&target, &scene, Layer::EMPTY);
        read_rgba8(&self.device, &self.queue, target.texture(), width, height)
            .as_chunks::<4>()
            .0
            .to_vec()
    }
}

pub fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 40 && p[2] < 40
}

pub fn is_green(p: [u8; 4]) -> bool {
    p[1] > 200 && p[0] < 40 && p[2] < 40
}

pub fn looking_down_minus_z(fov_degrees: f32, far: Option<f32>) -> View3D {
    View3D::perspective(
        Affine3A::IDENTITY,
        fov_degrees.to_radians(),
        1.0,
        View3D::DEFAULT_NEAR,
        far,
    )
}
