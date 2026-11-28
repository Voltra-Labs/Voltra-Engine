//! Standard surfaces lit in pixels: one directional light and an ambient,
//! read back and compared with `render3d::surface::brdf`, the shader's
//! lighting on the CPU — an unlit white, a rough dielectric and a smooth
//! metal, normal maps read as data, both faces of a double-sided plane, a
//! mirrored wall.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the pictures.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    DirectionalLight, Instance3D, Lighting3D, Look3D, Mesh3D, MeshDraw3D, Scene3D, StandardSurface,
    SurfaceMaterial3D, SurfaceTextures,
};
use voltra_render::{read_rgba8, wgpu, Filter, Layer, RenderTarget, Texture};

#[test]
fn a_white_unlit_material_reads_back_white() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let unlit = unlit(&mut gpu.renderer);
    let pixels = picture(
        &mut gpu,
        (SIZE, SIZE),
        looking_down_minus_z(FOV, None),
        light(),
        &[Draw {
            mesh: &square,
            material: &unlit,
            instances: vec![wall()],
        }],
    );
    for (x, y) in PROBES {
        let p = pixels[(y * SIZE + x) as usize];
        assert!(within(p, [255; 3], 1), "{p:?} at {x}, {y}");
    }
}

#[test]
fn a_rough_dielectric_and_a_smooth_metal_match_the_cpu_brdf() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let rough_dielectric = StandardSurface {
        base_color: [0.8, 0.3, 0.2, 1.0],
        metallic: 0.0,
        roughness: 0.9,
        ..StandardSurface::DEFAULT
    };
    let smooth_metal = StandardSurface {
        base_color: [0.9, 0.7, 0.4, 1.0],
        metallic: 1.0,
        roughness: 0.25,
        ..StandardSurface::DEFAULT
    };
    for (name, surface) in [
        ("surface-rough-dielectric", rough_dielectric),
        ("surface-smooth-metal", smooth_metal),
    ] {
        let material = gpu
            .renderer
            .surface_material(&surface, &SurfaceTextures::NONE);
        let pixels = picture(
            &mut gpu,
            (SIZE, SIZE),
            looking_down_minus_z(FOV, None),
            light(),
            &[Draw {
                mesh: &square,
                material: &material,
                instances: vec![wall()],
            }],
        );
        save(name, SIZE, SIZE, &pixels);
        let mut worst = 0;
        for y in (2..SIZE).step_by(5) {
            for x in (2..SIZE).step_by(5) {
                let got = pixels[(y * SIZE + x) as usize];
                let want = expected(&surface, Vec3::Z, x, y);
                let off = (0..3)
                    .map(|i| (i32::from(got[i]) - i32::from(want[i])).abs())
                    .max()
                    .unwrap_or(0);
                worst = worst.max(off);
                assert!(off <= 2, "{name}: {got:?}, the CPU {want:?} at {x}, {y}");
            }
        }
        println!("{name}: within {worst}/255 of the CPU");
    }
    // Not a flat colour: the metal's highlight is where the mirror angle
    // is, the upper right; the far corner is several times darker.
    let metal = expected(&smooth_metal, Vec3::Z, 52, 12);
    let corner = expected(&smooth_metal, Vec3::Z, 4, 60);
    assert!(metal[0] > corner[0] + 60, "{metal:?} {corner:?}");
}

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
        let grazing = Lighting3D {
            directional: Some(DirectionalLight {
                direction: (-from - Vec3::Z * 0.3).normalize(),
                color: [1.0; 3],
                energy: 1.5,
            }),
            ambient: [0.05; 3],
        };
        let mut draw = |material: &SurfaceMaterial3D| {
            picture(
                &mut gpu,
                (SIZE, SIZE),
                looking_down_minus_z(FOV, None),
                grazing,
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

#[test]
fn a_double_sided_plane_is_lit_from_behind_as_its_front_turned_round() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let surface = StandardSurface {
        base_color: [0.6, 0.6, 0.6, 1.0],
        ..StandardSurface::DEFAULT
    };
    let one = gpu
        .renderer
        .surface_material(&surface, &SurfaceTextures::NONE);
    let both = gpu.renderer.surface_material(
        &StandardSurface {
            double_sided: true,
            ..surface
        },
        &SurfaceTextures::NONE,
    );
    // Turned half round: its back faces the eye.
    let back = instance(
        Vec3::new(0.0, 0.0, -DISTANCE),
        Quat::from_rotation_y(std::f32::consts::PI),
        8.0,
        WHITE,
    );
    let mut draw = |material: &SurfaceMaterial3D| {
        picture(
            &mut gpu,
            (SIZE, SIZE),
            looking_down_minus_z(FOV, None),
            light(),
            &[Draw {
                mesh: &square,
                material,
                instances: vec![back],
            }],
        )
    };
    let culled = draw(&one);
    let lit = draw(&both);
    for (x, y) in PROBES {
        let i = (y * SIZE + x) as usize;
        assert!(within(culled[i], [0, 0, 0], 0), "one side: {:?}", culled[i]);
        // Its normal turned toward the eye: lit as a front would be.
        let want = expected(&surface, Vec3::Z, x, y);
        assert!(within(lit[i], want, 2), "{:?}, the CPU {want:?}", lit[i]);
    }
}

/// What stays flat or mirrored is still a surface: a mirrored wall's
/// normal map and light read as the unmirrored one's.
#[test]
fn a_mirrored_wall_is_lit_as_the_unmirrored_one() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let surface = StandardSurface {
        base_color: [0.5, 0.7, 0.9, 1.0],
        roughness: 0.4,
        ..StandardSurface::DEFAULT
    };
    let material = gpu
        .renderer
        .surface_material(&surface, &SurfaceTextures::NONE);
    let mirrored = scaled(
        Vec3::new(0.0, 0.0, -DISTANCE),
        Quat::IDENTITY,
        Vec3::new(-8.0, 8.0, 8.0),
        WHITE,
    );
    let instances = [mirrored];
    let draws = [MeshDraw3D {
        mesh: &square,
        material: &material,
        instances: 0..1,
        winding: Instance3D::winding(&Affine3A::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
    }];
    let format = gpu.renderer.context().format();
    let target = RenderTarget::new(&gpu.device, "m", format, SIZE, SIZE, Filter::Linear);
    let scene = Scene3D {
        view: looking_down_minus_z(FOV, None),
        instances: &instances,
        draws: &draws,
        clear: wgpu::Color::BLACK,
        lighting: light(),
    };
    gpu.renderer
        .render_3d_into(&target, &scene, &Look3D::NEUTRAL, Layer::EMPTY);
    let pixels: Vec<[u8; 4]> = read_rgba8(&gpu.device, &gpu.queue, target.texture(), SIZE, SIZE)
        .as_chunks::<4>()
        .0
        .to_vec();
    for (x, y) in PROBES {
        let want = expected(&surface, Vec3::Z, x, y);
        let got = pixels[(y * SIZE + x) as usize];
        assert!(
            within(got, want, 2),
            "{got:?}, the CPU {want:?} at {x}, {y}"
        );
    }
}
