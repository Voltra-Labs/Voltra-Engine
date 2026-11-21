//! Normal maps in pixels: a white quad filling the frame, its normal map two
//! texels wide, lit by one lamp in the middle a little above the scene and
//! no ambient. Normals turned towards the lamp are lit, turned away dark; a
//! mirrored quad mirrors its normals with it; a flat range faces the screen;
//! a light with no height ignores normal maps.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::normals::{NormalPass, NORMAL_FORMAT};
use voltra_render::{
    pass, pipeline, texture, wgpu, Filter, LightPass, Lighting, Mesh, MeshDraw, PointLight,
    PostPass, PostSettings, RenderTarget, SceneDraw, Texture, Vertex,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A normal tilted 45° along ±x, as a normal map stores it.
const RIGHT: [u8; 4] = [218, 128, 218, 255];
const LEFT: [u8; 4] = [38, 128, 218, 255];

/// What normal map the quad gets.
#[derive(Clone, Copy)]
enum Surface {
    /// None at all: the flat stand-in.
    Flat,
    /// Left texel, then right, as given.
    Map([u8; 4], [u8; 4]),
}

/// The quad covering the camera's -1..1, its UVs mirrored left to right when
/// `mirrored` — a flipped sprite.
fn quad(device: &wgpu::Device, mirrored: bool) -> Mesh {
    let (u0, u1) = if mirrored { (1.0, 0.0) } else { (0.0, 1.0) };
    let white = [1.0; 4];
    let vertices = [
        Vertex::new([-1.0, 1.0], white, [u0, 0.0]),
        Vertex::new([-1.0, -1.0], white, [u0, 1.0]),
        Vertex::new([1.0, -1.0], white, [u1, 1.0]),
        Vertex::new([1.0, 1.0], white, [u1, 0.0]),
    ];
    Mesh::indexed(device, "normals-quad", &vertices, &[0, 1, 2, 2, 3, 0])
}

/// The frame, lit by `lamp` with no ambient, the quad drawn with `surface`.
fn frame(surface: Surface, mirrored: bool, lamp: PointLight) -> Option<Vec<Rgba>> {
    let (device, queue) = headless_device()?;
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, SIZE, SIZE, Filter::Linear);
    let normals = RenderTarget::new(
        &device,
        "normals",
        NORMAL_FORMAT,
        SIZE,
        SIZE,
        Filter::Nearest,
    );
    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let layout = texture::bind_group_layout(&device);
    let sprites = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let map = match surface {
        Surface::Flat => None,
        Surface::Map(left, right) => {
            let pixels: Vec<u8> = left.into_iter().chain(right).collect();
            let texture = Texture::from_rgba8(
                &device,
                &queue,
                "normal-map",
                &pixels,
                2,
                1,
                Filter::Nearest,
            )
            .expect("2x1 pixels");
            Some(texture.create_raw_bind_group(&device, &layout))
        }
    };
    let mesh = quad(&device, mirrored);
    let draws = [SceneDraw::Mesh(MeshDraw {
        texture: &white,
        normal: map.as_ref(),
        light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
        indices: 0..6,
    })];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("normals-test"),
    });
    let mesh_draws = [MeshDraw {
        texture: &white,
        normal: None,
        light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
        indices: 0..6,
    }];
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        camera.bind_group(),
        Some(&mesh),
        &mesh_draws,
        wgpu::Color::BLACK,
    );
    let normal_pass = NormalPass::new(&device, &queue, camera.layout(), &layout);
    normal_pass.record(
        &mut encoder,
        normals.view(),
        camera.bind_group(),
        Some(&mesh),
        &draws,
    );

    let lighting = Lighting {
        ambient: Some([0.0; 3]),
        lights: vec![lamp],
        ..Lighting::default()
    };
    let lights = LightPass::new(&device, camera.layout());
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut graph = RenderGraph::new();
    let drawn = graph.import(scene.view());
    let target = graph.import_output(output.view());
    let source = lights
        .add(
            &mut graph,
            &device,
            camera.bind_group(),
            drawn,
            SIZE,
            SIZE,
            &lighting,
            Some(normals.view()),
            None,
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
            1.0,
        );
    });
    graph
        .run(&device, &mut encoder, &mut pool)
        .expect("a valid graph");
    queue.submit(Some(encoder.finish()));
    Some(read_texture(&device, &queue, output.texture(), SIZE, SIZE))
}

/// A lamp in the middle, `height` above the scene, dim enough not to clip.
fn lamp(height: Option<f32>) -> PointLight {
    PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 0.8,
        radius: 2.0,
        falloff: 1.0,
        shadow: None,
        cone: None,
        height,
        layers: voltra_render::ALL_LIGHT_LAYERS,
    }
}

/// Brightness half a unit left and right of the lamp.
fn sides(pixels: &[Rgba]) -> (u8, u8) {
    let row = (SIZE / 2) as usize;
    let at = |x: u32| pixels[row * SIZE as usize + x as usize].r;
    (at(SIZE / 4), at(3 * SIZE / 4))
}

const TOWARDS: Surface = Surface::Map(RIGHT, LEFT);
const AWAY: Surface = Surface::Map(LEFT, RIGHT);

#[test]
fn normals_turned_towards_the_lamp_are_lit_and_turned_away_are_dark() {
    let Some(towards) = frame(TOWARDS, false, lamp(Some(0.2))) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let away = frame(AWAY, false, lamp(Some(0.2))).expect("adapter, found above");
    let (tl, tr) = sides(&towards);
    let (al, ar) = sides(&away);
    assert!(tl > 100 && tr > 100, "facing the lamp: {tl} {tr}");
    assert!(al < 10 && ar < 10, "facing away: {al} {ar}");
}

#[test]
fn a_mirrored_quad_mirrors_its_normals_with_it() {
    // Mirrored, each side shows the other texel, and +u now runs leftward:
    // the texel that pointed along +u (away, on the right) is on the left
    // pointing left — still away. A frame that ignored the flip would read
    // it as pointing right, at the lamp, and light it.
    let Some(away) = frame(AWAY, true, lamp(Some(0.2))) else {
        return;
    };
    let towards = frame(TOWARDS, true, lamp(Some(0.2))).expect("adapter");
    let (al, ar) = sides(&away);
    let (tl, tr) = sides(&towards);
    assert!(al < 10 && ar < 10, "still facing away: {al} {ar}");
    assert!(tl > 100 && tr > 100, "still facing the lamp: {tl} {tr}");
}

#[test]
fn a_flat_range_faces_the_screen_and_a_light_with_no_height_ignores_normals() {
    let Some(flat) = frame(Surface::Flat, false, lamp(Some(0.2))) else {
        return;
    };
    let plain = frame(Surface::Flat, false, lamp(None)).expect("adapter");
    let ignored = frame(AWAY, false, lamp(None)).expect("adapter");
    let towards = frame(TOWARDS, false, lamp(Some(0.2))).expect("adapter");
    // Facing the screen, a light low beside it reaches the flat quad at a
    // slant: dimmer than with no height, brighter than normals facing away
    // and dimmer than normals turned to it.
    let (f, p, t) = (sides(&flat).0, sides(&plain).0, sides(&towards).0);
    assert!(
        f > 10 && f < p && f < t,
        "flat {f}, no height {p}, towards {t}"
    );
    assert_eq!(sides(&ignored), sides(&plain), "no height ignores the map");
}
