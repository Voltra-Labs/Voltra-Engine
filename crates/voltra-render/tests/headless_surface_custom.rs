//! Custom surface materials in pixels: a `surface()` of the game's own
//! composed around the engine's vertex stage and lighting — the shipped
//! world tint, scroll and dissolve, and a custom texture read as colour and
//! as data with a custom uniform.

mod common;

use common::custom::*;
use common::surface::*;
use common::*;
use voltra_render::material::Uniform;
use voltra_render::render3d::{
    AlphaClass, AlphaMode, StandardSurface, SurfaceLighting, SurfaceTextures, TextureSpace,
};

#[test]
fn the_world_tint_colours_by_where_the_surface_is_in_the_world() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let custom = shading(
        WORLD_TINT,
        &[
            ("start_color", Uniform::Color((1.0, 0.0, 0.0, 1.0))),
            ("period", Uniform::Float(8.0)),
            ("end_color", Uniform::Color((0.0, 0.0, 1.0, 1.0))),
        ],
        &[],
        SurfaceLighting::Unlit,
    );
    let material = gpu.renderer.custom_surface_material(
        &StandardSurface::UNLIT,
        &SurfaceTextures::NONE,
        Some(&custom),
    );
    // Moving the square under the view changes nothing: the colour is the
    // world's, not the mesh's.
    for shift in [0.0, 1.0, -2.5] {
        let pixels = row(&mut gpu, 8, &material, (shift, 2.0), 0.0);
        for (x, p) in pixels.iter().enumerate() {
            let world = x as f32 - 3.5;
            let t = (world / 8.0).rem_euclid(1.0);
            let want = [encode(1.0 - t), 0, encode(t)];
            assert!(
                within(*p, want, 1),
                "shift {shift}: pixel {x} {p:?}, want {want:?}"
            );
        }
    }
}

#[test]
fn the_scroll_moves_its_texture_by_a_texel_a_second() {
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
    let pattern = strip(&gpu, &colours);
    // A quarter of the texture — one texel of four — a second.
    let custom = shading(
        SCROLL,
        &[("speed", Uniform::Vec2((0.25, 0.0)))],
        &["pattern"],
        SurfaceLighting::Unlit,
    );
    let material = gpu.renderer.custom_surface_material(
        &StandardSurface::UNLIT,
        &with_custom(&[(&pattern, TextureSpace::Color)]),
        Some(&custom),
    );
    // An hour and a second is a second: the time wraps at an hour.
    for (time, shift) in [(0.0f32, 0usize), (1.0, 1), (2.0, 2), (3.0, 3), (3601.0, 1)] {
        let pixels = row(&mut gpu, 4, &material, (0.0, 1.0), time);
        for (x, p) in pixels.iter().enumerate() {
            let texel = colours[(x + shift) % 4];
            assert!(
                within(*p, [texel[0], texel[1], texel[2]], 1),
                "at {time} s: pixel {x} is {p:?}, texel {texel:?}"
            );
        }
    }
}

#[test]
fn a_custom_texture_is_read_as_its_space_says_times_a_custom_uniform() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let grey = strip(&gpu, &[[128, 128, 128, 255]]);
    let source = "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = material.tint.rgb * textureSample(level, surface_sampler, in.uv).r;\n    return s;\n}\n";
    let custom = shading(
        source,
        &[("tint", Uniform::Color((1.0, 0.5, 0.25, 1.0)))],
        &["level"],
        SurfaceLighting::Unlit,
    );
    for (space, level) in [
        (TextureSpace::Data, 128.0 / 255.0),
        (TextureSpace::Color, decode(128)),
    ] {
        let material = gpu.renderer.custom_surface_material(
            &StandardSurface::UNLIT,
            &with_custom(&[(&grey, space)]),
            Some(&custom),
        );
        let want = [encode(level), encode(level * 0.5), encode(level * 0.25)];
        for p in row(&mut gpu, 4, &material, (0.0, 1.0), 0.0) {
            assert!(within(p, want, 1), "{space:?}: {p:?}, want {want:?}");
        }
    }
}

#[test]
fn the_dissolve_discards_where_its_noise_is_under_the_amount() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    // Noise 0, 100, 140, 255 of 255 against an amount of 0.45 (115): the
    // first two cut, the last two kept, as data — read as sRGB, 140 would
    // be 0.26 and cut too.
    let noise = strip(
        &gpu,
        &[
            [0, 0, 0, 255],
            [100, 100, 100, 255],
            [140, 140, 140, 255],
            [255, 255, 255, 255],
        ],
    );
    let custom = shading(
        DISSOLVE,
        &[
            ("amount", Uniform::Float(0.45)),
            ("edge", Uniform::Float(0.0)),
            ("edge_color", Uniform::Color((0.0, 0.0, 0.0, 1.0))),
        ],
        &["noise"],
        SurfaceLighting::Unlit,
    );
    let surface = StandardSurface {
        base_color: [0.2, 0.9, 0.3, 1.0],
        alpha: AlphaMode::Mask(0.5),
        ..StandardSurface::UNLIT
    };
    let green = [encode(0.2), encode(0.9), encode(0.3)];
    for samples in [1, 4] {
        gpu.renderer.set_msaa(samples);
        let material = gpu.renderer.custom_surface_material(
            &surface,
            &with_custom(&[(&noise, TextureSpace::Data)]),
            Some(&custom),
        );
        assert_eq!(material.alpha(), AlphaClass::Mask);
        let pixels = row(&mut gpu, 4, &material, (0.0, 1.0), 0.0);
        for (x, p) in pixels.iter().enumerate() {
            let want = if x < 2 { [0, 0, 0] } else { green };
            assert!(
                within(*p, want, 1),
                "{samples}×: pixel {x} {p:?}, want {want:?}"
            );
        }
    }
}
