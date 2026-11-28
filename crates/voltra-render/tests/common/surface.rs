//! What the surface tests share: a lit wall filling the view, the CPU's
//! reading of each of its pixels through `render3d::surface::brdf`, a
//! picture of a few draws read back.

use super::*;
use voltra_render::glam::{Affine3A, Vec3};
use voltra_render::render3d::surface::brdf;
use voltra_render::render3d::{
    DirectionalLight, Instance3D, Lighting3D, Look3D, Mesh3D, MeshData3D, MeshDraw3D, Scene3D,
    StandardSurface, SurfaceMaterial3D, Vertex3D, View3D, Winding,
};
use voltra_render::{read_rgba8, Filter, Layer, RenderTarget, Texture};

pub const SIZE: u32 = 64;
pub const FOV: f32 = 50.0;
/// How far in front of the eye the plane is.
pub const DISTANCE: f32 = 3.0;

/// Linear to the 8-bit sRGB a picture stores.
pub fn encode(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

pub fn decode(byte: u8) -> f32 {
    let s = f32::from(byte) / 255.0;
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

pub fn light() -> Lighting3D {
    Lighting3D {
        directional: Some(DirectionalLight {
            // Down, left and away from the eye: lit from the upper right.
            direction: Vec3::new(-0.5, -0.3, -1.0).normalize(),
            color: [1.0, 0.95, 0.9],
            energy: 1.0,
        }),
        ambient: [0.05, 0.06, 0.08],
    }
}

pub fn save(name: &str, width: u32, height: u32, pixels: &[[u8; 4]]) {
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        let rgba: Vec<u8> = pixels.iter().flatten().copied().collect();
        image::save_buffer(
            std::path::Path::new(&dir).join(format!("{name}.png")),
            &rgba,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .expect("the picture is written");
    }
}

/// One instanced draw of a picture: a mesh, its material, its instances.
pub struct Draw<'a> {
    pub mesh: &'a Mesh3D,
    pub material: &'a SurfaceMaterial3D,
    pub instances: Vec<Instance3D>,
}

/// `draws` in order, through `view`, lit by `lighting`, no curve, one
/// sample a pixel unless `gpu` was set otherwise.
pub fn picture(
    gpu: &mut Gpu,
    size: (u32, u32),
    view: View3D,
    lighting: Lighting3D,
    draws: &[Draw<'_>],
) -> Vec<[u8; 4]> {
    picture_at(gpu, size, view, lighting, draws, 0.0, &Look3D::NEUTRAL)
}

/// [`picture`] at `time` seconds, through `look`.
pub fn picture_at(
    gpu: &mut Gpu,
    (width, height): (u32, u32),
    view: View3D,
    lighting: Lighting3D,
    draws: &[Draw<'_>],
    time: f32,
    look: &Look3D,
) -> Vec<[u8; 4]> {
    let mut instances = Vec::new();
    let mut ranges = Vec::new();
    for draw in draws {
        let start = instances.len() as u32;
        instances.extend(draw.instances.iter().copied());
        ranges.push(start..instances.len() as u32);
    }
    let draws: Vec<MeshDraw3D<'_>> = draws
        .iter()
        .zip(ranges)
        .map(|(draw, instances)| MeshDraw3D {
            mesh: draw.mesh,
            material: draw.material,
            instances,
            winding: Winding::CounterClockwise,
        })
        .collect();
    let format = gpu.renderer.context().format();
    let target = RenderTarget::new(
        &gpu.device,
        "surface",
        format,
        width,
        height,
        Filter::Linear,
    );
    let scene = Scene3D {
        view,
        instances: &instances,
        draws: &draws,
        clear: CLEAR,
        lighting,
        time,
    };
    gpu.renderer
        .render_3d_into(&target, &scene, look, Layer::EMPTY);
    read_rgba8(&gpu.device, &gpu.queue, target.texture(), width, height)
        .as_chunks::<4>()
        .0
        .to_vec()
}

/// A square facing the eye `DISTANCE` ahead, wide enough to fill the view.
pub fn wall() -> Instance3D {
    instance(Vec3::new(0.0, 0.0, -DISTANCE), Quat::IDENTITY, 8.0, WHITE)
}

pub const WHITE: [f32; 4] = [1.0; 4];

/// Where pixel `(x, y)`'s centre lands on the wall, in the world.
pub fn on_wall(x: u32, y: u32) -> Vec3 {
    let tan = (FOV.to_radians() * 0.5).tan();
    let nx = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
    let ny = 1.0 - (y as f32 + 0.5) / SIZE as f32 * 2.0;
    Vec3::new(nx * tan, ny * tan, -1.0) * DISTANCE
}

/// What the CPU says pixel `(x, y)` of the wall reads, shaded `n`.
pub fn expected(surface: &StandardSurface, n: Vec3, x: u32, y: u32) -> [u8; 3] {
    expected_under(&light(), surface, n, x, y)
}

/// [`expected`] lit by `lighting`.
pub fn expected_under(
    lighting: &Lighting3D,
    surface: &StandardSurface,
    n: Vec3,
    x: u32,
    y: u32,
) -> [u8; 3] {
    let sun = lighting.directional.expect("a light");
    let point = brdf::Point {
        base: Vec3::new(
            surface.base_color[0],
            surface.base_color[1],
            surface.base_color[2],
        ),
        metallic: surface.metallic,
        roughness: surface.roughness,
        occlusion: 1.0,
        emissive: Vec3::ZERO,
    };
    let out = brdf::shade(
        &point,
        &brdf::Light {
            n,
            v: (-on_wall(x, y)).normalize(),
            l: -sun.direction.normalize(),
            color: Vec3::from(sun.color) * sun.energy,
            ambient: Vec3::from(lighting.ambient),
        },
    );
    [encode(out.x), encode(out.y), encode(out.z)]
}

pub fn within(got: [u8; 4], want: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (i32::from(got[i]) - i32::from(want[i])).abs() <= by)
}

/// Pixels spread over the picture, the centre and its corners' insides.
pub const PROBES: [(u32, u32); 6] = [(32, 32), (8, 8), (56, 8), (8, 56), (56, 56), (40, 20)];

/// `colours` in a row, one texel each, sRGB bytes.
pub fn strip(gpu: &Gpu, colours: &[[u8; 4]]) -> Texture {
    let rgba: Vec<u8> = colours.iter().flatten().copied().collect();
    Texture::from_rgba8(
        &gpu.device,
        &gpu.queue,
        "strip",
        &rgba,
        colours.len() as u32,
        1,
        Filter::Linear,
    )
    .expect("a strip")
}

/// An orthographic view one metre a pixel, `width` across: each pixel's
/// centre on its texel's.
pub fn columns_view(width: u32) -> View3D {
    View3D::orthographic(Affine3A::IDENTITY, 0.5, width as f32, 0.1, 10.0)
}

/// A UV sphere of `rings` × `segments`, a metre across, normals out.
pub fn sphere(rings: u32, segments: u32) -> MeshData3D {
    let mut mesh = MeshData3D::default();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let (sp, cp) = (v * std::f32::consts::PI).sin_cos();
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let (st, ct) = (u * std::f32::consts::TAU).sin_cos();
            let n = Vec3::new(sp * ct, cp, -sp * st);
            mesh.vertices.push(Vertex3D {
                position: (n * 0.5).to_array(),
                normal: n.to_array(),
                uv: [u, v],
                tangent: [-st, 0.0, -ct, 1.0],
            });
        }
    }
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let (b, c, d) = (a + row, a + row + 1, a + 1);
            mesh.indices.extend([a, b, c, a, c, d]);
        }
    }
    mesh
}
