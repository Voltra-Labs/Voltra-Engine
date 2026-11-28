//! Standard surfaces lit in pixels: one directional light and an ambient,
//! read back and compared with `render3d::surface::brdf`, the shader's
//! lighting on the CPU — an unlit white, a rough dielectric and a smooth
//! metal, both faces of a double-sided plane, a mirrored wall. Normal maps
//! are `headless_surface_normals.rs`.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the pictures.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    Instance3D, Look3D, Mesh3D, MeshDraw3D, Scene3D, StandardSurface, SurfaceMaterial3D,
    SurfaceTextures,
};
use voltra_render::{read_rgba8, wgpu, Filter, Layer, RenderTarget};

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
