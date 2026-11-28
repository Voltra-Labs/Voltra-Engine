//! Light past a half float's range: a mirror-smooth metal sphere lit from
//! behind, its rim near the mirror angle at grazing — where GGX and Smith
//! reach some 10⁵ at energy 1 — and beside it a sphere whose emissive is
//! 10³⁰, sure to overflow, drawn with bloom through every curve. An
//! infinite texel turns to NaN in bloom's threshold and in Reinhard's and
//! ACES's ratios, and a NaN reads back black and spreads over the glow.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the pictures.

mod common;

use common::surface::*;
use common::*;
use voltra_render::bloom::BloomSettings;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::post::PostSettings;
use voltra_render::render3d::{
    DirectionalLight, Lighting3D, Look3D, Mesh3D, MeshDraw3D, Scene3D, StandardSurface,
    SurfaceTextures, Tonemap, View3D, Winding,
};
use voltra_render::{read_rgba8, wgpu, Filter, Layer, RenderTarget};

#[test]
fn a_mirror_rim_lit_from_behind_stays_finite_through_bloom_and_every_curve() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let ball = Mesh3D::new(&gpu.device, "sphere", &sphere(48, 96)).expect("a sphere");
    let mirror = gpu.renderer.surface_material(
        &StandardSurface {
            metallic: 1.0,
            roughness: 0.0,
            ..StandardSurface::DEFAULT
        },
        &SurfaceTextures::NONE,
    );
    let glowing = gpu.renderer.surface_material(
        &StandardSurface {
            emissive: [1.0e30; 3],
            ..StandardSurface::DEFAULT
        },
        &SurfaceTextures::NONE,
    );
    let instances = [
        instance(Vec3::new(-0.8, 0.0, -3.0), Quat::IDENTITY, 1.2, [1.0; 4]),
        instance(Vec3::new(0.8, 0.0, -3.0), Quat::IDENTITY, 1.2, [1.0; 4]),
    ];
    let draws = [
        MeshDraw3D {
            mesh: &ball,
            material: &mirror,
            instances: 0..1,
            winding: Winding::CounterClockwise,
        },
        MeshDraw3D {
            mesh: &ball,
            material: &glowing,
            instances: 1..2,
            winding: Winding::CounterClockwise,
        },
    ];
    let size = 64;
    let view = View3D::perspective(Affine3A::IDENTITY, 40f32.to_radians(), 1.0, 0.1, None);
    for curve in Tonemap::ALL {
        let look = Look3D {
            tonemap: curve,
            post: PostSettings {
                // Exposed down so every finite light — the capped surface,
                // its glow — lands under 1, and only infinity or NaN
                // saturates.
                exposure: 1.0e-4,
                bloom: BloomSettings {
                    intensity: 0.05,
                    ..BloomSettings::OFF
                },
                ..PostSettings::NEUTRAL
            },
        };
        let format = gpu.renderer.context().format();
        let target = RenderTarget::new(&gpu.device, "overflow", format, size, size, Filter::Linear);
        let scene = Scene3D {
            view,
            instances: &instances,
            draws: &draws,
            clear: wgpu::Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            },
            lighting: Lighting3D {
                directional: Some(DirectionalLight {
                    // From behind the sphere toward the eye, a little down:
                    // its rim reflects it into the eye near grazing.
                    direction: Vec3::new(0.0, -0.08, 1.0).normalize(),
                    color: [1.0; 3],
                    energy: 4.0,
                }),
                ambient: [0.2; 3],
            },
            time: 0.0,
        };
        gpu.renderer
            .render_3d_into(&target, &scene, &look, Layer::EMPTY);
        let pixels: Vec<[u8; 4]> =
            read_rgba8(&gpu.device, &gpu.queue, target.texture(), size, size)
                .as_chunks::<4>()
                .0
                .to_vec();
        save(&format!("surface-overflow-{curve:?}"), size, size, &pixels);
        // Lavapipe writes an infinite or NaN texel as 1 into the 8-bit
        // picture (another device may write 0): with the light exposed
        // under 1, a saturated pixel is one.
        let saturated = pixels
            .iter()
            .filter(|p| p[..3].iter().any(|&c| c >= 250))
            .count();
        assert_eq!(
            saturated, 0,
            "{curve:?}: {saturated} pixels of infinite light"
        );
        let brightest = pixels.iter().map(|p| p[0]).max().unwrap_or(0);
        assert!(
            brightest > 60,
            "{curve:?}: the capped glow drawn: {brightest}"
        );
    }
}
