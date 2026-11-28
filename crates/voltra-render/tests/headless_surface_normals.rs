//! Normal maps in pixels: a map leaning along the tangent and one along
//! the bitangent, each under a light grazing from its side, compared with
//! `render3d::surface::brdf` with the map's bytes read as data — and not
//! as sRGB.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the pictures.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::Vec3;
use voltra_render::render3d::{
    DirectionalLight, Lighting3D, Mesh3D, StandardSurface, SurfaceMaterial3D, SurfaceTextures,
};
use voltra_render::{Filter, Texture};

#[test]
fn a_normal_map_tilts_the_shading_and_is_read_as_data() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let surface = StandardSurface {
        base_color: [0.7, 0.7, 0.7, 1.0],
        roughness: 0.6,
        ..StandardSurface::DEFAULT
    };
    let flat = gpu
        .renderer
        .surface_material(&surface, &SurfaceTextures::NONE);
    // Every texel the same normal: leaning toward +X along the tangent (a
    // byte of 200 is +0.569 as data, +0.155 decoded from sRGB), then
    // toward +Y along the bitangent, cross(n, t) — up the image — each
    // under a light grazing from that side.
    for (name, texel, from) in [
        ("tangent", [200u8, 128, 210, 255], Vec3::X),
        ("bitangent", [128u8, 200, 210, 255], Vec3::Y),
    ] {
        let map = Texture::from_rgba8(
            &gpu.device,
            &gpu.queue,
            "normal",
            &texel.repeat(4),
            2,
            2,
            Filter::Linear,
        )
        .expect("a 2 × 2 map");
        let mapped = gpu.renderer.surface_material(
            &surface,
            &SurfaceTextures {
                normal: Some(&map),
                ..SurfaceTextures::NONE
            },
        );
        let grazing = Lighting3D::sun(
            DirectionalLight {
                direction: (-from - Vec3::Z * 0.3).normalize(),
                color: [1.0; 3],
                energy: 1.5,
            },
            [0.05; 3],
        );
        let mut draw = |material: &SurfaceMaterial3D| {
            picture(
                &mut gpu,
                (SIZE, SIZE),
                looking_down_minus_z(FOV, None),
                grazing.clone(),
                &[Draw {
                    mesh: &square,
                    material,
                    instances: vec![wall()],
                }],
            )
        };
        let plain = draw(&flat);
        let bent = draw(&mapped);
        save(&format!("surface-normal-map-{name}"), SIZE, SIZE, &bent);
        // The tangent is +X, the bitangent +Y: the map's normal in the
        // world, as data.
        let data = |b: u8| f32::from(b) / 255.0 * 2.0 - 1.0;
        let n = Vec3::new(data(texel[0]), data(texel[1]), data(texel[2])).normalize();
        // As colour it would lean much less.
        let colour = |b: u8| decode(b) * 2.0 - 1.0;
        let wrong = Vec3::new(colour(texel[0]), colour(texel[1]), colour(texel[2])).normalize();
        for (x, y) in PROBES {
            let i = (y * SIZE + x) as usize;
            let want = expected_under(&grazing, &surface, n, x, y);
            assert!(
                within(bent[i], want, 2),
                "{name}: {:?}, the CPU {want:?} at {x}, {y}",
                bent[i]
            );
            assert!(within(
                plain[i],
                expected_under(&grazing, &surface, Vec3::Z, x, y),
                2
            ));
            // Leaning toward the light, the map brightens every probe.
            assert!(
                bent[i][0] > plain[i][0] + 40,
                "{name}: {:?} {:?}",
                bent[i],
                plain[i]
            );
            assert!(
                !within(bent[i], expected_under(&grazing, &surface, wrong, x, y), 2),
                "{name}: the map at {x}, {y} reads as if decoded from sRGB"
            );
        }
    }
}
