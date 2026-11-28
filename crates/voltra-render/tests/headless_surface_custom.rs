//! Custom surface materials in pixels: a `surface()` of the game's own
//! composed around the engine's vertex stage and lighting — the shipped
//! world tint, scroll and dissolve, a custom texture read as colour and as
//! data with a custom uniform, a custom surface returning the standard one
//! drawing exactly as the standard does, and the bank letting an edited
//! shader's pipelines go.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::surface::*;
use common::*;
use voltra_render::glam::{Quat, Vec3};
use voltra_render::material::{Layout, Uniform};
use voltra_render::render3d::{
    compose_surface, AlphaClass, AlphaMode, CustomImage, CustomShading, Lighting3D, Look3D, Mesh3D,
    StandardSurface, SurfaceBank, SurfaceKey, SurfaceLayouts, SurfaceLighting, SurfaceMaterial3D,
    SurfacePass, SurfaceShader, SurfaceTextures, SurfaceVertex, TextureSpace, Winding,
    CUSTOM_TEXTURES,
};
use voltra_render::{wgpu, Texture};

const WORLD_TINT: &str = include_str!("../../../assets/materials/world_tint.wgsl");
const SCROLL: &str = include_str!("../../../assets/materials/scroll.wgsl");
const DISSOLVE: &str = include_str!("../../../assets/materials/dissolve.wgsl");

/// `source`'s shading with `uniforms` at their defaults, sampling the
/// named `textures`.
fn shading(
    source: &str,
    uniforms: &[(&str, Uniform)],
    textures: &[&str],
    lighting: SurfaceLighting,
) -> CustomShading {
    let uniforms: BTreeMap<String, Uniform> = uniforms
        .iter()
        .map(|(n, u)| ((*n).to_owned(), *u))
        .collect();
    let layout = Layout::new(&uniforms).expect("fits");
    let composed = compose_surface(source, &layout, textures, lighting).expect("composes");
    let shader = SurfaceShader::new(composed, textures.len()).unwrap_or_else(|e| panic!("{e}"));
    CustomShading {
        shader: Arc::new(shader),
        block: layout.block(&BTreeMap::new()),
    }
}

fn with_custom<'a>(images: &[(&'a Texture, TextureSpace)]) -> SurfaceTextures<'a> {
    let mut custom = [None; CUSTOM_TEXTURES];
    for (slot, (texture, space)) in custom.iter_mut().zip(images) {
        *slot = Some(CustomImage {
            texture,
            space: *space,
        });
    }
    SurfaceTextures {
        custom,
        ..SurfaceTextures::NONE
    }
}

/// A square `width` × `span` metres wide, moved `shift` along x, under a
/// `width` × 1 columns view: filling it while `span` covers the shift.
fn row(
    gpu: &mut Gpu,
    width: u32,
    material: &SurfaceMaterial3D,
    (shift, span): (f32, f32),
    time: f32,
) -> Vec<[u8; 4]> {
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    picture_at(
        gpu,
        (width, 1),
        columns_view(width),
        Lighting3D::DARK,
        &[Draw {
            mesh: &square,
            material,
            instances: vec![scaled(
                Vec3::new(shift, 0.0, -5.0),
                Quat::IDENTITY,
                Vec3::new(width as f32 * span, 1.0, 1.0),
                WHITE,
            )],
        }],
        time,
        &Look3D::NEUTRAL,
    )
}

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

#[test]
fn a_custom_surface_returning_the_standard_one_draws_as_the_standard_does() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let surface = StandardSurface {
        base_color: [0.9, 0.6, 0.3, 1.0],
        metallic: 0.7,
        roughness: 0.35,
        ..StandardSurface::DEFAULT
    };
    let plane = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let standard = gpu
        .renderer
        .surface_material(&surface, &SurfaceTextures::NONE);
    // The standard's own text is the standard's shader, one pipeline.
    let same = shading(
        "fn surface(in: SurfaceInput) -> Surface {\n    return standard_surface(in);\n}\n",
        &[],
        &[],
        SurfaceLighting::Lit,
    );
    assert_eq!(same.shader.revision(), SurfaceShader::standard().revision());
    let custom = shading(
        "fn surface(in: SurfaceInput) -> Surface {\n    let s = standard_surface(in);\n    return s;\n}\n",
        &[],
        &[],
        SurfaceLighting::Lit,
    );
    assert_ne!(
        custom.shader.revision(),
        SurfaceShader::standard().revision(),
        "its own pipeline, not the standard's"
    );
    let own = gpu
        .renderer
        .custom_surface_material(&surface, &SurfaceTextures::NONE, Some(&custom));
    let mut draw = |material: &SurfaceMaterial3D| {
        picture(
            &mut gpu,
            (SIZE, SIZE),
            looking_down_minus_z(FOV, None),
            light(),
            &[Draw {
                mesh: &plane,
                material,
                instances: vec![wall()],
            }],
        )
    };
    let a = draw(&standard);
    let b = draw(&own);
    assert!(a.iter().any(|p| p[..3] != [0, 0, 0]), "lit");
    // The standard is pinned against the CPU's BRDF (headless_surface.rs).
    assert_eq!(a, b, "pixel for pixel");
}

#[test]
fn an_edited_shader_s_pipelines_are_let_go_once_unused() {
    let Some(gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let layouts = SurfaceLayouts::new(&gpu.device, &gpu.queue);
    let mut bank = SurfaceBank::new(&gpu.device, &layouts, wgpu::TextureFormat::Rgba16Float, 1);
    let key = |shader: &SurfaceShader| SurfaceKey {
        shader: shader.revision(),
        vertex: SurfaceVertex::Plain,
        alpha: AlphaClass::Opaque,
        double_sided: false,
        winding: Winding::CounterClockwise,
        pass: SurfacePass::Main,
    };
    let standard = SurfaceShader::standard();
    let first = shading(
        WORLD_TINT,
        &[
            ("start_color", Uniform::Color((1.0, 0.0, 0.0, 1.0))),
            ("period", Uniform::Float(1.0)),
            ("end_color", Uniform::Color((0.0, 0.0, 1.0, 1.0))),
        ],
        &[],
        SurfaceLighting::Lit,
    )
    .shader;
    let edited = shading(
        WORLD_TINT,
        &[
            ("start_color", Uniform::Color((1.0, 0.0, 0.0, 1.0))),
            ("period", Uniform::Float(1.0)),
            ("end_color", Uniform::Color((0.0, 0.0, 1.0, 1.0))),
        ],
        &[],
        SurfaceLighting::Unlit,
    )
    .shader;
    bank.begin();
    bank.prepare(&gpu.device, key(&standard), &standard);
    bank.prepare(&gpu.device, key(&first), &first);
    assert_eq!((bank.shaders(), bank.len()), (2, 2));
    // The material is edited: every later shot draws the new shader.
    for _ in 0..=voltra_render::render3d::surface::KEEP_USES {
        bank.begin();
        bank.prepare(&gpu.device, key(&edited), &edited);
    }
    assert!(bank.pipeline(key(&first)).is_none(), "the old one let go");
    assert!(bank.pipeline(key(&edited)).is_some());
    assert!(bank.pipeline(key(&standard)).is_some(), "the standard kept");
    assert_eq!(bank.shaders(), 2);
}

/// A texture shipped in `assets/textures/`.
fn shipped(gpu: &Gpu, name: &str) -> Texture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/textures")
        .join(name);
    let image = image::open(&path).expect("the shipped texture").to_rgba8();
    Texture::from_rgba8(
        &gpu.device,
        &gpu.queue,
        name,
        image.as_raw(),
        image.width(),
        image.height(),
        voltra_render::Filter::Linear,
    )
    .expect("uploaded")
}

#[test]
fn the_shipped_custom_materials_on_spheres() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let size = 256;
    let ball = Mesh3D::new(&gpu.device, "sphere", &sphere(32, 64)).expect("a sphere");
    let noise = shipped(&gpu, "noise.png");
    let stripes = shipped(&gpu, "stripes.png");
    let lit = StandardSurface {
        roughness: 0.4,
        ..StandardSurface::DEFAULT
    };
    let tint = shading(
        WORLD_TINT,
        &[
            ("start_color", Uniform::Color((1.0, 0.55, 0.3, 1.0))),
            ("period", Uniform::Float(1.0)),
            ("end_color", Uniform::Color((0.3, 0.6, 1.0, 1.0))),
        ],
        &[],
        SurfaceLighting::Lit,
    );
    let scroll = shading(
        SCROLL,
        &[("speed", Uniform::Vec2((0.25, 0.0)))],
        &["pattern"],
        SurfaceLighting::Lit,
    );
    let dissolve = shading(
        DISSOLVE,
        &[
            ("amount", Uniform::Float(0.45)),
            ("edge", Uniform::Float(0.06)),
            ("edge_color", Uniform::Color((4.0, 1.2, 0.2, 1.0))),
        ],
        &["noise"],
        SurfaceLighting::Lit,
    );
    let masked = StandardSurface {
        alpha: AlphaMode::Mask(0.5),
        double_sided: true,
        ..lit
    };
    let materials = [
        gpu.renderer
            .custom_surface_material(&lit, &SurfaceTextures::NONE, Some(&tint)),
        gpu.renderer.custom_surface_material(
            &lit,
            &with_custom(&[(&stripes, TextureSpace::Color)]),
            Some(&scroll),
        ),
        gpu.renderer.custom_surface_material(
            &masked,
            &with_custom(&[(&noise, TextureSpace::Data)]),
            Some(&dissolve),
        ),
        gpu.renderer.surface_material(&lit, &SurfaceTextures::NONE),
    ];
    let at = [(-0.75, 0.75), (0.75, 0.75), (-0.75, -0.75), (0.75, -0.75)];
    let draws: Vec<Draw<'_>> = materials
        .iter()
        .zip(at)
        .map(|(material, (x, y))| Draw {
            mesh: &ball,
            material,
            instances: vec![instance(
                Vec3::new(x, y, -5.0),
                Quat::from_rotation_y(0.6),
                1.3,
                WHITE,
            )],
        })
        .collect();
    let view = looking_down_minus_z(40.0, None);
    let pixels = picture_at(
        &mut gpu,
        (size, size),
        view,
        light(),
        &draws,
        1.5,
        &Look3D::default(),
    );
    save("custom_surfaces", size, size, &pixels);
    // Each sphere drew; the dissolved one has holes the clear shows through.
    let quadrant = |qx: u32, qy: u32| {
        let mut drawn = 0;
        for y in qy * size / 2..(qy + 1) * size / 2 {
            for x in qx * size / 2..(qx + 1) * size / 2 {
                if pixels[(y * size + x) as usize][..3] != [0, 0, 0] {
                    drawn += 1;
                }
            }
        }
        drawn
    };
    let full = quadrant(1, 1);
    assert!(full > 5_000, "the standard sphere: {full}");
    assert!(quadrant(0, 0) > 5_000 && quadrant(1, 0) > 5_000);
    let dissolved = quadrant(0, 1);
    assert!(dissolved < full - 500, "holes: {dissolved} of {full}");
}
