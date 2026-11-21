//! The lit sample scene, `assets/scenes/lights.ron`, and the tests that keep
//! it honest.
//!
//! As with `demo_scene.rs`: `regenerate_the_lights_scene` writes the file
//! through the same `save` the editor's Save menu uses and is `#[ignore]`d,
//! because a test run must never rewrite the working tree; the rest load
//! what is committed. One of them draws it, lights and all, so a scene that
//! still loads but no longer casts a shadow is caught too.
//!
//! `write_the_lights_preview` (also ignored) renders it to
//! `target/lights-preview.png`, for looking at without a window.
//!
//! The GPU tests skip themselves when no adapter is available.

use std::path::{Path, PathBuf};

use voltra_ecs::World;
use voltra_render::camera::CameraBinding;
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{
    pipeline, texture, wgpu, Filter, LightPass, PostPass, PostSettings, RenderTarget, Texture,
};
use voltra_scene::format::{load, save};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{
    camera, light, Camera, CasterShape, Collider, ComponentRegistry, LightShadows, Name,
    PointLight2D, SceneId, ShadowCaster2D, Sprite, SpriteBatch, Transform,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

fn lights_scene() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenes/lights.ron")
}

/// Spawns a named entity at `at` with `scale`, as the editor would.
fn named(world: &mut World, name: &str, at: Vec2, scale: Vec2) -> voltra_ecs::Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::from_translation(at).with_scale(scale));
    entity
}

/// A flat-coloured quad, drawn at `order`.
fn quad(color: [f32; 3], order: i32) -> Sprite {
    Sprite {
        color: [color[0], color[1], color[2], 1.0],
        ..Sprite::default().with_sort_order(order)
    }
}

/// The unit box a unit quad covers; the transform's scale sizes both.
const UNIT_BOX: Collider = Collider::Box {
    half_extents: Vec2::splat(0.5),
};

/// A night room: a pale wall, a floor, two pillars and a crate standing on
/// it and a wire overhead, lit by a warm lamp at the left and a cool one at
/// the top right, plus a small ember that casts no shadow.
///
/// Every way a shadow can be cast is in it: colliders as they are (the
/// pillars), a turned box that shadows itself (the crate), and a polygon of
/// its own with no collider at all (the wire, an open line).
fn lights_world() -> World {
    let mut world = World::new();

    let eye = named(&mut world, "Camera", Vec2::ZERO, Vec2::ONE);
    world.insert(
        eye,
        Camera {
            size: 4.0,
            ambient: Some([0.06, 0.07, 0.12]),
            ..Camera::default()
        },
    );

    let wall = named(&mut world, "Wall", Vec2::ZERO, Vec2::new(16.0, 9.0));
    world.insert(wall, quad([0.85, 0.84, 0.8], -10));

    let floor = named(
        &mut world,
        "Floor",
        Vec2::new(0.0, -3.5),
        Vec2::new(16.0, 1.0),
    );
    world.insert(floor, quad([0.35, 0.32, 0.3], 0));
    world.insert(floor, UNIT_BOX);

    for (name, x) in [("Pillar left", -2.0), ("Pillar right", 1.0)] {
        let pillar = named(&mut world, name, Vec2::new(x, -1.75), Vec2::new(0.6, 2.5));
        world.insert(pillar, quad([0.55, 0.5, 0.45], 1));
        world.insert(pillar, UNIT_BOX);
        world.insert(pillar, ShadowCaster2D::default());
    }

    let crate_box = named(&mut world, "Crate", Vec2::new(3.5, -2.4), Vec2::splat(1.0));
    if let Some(transform) = world.get_mut::<Transform>(crate_box) {
        transform.rotation = 0.35;
    }
    world.insert(crate_box, quad([0.6, 0.42, 0.25], 1));
    world.insert(crate_box, UNIT_BOX);
    world.insert(
        crate_box,
        ShadowCaster2D {
            self_shadow: true,
            ..ShadowCaster2D::default()
        },
    );

    let wire = named(
        &mut world,
        "Wire",
        Vec2::new(-0.5, 2.4),
        Vec2::new(3.0, 0.08),
    );
    world.insert(wire, quad([0.2, 0.2, 0.22], 1));
    world.insert(
        wire,
        ShadowCaster2D {
            shape: CasterShape::Polygon {
                points: vec![Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0)],
                closed: false,
            },
            self_shadow: false,
        },
    );

    let warm = named(&mut world, "Warm lamp", Vec2::new(-4.5, 1.0), Vec2::ONE);
    world.insert(
        warm,
        PointLight2D {
            color: [1.0, 0.8, 0.55],
            intensity: 1.6,
            radius: 10.0,
            falloff: 1.5,
            shadows: Some(LightShadows::default()),
        },
    );

    let cool = named(&mut world, "Cool lamp", Vec2::new(4.5, 3.0), Vec2::ONE);
    world.insert(
        cool,
        PointLight2D {
            color: [0.5, 0.7, 1.0],
            intensity: 1.2,
            radius: 8.0,
            falloff: 2.0,
            shadows: Some(LightShadows {
                strength: 0.85,
                softness: 0.08,
            }),
        },
    );

    let ember = named(&mut world, "Ember", Vec2::new(-0.5, -2.6), Vec2::ONE);
    world.insert(
        ember,
        PointLight2D {
            color: [1.0, 0.3, 0.15],
            intensity: 1.5,
            radius: 1.5,
            falloff: 2.0,
            shadows: None,
        },
    );

    world
}

fn committed() -> World {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    load(&lights_scene(), &registry, &mut world).expect("the committed lights scene must load");
    world
}

#[test]
#[ignore = "writes into the working tree; run with --ignored to regenerate"]
fn regenerate_the_lights_scene() {
    let registry = ComponentRegistry::with_defaults();
    save(&lights_world(), &registry, &lights_scene()).expect("writing the lights scene");
}

#[test]
fn the_committed_lights_scene_is_lit_and_casts_every_kind_of_shadow() {
    let world = committed();
    let lighting = light::gather(&world, Vec2::splat(-8.0), Vec2::splat(8.0));
    assert!(lighting.ambient.is_some(), "the active camera must be lit");
    assert_eq!(lighting.lights.len(), 3);
    assert_eq!(
        lighting
            .lights
            .iter()
            .filter(|l| l.shadow.is_some())
            .count(),
        2,
        "two lamps cast, the ember does not"
    );

    let casters: Vec<&ShadowCaster2D> = world.query::<ShadowCaster2D>().map(|(_, c)| c).collect();
    assert!(casters.iter().any(|c| c.self_shadow), "the crate");
    assert!(
        casters
            .iter()
            .any(|c| matches!(c.shape, CasterShape::Polygon { closed: false, .. })),
        "the wire"
    );
    // Two pillars, the crate and the wire.
    assert_eq!(lighting.occluders.len(), 4, "{:?}", lighting.occluders);
}

/// The scene's frame, `width` × `height`, as the game would draw it: the
/// sprite batch through the active camera, lit, graded neutral. `None`
/// without an adapter.
fn render(world: &World, width: u32, height: u32) -> Option<Vec<Rgba>> {
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    let (device, queue) = headless_device()?;
    let aspect = width as f32 / height as f32;
    let eye = camera::active(world).expect("the scene has an active camera");
    let view = camera::view(world, eye, aspect).expect("the active camera has a view");

    let scene = RenderTarget::new(&device, "scene", FORMAT, width, height, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, width, height, Filter::Linear);
    let binding = CameraBinding::new(&device);
    binding.upload(&queue, &view);
    let layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let sprites = pipeline::create_flat_color(&device, FORMAT, binding.layout(), &layout);

    let batch = SpriteBatch::from_world(world, Sheets::default());
    let mesh = batch.upload(&device);
    let draws: Vec<MeshDraw> = batch
        .ranges
        .iter()
        .map(|range| {
            assert!(range.texture.is_none(), "the scene is untextured");
            MeshDraw {
                texture: &white,
                indices: range.indices.clone(),
            }
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("lights-scene"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        binding.bind_group(),
        mesh.as_ref(),
        &draws,
        wgpu::Color::BLACK,
    );

    let half = view.half_extents();
    let lighting = light::gather(world, view.position - half, view.position + half);
    let lights = LightPass::new(&device, binding.layout());
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut graph = RenderGraph::new();
    let drawn = graph.import(scene.view());
    let target = graph.import_output(output.view());
    let source = lights
        .add(
            &mut graph,
            &device,
            binding.bind_group(),
            drawn,
            width,
            height,
            &lighting,
        )
        .unwrap_or(drawn);
    let (device_ref, queue_ref) = (&device, &queue);
    graph.add_pass("grade", &[source], &[target], move |encoder, views| {
        grade.record(
            device_ref,
            queue_ref,
            encoder,
            views.get(source),
            None,
            views.get(target),
            &PostSettings::NEUTRAL,
            aspect,
        );
    });
    graph
        .run(&device, &mut encoder, &mut pool)
        .expect("a valid graph");
    queue.submit(Some(encoder.finish()));
    Some(read_texture(
        &device,
        &queue,
        output.texture(),
        width,
        height,
    ))
}

/// The pixel at world `at`, in a `width` × `height` frame of the scene's
/// camera (size 4, at the origin).
fn at(pixels: &[Rgba], width: u32, height: u32, at: Vec2) -> Rgba {
    let half = Vec2::new(4.0 * width as f32 / height as f32, 4.0);
    let x = ((at.x / half.x + 1.0) * 0.5 * width as f32) as u32;
    let y = ((1.0 - at.y / half.y) * 0.5 * height as f32) as u32;
    pixels[(y.min(height - 1) * width + x.min(width - 1)) as usize]
}

fn brightness(pixel: Rgba) -> u32 {
    u32::from(pixel.r) + u32::from(pixel.g) + u32::from(pixel.b)
}

#[test]
fn the_lights_scene_draws_its_shadows() {
    const W: u32 = 160;
    const H: u32 = 90;
    let world = committed();
    let Some(lit) = render(&world, W, H) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let mut open = committed();
    let casters: Vec<_> = open.query::<ShadowCaster2D>().map(|(e, _)| e).collect();
    for entity in casters {
        open.remove::<ShadowCaster2D>(entity);
    }
    let unblocked = render(&open, W, H).expect("an adapter, found above");

    // On the wall behind the left pillar, seen from the warm lamp, and out
    // of the ember's reach (which would light it either way).
    let behind = Vec2::new(-1.5, -1.0);
    let (shadowed, clear) = (at(&lit, W, H, behind), at(&unblocked, W, H, behind));
    assert!(
        brightness(shadowed) * 2 < brightness(clear),
        "the pillar's shadow: {shadowed:?} against {clear:?}"
    );
    // Above the wire, nothing is in the way of the warm lamp.
    let open_wall = Vec2::new(-3.5, 3.0);
    let (a, b) = (
        brightness(at(&lit, W, H, open_wall)),
        brightness(at(&unblocked, W, H, open_wall)),
    );
    assert!(
        a.abs_diff(b) <= 6,
        "unshadowed wall unchanged: {a} against {b}"
    );
    // The ember sits in that same shadow and casts none of its own, so its
    // red glow still shows there.
    let glow = at(&lit, W, H, Vec2::new(-0.5, -2.6));
    assert!(
        u32::from(glow.r) > u32::from(shadowed.r) + 40 && glow.r > glow.b,
        "the ember's glow: {glow:?} against the shadow's {shadowed:?}"
    );
}

#[test]
#[ignore = "writes target/lights-preview.png; run with --ignored to look at the scene"]
fn write_the_lights_preview() {
    use image::ImageEncoder;

    const W: u32 = 960;
    const H: u32 = 540;
    let pixels = render(&committed(), W, H).expect("an adapter to render with");
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/lights-preview.png");
    let file = std::fs::File::create(&path).expect("creating the preview");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&bytes, W, H, image::ExtendedColorType::Rgba8)
        .expect("encoding the preview");
}
