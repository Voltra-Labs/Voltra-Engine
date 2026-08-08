//! Renders hand-built egui primitives and inspects the pixels.
//!
//! The backend is ours rather than `egui-wgpu`'s, so nothing upstream is
//! checking it. Every part of it fails silently: a wrong scissor clips the
//! wrong half, a wrong blend factor double-multiplies alpha, a wrong texture
//! format washes every colour out, and a wrong patch origin corrupts glyphs
//! without ever raising a validation error.
//!
//! The primitives are built directly from `epaint`, so these tests exercise the
//! backend without dragging widgets and layout into the render crate.

use epaint::textures::{TextureOptions, TexturesDelta};
use epaint::{
    pos2, ClippedPrimitive, Color32, ColorImage, ImageDelta, Mesh, Primitive, Rect, TextureId,
    Vertex,
};
use voltra_render::egui_backend::{EguiBackend, ScreenDescriptor};
use voltra_render::target::RenderTarget;
use voltra_render::{wgpu, Filter};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
/// sRGB, matching a real swapchain, so the gamma-converting entry point of the
/// shader is the one under test.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

const SCREEN: ScreenDescriptor = ScreenDescriptor {
    width: SIZE,
    height: SIZE,
    pixels_per_point: 1.0,
};

/// Fully transparent, so anything that shows up came from the UI.
const CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

/// A quad covering `rect` in egui points, tinted by `color`.
fn quad(texture_id: TextureId, rect: Rect, color: Color32) -> Mesh {
    let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: pos2(x, y),
        uv: pos2(u, v),
        color,
    };
    Mesh {
        indices: vec![0, 1, 2, 0, 2, 3],
        vertices: vec![
            vertex(rect.min.x, rect.min.y, 0.0, 0.0),
            vertex(rect.min.x, rect.max.y, 0.0, 1.0),
            vertex(rect.max.x, rect.max.y, 1.0, 1.0),
            vertex(rect.max.x, rect.min.y, 1.0, 0.0),
        ],
        texture_id,
    }
}

fn full_screen() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(SIZE as f32, SIZE as f32))
}

/// A one-texture delta, which is the shape of egui's first frame.
fn upload(id: TextureId, image: ColorImage) -> TexturesDelta {
    TexturesDelta {
        set: vec![(id, ImageDelta::full(image, TextureOptions::NEAREST))],
        free: Vec::new(),
    }
}

/// Prepares and draws `primitives`, returning the target's pixels.
fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    backend: &mut EguiBackend,
    target: &RenderTarget,
    primitives: &[ClippedPrimitive],
    delta: &TexturesDelta,
) -> Vec<Rgba> {
    backend.prepare(device, queue, primitives, delta, SCREEN);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("egui-test-encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui-test-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        backend.render(&mut pass);
    }
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, target.texture(), SIZE, SIZE)
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

fn setup() -> Option<(wgpu::Device, wgpu::Queue, EguiBackend, RenderTarget)> {
    let (device, queue) = headless_device()?;
    let backend = EguiBackend::new(&device, FORMAT);
    let target = RenderTarget::new(&device, "egui-test", FORMAT, SIZE, SIZE, Filter::Nearest);
    Some((device, queue, backend, target))
}

#[test]
fn a_textured_quad_keeps_its_colour_end_to_end() {
    let Some((device, queue, mut backend, target)) = setup() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // A mid-grey texel is the interesting case: if the shader treated egui's
    // gamma-encoded bytes as linear, or skipped the conversion before writing
    // to an sRGB target, this comes back nowhere near 128.
    let id = TextureId::Managed(0);
    let delta = upload(
        id,
        ColorImage::filled([1, 1], Color32::from_rgb(128, 200, 40)),
    );
    let primitives = vec![ClippedPrimitive {
        clip_rect: full_screen(),
        primitive: Primitive::Mesh(quad(id, full_screen(), Color32::WHITE)),
    }];

    let pixels = draw(&device, &queue, &mut backend, &target, &primitives, &delta);
    let centre = at(&pixels, SIZE / 2, SIZE / 2);

    let close = |got: u8, want: u8| (got as i32 - want as i32).abs() <= 2;
    assert!(
        close(centre.r, 128) && close(centre.g, 200) && close(centre.b, 40),
        "expected the texel colour back, got {centre:?}"
    );
    assert_eq!(centre.a, 255, "an opaque quad should end up opaque");
}

#[test]
fn the_clip_rect_scissors_the_draw() {
    let Some((device, queue, mut backend, target)) = setup() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let id = TextureId::Managed(0);
    let delta = upload(id, ColorImage::filled([1, 1], Color32::WHITE));

    // The mesh covers the whole screen; the clip rect admits only the left
    // half. Panels rely entirely on this — without it every window paints over
    // its neighbours.
    let primitives = vec![ClippedPrimitive {
        clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(SIZE as f32 / 2.0, SIZE as f32)),
        primitive: Primitive::Mesh(quad(id, full_screen(), Color32::WHITE)),
    }];

    let pixels = draw(&device, &queue, &mut backend, &target, &primitives, &delta);

    let inside = at(&pixels, SIZE / 4, SIZE / 2);
    let outside = at(&pixels, SIZE - SIZE / 4, SIZE / 2);
    assert!(
        inside.r > 200 && inside.a > 200,
        "inside the clip rect should be painted, got {inside:?}"
    );
    assert_eq!(
        outside.a, 0,
        "outside the clip rect should be untouched, got {outside:?}"
    );
}

#[test]
fn a_registered_view_can_be_drawn_as_a_texture() {
    let Some((device, queue, mut backend, target)) = setup() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // This is the viewport path: the scene is rendered to its own target, then
    // handed to egui as an ordinary image to show inside a panel.
    let scene = RenderTarget::new(&device, "scene", FORMAT, 4, 4, Filter::Nearest);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("scene-clear"),
    });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: scene.view(),
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                // Clear colours are linear; an sRGB target encodes them on the
                // way in, so these land as the bytes 128, 64 and 200. Mid-tones
                // on purpose: 0 and 1 survive a stray gamma conversion
                // unchanged and would prove nothing.
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.2159,
                    g: 0.0565,
                    b: 0.5694,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    queue.submit(Some(encoder.finish()));

    // The raw view, not the sRGB one: egui's shader converts gamma itself, and
    // letting the sampler convert as well darkens the whole viewport.
    let id = backend.register_view(&device, scene.raw_view(), Filter::Nearest);
    assert!(
        matches!(id, TextureId::User(_)),
        "an application texture must not collide with egui's own ids, got {id:?}"
    );

    let primitives = vec![ClippedPrimitive {
        clip_rect: full_screen(),
        primitive: Primitive::Mesh(quad(id, full_screen(), Color32::WHITE)),
    }];
    let pixels = draw(
        &device,
        &queue,
        &mut backend,
        &target,
        &primitives,
        &TexturesDelta::default(),
    );

    let centre = at(&pixels, SIZE / 2, SIZE / 2);
    let close = |got: u8, want: u8| (got as i32 - want as i32).abs() <= 3;
    assert!(
        close(centre.r, 128) && close(centre.g, 64) && close(centre.b, 200),
        "the viewport should come through unchanged; a double sRGB conversion \
         would land near (56, 15, 145). Got {centre:?}"
    );
}

#[test]
fn a_partial_delta_patches_the_texture_in_place() {
    let Some((device, queue, mut backend, target)) = setup() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // egui grows its font atlas by re-uploading only the newly rasterised
    // glyphs. Getting the origin wrong writes them over existing ones, which
    // shows up as garbled text and never as an error.
    let id = TextureId::Managed(0);
    let base = upload(id, ColorImage::filled([2, 2], Color32::from_rgb(255, 0, 0)));
    let primitives = vec![ClippedPrimitive {
        clip_rect: full_screen(),
        primitive: Primitive::Mesh(quad(id, full_screen(), Color32::WHITE)),
    }];
    draw(&device, &queue, &mut backend, &target, &primitives, &base);

    let patch = TexturesDelta {
        set: vec![(
            id,
            ImageDelta::partial(
                [1, 1],
                ColorImage::filled([1, 1], Color32::from_rgb(0, 255, 0)),
                TextureOptions::NEAREST,
            ),
        )],
        free: Vec::new(),
    };
    let pixels = draw(&device, &queue, &mut backend, &target, &primitives, &patch);

    // The patched texel is the bottom-right quarter of a 2x2 texture, so it
    // owns the bottom-right quadrant of the quad. The rest must survive.
    let q = SIZE / 4;
    let patched = at(&pixels, SIZE - q, SIZE - q);
    let untouched = at(&pixels, q, q);
    assert!(
        patched.g > 200 && patched.r < 60,
        "the patch should have landed bottom-right, got {patched:?}"
    );
    assert!(
        untouched.r > 200 && untouched.g < 60,
        "the patch overwrote texels it was not given, got {untouched:?}"
    );
}

#[test]
fn nothing_to_draw_is_not_an_error() {
    let Some((device, queue, mut backend, target)) = setup() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // The first frame arrives before any texture exists, and a hidden UI
    // tessellates to nothing. Both must record a pass without touching an
    // empty vertex buffer.
    let pixels = draw(
        &device,
        &queue,
        &mut backend,
        &target,
        &[],
        &TexturesDelta::default(),
    );
    assert!(pixels.iter().all(|p| p.a == 0), "something was drawn");
}
