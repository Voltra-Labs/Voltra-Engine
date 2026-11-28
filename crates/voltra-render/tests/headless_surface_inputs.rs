//! A standard surface's inputs around its lighting, in pixels: a UV
//! offset moving a texture by a known texel, an alpha mask cutting with
//! and without MSAA, alpha to coverage weighing an edge once, a blend over
//! what is behind.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    AlphaMode, Instance3D, Lighting3D, Mesh3D, StandardSurface, SurfaceTextures, UvTransform,
};

#[test]
fn a_uv_offset_moves_the_texture_by_a_known_texel() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let colours = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ];
    let texture = strip(&gpu, &colours);
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let stretched = Instance3D::new(
        Affine3A::from_scale_rotation_translation(
            Vec3::new(4.0, 1.0, 1.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.0, -5.0),
        ),
        WHITE,
    )
    .expect("drawn");
    let textures = SurfaceTextures {
        base_color: Some(&texture),
        ..SurfaceTextures::NONE
    };
    for (shift, offset) in [(0usize, 0.0f32), (1, 0.25), (2, 0.5), (3, -0.25)] {
        let surface = StandardSurface {
            uv_transform: UvTransform {
                offset: [offset, 0.0],
                ..UvTransform::IDENTITY
            },
            ..StandardSurface::UNLIT
        };
        let material = gpu.renderer.surface_material(&surface, &textures);
        let pixels = picture(
            &mut gpu,
            (4, 1),
            columns_view(4),
            Lighting3D::DARK,
            &[Draw {
                mesh: &square,
                material: &material,
                instances: vec![stretched],
            }],
        );
        for (x, p) in pixels.iter().enumerate() {
            let texel = colours[(x + shift) % 4];
            assert!(
                within(*p, [texel[0], texel[1], texel[2]], 1),
                "offset {offset}: pixel {x} is {p:?}, texel {texel:?}"
            );
        }
    }
}

#[test]
fn a_mask_cuts_where_alpha_is_under_its_cutoff_with_and_without_msaa() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    // Transparent on the left half, opaque on the right.
    let texture = strip(
        &gpu,
        &[
            [255, 255, 255, 0],
            [255, 255, 255, 0],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
        ],
    );
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let stretched = Instance3D::new(
        Affine3A::from_scale_rotation_translation(
            Vec3::new(8.0, 1.0, 1.0),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.0, -5.0),
        ),
        [0.2, 0.9, 0.3, 1.0],
    )
    .expect("drawn");
    let surface = StandardSurface {
        alpha: AlphaMode::Mask(0.5),
        ..StandardSurface::UNLIT
    };
    let textures = SurfaceTextures {
        base_color: Some(&texture),
        ..SurfaceTextures::NONE
    };
    for samples in [1, 4] {
        gpu.renderer.set_msaa(samples);
        let material = gpu.renderer.surface_material(&surface, &textures);
        let pixels = picture(
            &mut gpu,
            (8, 1),
            columns_view(8),
            Lighting3D::DARK,
            &[Draw {
                mesh: &square,
                material: &material,
                instances: vec![stretched],
            }],
        );
        let green = [encode(0.2), encode(0.9), encode(0.3)];
        for (x, p) in pixels.iter().enumerate() {
            // Away from the middle, where the filter blends the halves.
            if x < 3 {
                assert!(within(*p, [0, 0, 0], 1), "{samples}×: cut at {x}: {p:?}");
            } else if x > 4 {
                assert!(within(*p, green, 1), "{samples}×: kept at {x}: {p:?}");
            }
        }
    }
}

#[test]
fn alpha_to_coverage_weighs_an_opaque_edge_by_its_coverage_once() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    // A white masked wall over black whose alpha sits a known way from the
    // cutoff and does not vary, so its ramp is the floor's (10⁻⁴) and every
    // pixel has one coverage: of 4 samples, 1, 2 and 3. The resolve weighs
    // the white by the samples covered; a colour premultiplied by the
    // coverage as well read coverage² (half coverage: 137, not 188).
    gpu.renderer.set_msaa(4);
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let cutoff = 0.5;
    for coverage in [0.25f32, 0.5, 0.75] {
        let surface = StandardSurface {
            base_color: [1.0, 1.0, 1.0, cutoff + (coverage - 0.5) * 1.0e-4],
            alpha: AlphaMode::Mask(cutoff),
            ..StandardSurface::UNLIT
        };
        let material = gpu
            .renderer
            .surface_material(&surface, &SurfaceTextures::NONE);
        let pixels = picture(
            &mut gpu,
            (8, 8),
            columns_view(8),
            Lighting3D::DARK,
            &[Draw {
                mesh: &square,
                material: &material,
                instances: vec![scaled(
                    Vec3::new(0.0, 0.0, -5.0),
                    Quat::IDENTITY,
                    Vec3::splat(16.0),
                    WHITE,
                )],
            }],
        );
        let want = encode(coverage);
        for p in &pixels {
            assert!(
                within(*p, [want; 3], 2),
                "coverage {coverage}: {p:?}, want {want}"
            );
            assert_eq!(p[3], 255, "over an opaque clear, opaque");
        }
    }
}

#[test]
fn a_blend_lays_its_colour_over_what_is_behind_by_its_alpha() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let unlit = unlit(&mut gpu.renderer);
    let glass = gpu.renderer.surface_material(
        &StandardSurface {
            alpha: AlphaMode::Blend,
            ..StandardSurface::UNLIT
        },
        &SurfaceTextures::NONE,
    );
    let behind = instance(Vec3::new(0.0, 0.0, -6.0), Quat::IDENTITY, 8.0, RED);
    let front = instance(
        Vec3::new(0.0, 0.0, -3.0),
        Quat::IDENTITY,
        8.0,
        [0.0, 0.0, 1.0, 0.25],
    );
    let pixels = picture(
        &mut gpu,
        (SIZE, SIZE),
        looking_down_minus_z(FOV, None),
        Lighting3D::DARK,
        &[
            Draw {
                mesh: &square,
                material: &unlit,
                instances: vec![behind],
            },
            Draw {
                mesh: &square,
                material: &glass,
                instances: vec![front],
            },
        ],
    );
    let want = [encode(0.75), 0, encode(0.25)];
    let p = pixels[(32 * SIZE + 32) as usize];
    assert!(within(p, want, 1), "{p:?}, {want:?}");
    assert_eq!(p[3], 255, "over an opaque wall, opaque");
}
