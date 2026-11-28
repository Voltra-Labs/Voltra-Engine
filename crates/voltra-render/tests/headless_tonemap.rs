//! A 3D shot's post pass in pixels: linear HDR light through each tone
//! curve reads back what `Tonemap::apply` says on the CPU, one conversion
//! each way — no curve of linear 0.5 is 188, as `headless_post.rs` pins
//! for 2D — and MSAA smooths an edge that one sample leaves hard.
//!
//! Set `VOLTRA_SHOT_DIR` to keep a picture of the four curves.

mod common;

use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    Lighting3D, Look3D, Mesh3D, MeshDraw3D, Scene3D, Tonemap, View3D, Winding,
};
use voltra_render::{read_rgba8, Filter, Layer, RenderTarget};

/// Linear to the 8-bit sRGB a picture stores.
fn encode(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// A `width` × `height` picture of `tints[i]` filling column `i` of an
/// orthographic view, drawn through `look`; its pixels.
fn columns(gpu: &mut Gpu, tints: &[[f32; 4]], height: u32, look: &Look3D) -> Vec<[u8; 4]> {
    let width = tints.len() as u32;
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    // One metre a column, the view `width` metres across.
    let view = View3D::orthographic(
        Affine3A::IDENTITY,
        height as f32 * 0.5,
        width as f32 / height as f32,
        0.1,
        10.0,
    );
    let instances: Vec<_> = tints
        .iter()
        .enumerate()
        .map(|(i, &tint)| {
            scaled(
                Vec3::new(i as f32 - width as f32 * 0.5 + 0.5, 0.0, -5.0),
                Quat::IDENTITY,
                Vec3::new(1.0, height as f32, 1.0),
                tint,
            )
        })
        .collect();
    let unlit = unlit(&mut gpu.renderer);
    let draws = [MeshDraw3D {
        mesh: &square,
        material: &unlit,
        instances: 0..instances.len() as u32,
        winding: Winding::CounterClockwise,
    }];
    let format = gpu.renderer.context().format();
    let target = RenderTarget::new(&gpu.device, "curves", format, width, height, Filter::Linear);
    let scene = Scene3D {
        view,
        instances: &instances,
        draws: &draws,
        clear: CLEAR,
        lighting: Lighting3D::DARK,
        time: 0.0,
    };
    gpu.renderer
        .render_3d_into(&target, &scene, look, Layer::EMPTY);
    read_rgba8(&gpu.device, &gpu.queue, target.texture(), width, height)
        .as_chunks::<4>()
        .0
        .to_vec()
}

fn look(tonemap: Tonemap) -> Look3D {
    Look3D {
        tonemap,
        ..Look3D::NEUTRAL
    }
}

#[test]
fn no_curve_of_linear_half_reads_back_188() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let pixels = columns(&mut gpu, &[[0.5, 0.5, 0.5, 1.0]], 4, &look(Tonemap::None));
    for p in pixels {
        assert!((186..=190).contains(&p[0]), "{p:?}");
        assert_eq!((p[0], p[1]), (p[1], p[2]), "grey: {p:?}");
        assert_eq!(p[3], 255, "an opaque shot");
    }
}

#[test]
fn every_curve_on_the_gpu_is_its_twin_on_the_cpu() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    // Greys from black to 16, and saturated colours past one.
    let mut tints: Vec<[f32; 4]> = (0..24)
        .map(|i| {
            let v = if i == 0 {
                0.0
            } else {
                2f32.powf(i as f32 / 2.0 - 8.0)
            };
            [v, v, v, 1.0]
        })
        .collect();
    tints.extend([
        [8.0, 0.2, 0.05, 1.0],
        [0.1, 3.0, 0.4, 1.0],
        [0.02, 0.1, 12.0, 1.0],
        [0.9, 0.15, 0.1, 1.0],
    ]);
    let mut strip = Vec::new();
    for curve in Tonemap::ALL {
        let pixels = columns(&mut gpu, &tints, 2, &look(curve));
        for (i, tint) in tints.iter().enumerate() {
            let want = curve.apply([tint[0], tint[1], tint[2]]).map(encode);
            let got = pixels[i];
            for c in 0..3 {
                assert!(
                    (i32::from(got[c]) - i32::from(want[c])).abs() <= 1,
                    "{curve:?} of {tint:?}: {got:?}, the CPU says {want:?}"
                );
            }
        }
        strip.push(curve);
    }
    // The picture to look at: each curve a band, light doubling every 8
    // pixels from 2⁻⁸ on the left to 2⁸ on the right.
    let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") else {
        return;
    };
    let ramp: Vec<[f32; 4]> = (0..256)
        .map(|x| {
            let v = 2f32.powf(x as f32 / 16.0 - 8.0);
            [v, v * 0.6, v * 0.25, 1.0]
        })
        .collect();
    let mut image = Vec::new();
    for curve in strip {
        let band = columns(&mut gpu, &ramp, 32, &look(curve));
        image.extend(band.iter().flatten());
    }
    let path = std::path::PathBuf::from(dir).join("tonemap-curves.png");
    image::save_buffer(&path, &image, 256, 128, image::ColorType::Rgba8).expect("written");
}

#[test]
fn msaa_smooths_an_edge_that_one_sample_leaves_hard() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let view = looking_down_minus_z(45.0, None);
    // A red square turned 20° in the screen's plane, on black: every edge
    // pixel it half covers.
    let instances = [instance(
        Vec3::new(0.0, 0.0, -3.0),
        Quat::from_rotation_z(0.35),
        1.4,
        RED,
    )];
    let unlit = unlit(&mut gpu.renderer);
    let draws = [MeshDraw3D {
        mesh: &square,
        material: &unlit,
        instances: 0..1,
        winding: Winding::CounterClockwise,
    }];
    let mut between = Vec::new();
    for samples in [1, 4] {
        gpu.renderer.set_msaa(samples);
        assert_eq!(gpu.renderer.hdr_3d().samples, samples, "lavapipe has 4×");
        let pixels = gpu.draw_all((64, 64), view, &instances, &draws);
        let red = pixels.iter().filter(|p| is_red(**p)).count();
        // Neither black nor the square's red: a blend of the two.
        let mixed = pixels
            .iter()
            .filter(|p| p[0] > 20 && p[0] < 235 && p[1] < 10)
            .count();
        assert!(red > 300, "{samples}: {red}");
        between.push(mixed);
        if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
            let bytes: Vec<u8> = pixels.iter().flatten().copied().collect();
            let path = std::path::PathBuf::from(dir).join(format!("msaa-{samples}.png"));
            image::save_buffer(&path, &bytes, 64, 64, image::ColorType::Rgba8).expect("written");
        }
    }
    assert_eq!(between[0], 0, "one sample: hard edges");
    // Some 4 edges × 25 pixels, all but a few partly covered.
    assert!(between[1] > 60, "four samples: {} edge pixels", between[1]);
}
