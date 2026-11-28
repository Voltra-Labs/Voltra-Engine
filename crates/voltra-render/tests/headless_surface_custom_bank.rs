//! Custom surfaces beside the standard one: a custom surface returning the
//! standard drawing exactly as the standard does, the bank letting an
//! edited shader's pipelines go, and the shipped materials on spheres.

mod common;

use common::custom::*;
use common::surface::*;
use common::*;
use voltra_render::glam::{Quat, Vec3};
use voltra_render::material::Uniform;
use voltra_render::render3d::{
    AlphaClass, AlphaMode, Look3D, Mesh3D, StandardSurface, SurfaceBank, SurfaceKey,
    SurfaceLayouts, SurfaceLighting, SurfaceMaterial3D, SurfacePass, SurfaceShader,
    SurfaceTextures, SurfaceVertex, TextureSpace, Winding,
};
use voltra_render::wgpu;

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
