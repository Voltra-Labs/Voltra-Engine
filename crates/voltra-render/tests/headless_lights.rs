//! 2D lights in pixels, through the graph the renderer builds: a white
//! frame lit by a dim ambient and one lamp is the ambient's grey far from
//! the lamp and bright at it, fading between; unlit, the frame is as drawn
//! and no lighting texture is made.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::{
    pass, wgpu, Cone, Cookie, Filter, LightPass, Lighting, PointLight, PostPass, PostSettings,
    RenderTarget,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A white frame — the camera sees -1..1, 32 pixels a unit — lit by `lighting`, and how
/// many textures the graph's pool ended with.
fn frame(lighting: &Lighting) -> Option<(Vec<Rgba>, usize)> {
    lit_by(|_, _| lighting.clone())
}

/// [`frame`], its lighting made on the frame's device by `lighting` — for
/// lighting that holds GPU resources, as cookies do.
fn lit_by(
    lighting: impl FnOnce(&wgpu::Device, &wgpu::Queue) -> Lighting,
) -> Option<(Vec<Rgba>, usize)> {
    let (device, queue) = headless_device()?;
    let lighting = &lighting(&device, &queue);
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, SIZE, SIZE, Filter::Linear);
    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let sprites = voltra_render::pipeline::create_flat_color(
        &device,
        FORMAT,
        camera.layout(),
        &voltra_render::texture::bind_group_layout(&device),
    );
    let lights = LightPass::new(
        &device,
        camera.layout(),
        &voltra_render::texture::bind_group_layout(&device),
    );
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("lights-test"),
    });
    // Nothing drawn: the clear is the white frame.
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        camera.bind_group(),
        None,
        &[],
        wgpu::Color::WHITE,
    );

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
            lighting,
            None,
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
    Some((
        read_texture(&device, &queue, output.texture(), SIZE, SIZE),
        pool.len(),
    ))
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

/// A quarter-grey ambient: 0.25 linear, 137 in sRGB.
fn dim() -> Lighting {
    Lighting {
        ambient: Some([0.25; 3]),
        ..Lighting::default()
    }
}

#[test]
fn a_lamp_lights_its_disc_and_the_ambient_lights_the_rest() {
    let lamp = PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 1.0,
        radius: 0.25,
        falloff: 1.0,
        shadow: None,
        cone: None,
        height: None,
        layers: voltra_render::ALL_LIGHT_LAYERS,
        cookie: None,
    };
    let Some((pixels, pool)) = frame(&Lighting {
        lights: vec![lamp],
        ..dim()
    }) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 2, "a light map and a lit frame");
    let (centre, between, corner) = (at(&pixels, 32, 32), at(&pixels, 27, 32), at(&pixels, 1, 1));
    assert!(corner.r.abs_diff(137) <= 3, "the ambient alone: {corner:?}");
    assert!(centre.r > 250, "lit past white at the lamp: {centre:?}");
    assert!(
        corner.r < between.r && between.r < centre.r,
        "fading from the lamp: {corner:?} {between:?} {centre:?}"
    );
    // Outside the radius (8 px from the centre) only the ambient is left.
    assert!(at(&pixels, 32, 10).r.abs_diff(137) <= 3);
}

#[test]
fn an_unlit_frame_is_as_drawn_and_makes_no_texture() {
    let Some((pixels, pool)) = frame(&Lighting::default()) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 0);
    assert_eq!(at(&pixels, 1, 1).r, 255);
}

#[test]
fn a_light_that_gives_nothing_leaves_the_ambient() {
    let dark = PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 1.0,
        radius: 0.0,
        falloff: 2.0,
        shadow: None,
        cone: None,
        height: None,
        layers: voltra_render::ALL_LIGHT_LAYERS,
        cookie: None,
    };
    let Some((pixels, _)) = frame(&Lighting {
        lights: vec![dark],
        ..dim()
    }) else {
        return;
    };
    assert_eq!(at(&pixels, 32, 32), at(&pixels, 1, 1));
}

/// A lamp at the centre reaching past the frame's corners, pointing right
/// through a quarter-turn cone.
fn spot(softness: f32) -> PointLight {
    PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 1.0,
        radius: 2.0,
        falloff: 1.0,
        shadow: None,
        cone: Some(Cone {
            direction: Vec2::X,
            angle: std::f32::consts::FRAC_PI_2,
            softness,
        }),
        height: None,
        layers: voltra_render::ALL_LIGHT_LAYERS,
        cookie: None,
    }
}

#[test]
fn a_spot_light_lights_its_cone_and_leaves_the_rest_to_the_ambient() {
    let Some((pixels, _)) = frame(&Lighting {
        lights: vec![spot(0.0)],
        ..dim()
    }) else {
        return;
    };
    let ambient = at(&pixels, 1, 1);
    // Ahead of it, inside the cone.
    assert!(
        at(&pixels, 50, 32).r > ambient.r + 40,
        "ahead: {:?}",
        at(&pixels, 50, 32)
    );
    // Behind it, and beside it past the cone's 45° edge.
    assert!(at(&pixels, 14, 32).r.abs_diff(ambient.r) <= 3, "behind");
    assert!(
        at(&pixels, 36, 10).r.abs_diff(ambient.r) <= 3,
        "beside, outside"
    );
    // Just inside the edge (about 35° off the axis).
    assert!(at(&pixels, 50, 20).r > ambient.r + 20, "inside the edge");
}

#[test]
fn a_soft_cone_fades_towards_its_edge() {
    let Some((hard, _)) = frame(&Lighting {
        lights: vec![spot(0.0)],
        ..dim()
    }) else {
        return;
    };
    let (soft, _) = frame(&Lighting {
        lights: vec![spot(1.0)],
        ..dim()
    })
    .expect("adapter, found above");
    // About 35° off the axis, at the same distance: the soft cone has
    // already faded most of the way, the hard one not at all.
    let (h, s) = (at(&hard, 50, 20).r, at(&soft, 50, 20).r);
    assert!(s < h, "{s} against {h}");
    // On the axis both are the same, give or take the half pixel the
    // sample sits off it.
    assert!(at(&hard, 50, 32).r.abs_diff(at(&soft, 50, 32).r) <= 2);
}

#[test]
fn a_wide_spot_light_draws_round_behind_its_centre() {
    // Three quarters of a turn pointing up: its quad is the box the cone
    // reaches, which runs below the centre on both sides. Straight down is
    // the quarter it leaves out.
    let wide = PointLight {
        cone: Some(Cone {
            direction: Vec2::Y,
            angle: 1.5 * std::f32::consts::PI,
            softness: 0.0,
        }),
        ..spot(0.0)
    };
    let Some((pixels, _)) = frame(&Lighting {
        lights: vec![wide],
        ..dim()
    }) else {
        return;
    };
    let (unlit, _) = frame(&dim()).expect("adapter, found above");
    // The corners above are inside this cone; the ambient is a frame with no
    // light in it.
    let ambient = at(&unlit, 1, 1).r;
    // Either side, about 17° below the horizontal: inside its edges, which
    // are 45° below it.
    assert!(
        at(&pixels, 12, 38).r > ambient + 20,
        "left, behind the centre"
    );
    assert!(
        at(&pixels, 52, 38).r > ambient + 20,
        "right, behind the centre"
    );
    // Straight below: outside.
    assert!(at(&pixels, 32, 52).r.abs_diff(ambient) <= 3, "below");
}

/// A lamp at the centre reaching past the frame's corners, wearing the
/// cookie `texels` (`width` across, one row) laid along `x` and `y`.
fn wearing(texels: &'static [u8], width: u32, x: Vec2, y: Vec2) -> Option<Vec<Rgba>> {
    lit_by(|device, queue| {
        let layout = voltra_render::texture::bind_group_layout(device);
        let image = voltra_render::Texture::from_rgba8(
            device,
            queue,
            "cookie",
            texels,
            width,
            1,
            Filter::Nearest,
        )
        .expect("the texels fill the image");
        Lighting {
            lights: vec![PointLight {
                cookie: Some(Cookie { index: 0, x, y }),
                ..round()
            }],
            cookies: vec![image.create_bind_group(device, &layout)],
            ..dim()
        }
    })
    .map(|(pixels, _)| pixels)
}

/// [`spot`] shining every way.
fn round() -> PointLight {
    PointLight {
        cone: None,
        ..spot(0.0)
    }
}

/// Black on the image's left, white on its right.
const HALVES: &[u8] = &[0, 0, 0, 255, 255, 255, 255, 255];

#[test]
fn a_cookie_shapes_the_light_along_the_light_s_axes() {
    let Some(upright) = wearing(HALVES, 2, Vec2::X, Vec2::Y) else {
        return;
    };
    let ambient = frame(&dim()).expect("adapter, found above").0;
    let ambient = at(&ambient, 1, 1).r;
    assert!(
        at(&upright, 48, 32).r > ambient + 40,
        "the white half lights"
    );
    assert!(
        at(&upright, 16, 32).r.abs_diff(ambient) <= 3,
        "the black half does not"
    );

    // Turned half a turn with its light: the halves swap sides.
    let turned = wearing(HALVES, 2, Vec2::NEG_X, Vec2::NEG_Y).expect("adapter, found above");
    assert!(at(&turned, 16, 32).r > ambient + 40);
    assert!(at(&turned, 48, 32).r.abs_diff(ambient) <= 3);
}

#[test]
fn a_cookie_tints_the_light_by_its_colour() {
    let Some(red) = wearing(&[255, 0, 0, 255], 1, Vec2::X, Vec2::Y) else {
        return;
    };
    let ambient = frame(&dim()).expect("adapter, found above").0;
    let (lit, ambient) = (at(&red, 40, 32), at(&ambient, 1, 1));
    assert!(lit.r > ambient.r + 40, "red comes through: {lit:?}");
    assert!(lit.g.abs_diff(ambient.g) <= 3, "green does not: {lit:?}");
}

#[test]
fn a_cookie_the_frame_does_not_hold_is_no_cookie() {
    let missing = Lighting {
        lights: vec![PointLight {
            cookie: Some(Cookie {
                index: 3,
                x: Vec2::X,
                y: Vec2::Y,
            }),
            ..round()
        }],
        ..dim()
    };
    let Some((pixels, _)) = frame(&missing) else {
        return;
    };
    let plain = frame(&Lighting {
        lights: vec![round()],
        ..dim()
    })
    .expect("adapter, found above")
    .0;
    assert_eq!(at(&pixels, 16, 32), at(&plain, 16, 32));
    assert_eq!(at(&pixels, 48, 32), at(&plain, 48, 32));
}
