//! 2D shadows in pixels, through the graph the renderer builds: a white
//! frame with no ambient, lit by a lamp at its left, with a box between the
//! lamp and the right half. Behind the box is dark, beside it lit; the box
//! itself is lit unless it shadows itself; two lamps each keep their own
//! shadows; a light that casts none ignores the box.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::{
    pass, wgpu, Filter, LightPass, Lighting, Occluder, PointLight, PostPass, PostSettings,
    RenderTarget, Shadow,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A white frame — the camera sees -1..1, 32 pixels a unit — lit by
/// `lighting`, and how many textures the graph's pool ended with.
fn frame(lighting: &Lighting) -> Option<(Vec<Rgba>, usize)> {
    let (device, queue) = headless_device()?;
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
        label: Some("shadows-test"),
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
            &voltra_render::FrameBuffers::default(),
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

/// The pixel at world `(x, y)`.
fn at(pixels: &[Rgba], x: f32, y: f32) -> Rgba {
    let column = ((x + 1.0) * 32.0) as u32;
    let row = ((1.0 - y) * 32.0) as u32;
    pixels[(row.min(SIZE - 1) * SIZE + column.min(SIZE - 1)) as usize]
}

/// A bright lamp reaching the whole frame, casting a hard full shadow.
fn lamp(x: f32, shadow: Option<Shadow>) -> PointLight {
    PointLight {
        position: Vec2::new(x, 0.0),
        color: [1.0; 3],
        intensity: 8.0,
        radius: 4.0,
        falloff: 1.0,
        shadow,
        cone: None,
        height: None,
        layers: voltra_render::ALL_LIGHT_LAYERS,
        cookie: None,
    }
}

const HARD: Option<Shadow> = Some(Shadow {
    strength: 1.0,
    softness: 0.0,
});

/// The box from -0.25 to 0 across, -0.25 to 0.25 up.
fn block(self_shadow: bool) -> Occluder {
    Occluder {
        points: vec![
            Vec2::new(-0.25, -0.25),
            Vec2::new(0.0, -0.25),
            Vec2::new(0.0, 0.25),
            Vec2::new(-0.25, 0.25),
        ],
        closed: true,
        self_shadow,
        layers: voltra_render::ALL_LIGHT_LAYERS,
    }
}

fn dark(lights: Vec<PointLight>, occluders: Vec<Occluder>) -> Lighting {
    Lighting {
        ambient: Some([0.0; 3]),
        lights,
        occluders,
        ..Lighting::default()
    }
}

#[test]
fn behind_the_box_is_dark_and_beside_it_lit() {
    let Some((pixels, pool)) = frame(&dark(vec![lamp(-0.75, HARD)], vec![block(false)])) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 3, "a light map, a lit frame and a shadow map");
    let behind = at(&pixels, 0.6, 0.0);
    let beside = at(&pixels, 0.6, 0.8);
    let front = at(&pixels, -0.5, 0.0);
    assert!(behind.r < 8, "in the shadow: {behind:?}");
    assert!(beside.r > 240, "past the shadow's edge: {beside:?}");
    assert!(front.r > 240, "between the lamp and the box: {front:?}");
    // The shadow widens away from the lamp: its edge is the line through
    // the box's near corners, at y = ±0.25 · 1.65 / 0.5 = ±0.825 by x = 0.9.
    assert!(at(&pixels, 0.9, 0.7).r < 8, "inside the widened shadow");
    assert!(at(&pixels, 0.9, 0.95).r > 240, "outside it");
}

#[test]
fn the_box_is_lit_unless_it_shadows_itself() {
    let inside = (-0.1, 0.0);
    let Some((open, _)) = frame(&dark(vec![lamp(-0.75, HARD)], vec![block(false)])) else {
        return;
    };
    let (closed, _) =
        frame(&dark(vec![lamp(-0.75, HARD)], vec![block(true)])).expect("adapter found above");
    assert!(at(&open, inside.0, inside.1).r > 240, "its lit face");
    assert!(at(&closed, inside.0, inside.1).r < 8, "in its own shadow");
    assert!(at(&closed, 0.6, 0.0).r < 8, "and still casting one");
}

#[test]
fn two_lamps_keep_their_own_shadows() {
    let both = dark(
        vec![lamp(-0.75, HARD), lamp(0.75, HARD)],
        vec![block(false)],
    );
    let Some((pixels, _)) = frame(&both) else {
        return;
    };
    let (left_only, _) =
        frame(&dark(vec![lamp(-0.75, HARD)], vec![block(false)])).expect("adapter");
    // Behind the box from the left lamp, in plain view of the right one.
    assert!(at(&left_only, 0.5, 0.0).r < 8);
    assert!(at(&pixels, 0.5, 0.0).r > 240, "the right lamp reaches it");
    // Behind it from the right lamp, in plain view of the left one.
    assert!(at(&pixels, -0.6, 0.0).r > 240, "the left lamp reaches it");
}

#[test]
fn a_light_that_casts_no_shadow_ignores_the_box() {
    let Some((pixels, pool)) = frame(&dark(vec![lamp(-0.75, None)], vec![block(false)])) else {
        return;
    };
    assert_eq!(pool, 2, "no shadow map");
    assert!(at(&pixels, 0.6, 0.0).r > 240);
}

#[test]
fn a_weak_shadow_takes_part_of_the_light() {
    let half = Some(Shadow {
        strength: 0.5,
        softness: 0.0,
    });
    // Dim enough that half its light is not past white.
    let mut light = lamp(-0.75, half);
    light.intensity = 1.0;
    let Some((pixels, _)) = frame(&dark(vec![light], vec![block(false)])) else {
        return;
    };
    let mut unshadowed = light;
    unshadowed.shadow = None;
    let (plain, _) = frame(&dark(vec![unshadowed], vec![block(false)])).expect("adapter");
    let (shadowed, lit) = (at(&pixels, 0.6, 0.0).r, at(&plain, 0.6, 0.0).r);
    assert!(shadowed > 8 && shadowed < lit, "{shadowed} against {lit}");
}

#[test]
fn a_soft_shadow_fades_across_its_edge() {
    let soft = Some(Shadow {
        strength: 1.0,
        softness: 0.1,
    });
    // Dim, so the fade is not lost past white.
    let dim = |shadow| PointLight {
        intensity: 1.0,
        ..lamp(-0.75, shadow)
    };
    let Some((pixels, _)) = frame(&dark(vec![dim(soft)], vec![block(false)])) else {
        return;
    };
    let (hard, _) = frame(&dark(vec![dim(HARD)], vec![block(false)])).expect("adapter");
    // Down the column at x = 0.9, across the shadow's edge at y ≈ 0.825:
    // hard is lit or dark, soft has a run of pixels in between. 0.1 rad at
    // about 1.9 from the lamp is about ±0.19, six pixels either side.
    let between = |pixels: &[Rgba]| {
        let lit = at(pixels, 0.9, 0.99).r;
        (0..SIZE / 2)
            .map(|row| at(pixels, 0.9, 1.0 - (row as f32 + 0.5) / 32.0).r)
            .filter(|&r| r > 8 && r + 8 < lit)
            .count()
    };
    let (soft_run, hard_run) = (between(&pixels), between(&hard));
    assert!(hard_run <= 1, "{hard_run}");
    assert!(soft_run >= 6, "{soft_run}");
    // And deep inside, still dark.
    assert!(at(&pixels, 0.6, 0.0).r < 8);
}

#[test]
fn every_quarter_turn_casts_and_so_does_the_seam_between_two() {
    // A lamp in the middle, a small box on each side of it and one on the
    // diagonal, where the right quarter's row meets the upper one's.
    let small = |x: f32, y: f32| Occluder {
        points: vec![
            Vec2::new(x - 0.1, y - 0.1),
            Vec2::new(x + 0.1, y - 0.1),
            Vec2::new(x + 0.1, y + 0.1),
            Vec2::new(x - 0.1, y + 0.1),
        ],
        closed: true,
        self_shadow: false,
        layers: voltra_render::ALL_LIGHT_LAYERS,
    };
    let boxes = vec![
        small(0.5, 0.0),
        small(0.0, 0.5),
        small(-0.5, 0.0),
        small(0.0, -0.5),
        small(0.4, 0.4),
    ];
    let Some((pixels, _)) = frame(&dark(vec![lamp(0.0, HARD)], boxes)) else {
        return;
    };
    for (x, y) in [(0.9, 0.0), (0.0, 0.9), (-0.9, 0.0), (0.0, -0.9)] {
        assert!(at(&pixels, x, y).r < 8, "behind the box towards ({x}, {y})");
    }
    // Both sides of the seam behind the diagonal box.
    assert!(at(&pixels, 0.8, 0.75).r < 8);
    assert!(at(&pixels, 0.75, 0.8).r < 8);
    // Between the boxes, lit.
    for (x, y) in [(0.9, -0.45), (-0.45, 0.9), (-0.9, -0.45), (0.45, -0.9)] {
        assert!(at(&pixels, x, y).r > 240, "between the boxes at ({x}, {y})");
    }
}

#[test]
fn a_soft_edge_on_the_seam_between_two_quarters_fades_both_ways() {
    // A wall whose shadow's upper edge runs from the lamp along the
    // diagonal, exactly where the right quarter's row meets the upper one's.
    let wall = Occluder {
        points: vec![Vec2::new(0.3, 0.3), Vec2::new(0.6, -0.3)],
        closed: false,
        self_shadow: false,
        layers: voltra_render::ALL_LIGHT_LAYERS,
    };
    let soft = PointLight {
        position: Vec2::ZERO,
        intensity: 1.0,
        shadow: Some(Shadow {
            strength: 1.0,
            softness: 0.1,
        }),
        ..lamp(0.0, None)
    };
    let Some((pixels, _)) = frame(&dark(vec![soft], vec![wall])) else {
        return;
    };
    // Along y = 0.92 the edge is at x = 0.92: lit to its left (above the
    // diagonal), dark to its right. 0.1 rad at about 1.3 from the lamp is
    // some six pixels of fade each way.
    let y = 0.92;
    let lit = at(&pixels, 0.65, y).r;
    let fading = |range: std::ops::Range<i32>| {
        range
            .map(|column| at(&pixels, (column as f32 + 0.5) / 32.0 - 1.0, y).r)
            .filter(|&r| r > 8 && r + 12 < lit)
            .count()
    };
    // Columns 61 and up are x > 0.92; 52..61 is the side above the diagonal.
    let (above, below) = (fading(52..61), fading(61..64));
    assert!(above >= 3, "the fade reaches above the seam: {above}");
    assert!(below >= 2, "and below it: {below}");
    assert!(at(&pixels, 0.99, 0.5).r < 8, "deep in the shadow");
}

#[test]
fn a_box_shadows_only_the_lamps_sharing_one_of_its_layers() {
    let on = |layers| PointLight {
        layers,
        ..lamp(-0.75, HARD)
    };
    let picky = Occluder {
        layers: 0b10,
        ..block(false)
    };
    let Some((other, _)) = frame(&dark(vec![on(0b01)], vec![picky.clone()])) else {
        return;
    };
    let (shared, _) = frame(&dark(vec![on(0b11)], vec![picky])).expect("adapter, found above");
    assert!(at(&other, 0.6, 0.0).r > 240, "not on its layers: lit");
    assert!(at(&shared, 0.6, 0.0).r < 8, "sharing one: in the shadow");
}
