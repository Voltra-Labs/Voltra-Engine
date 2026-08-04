//! Proves the offscreen render target is a usable attachment and that resizing
//! it behaves.
//!
//! The editor viewport draws the scene here instead of straight to the
//! swapchain, so a panel can show it as an image. Everything about that path is
//! invisible from the API side — a target can be created, bound and drawn to
//! with no validation error and still hand back an empty texture.

mod common;

use common::{headless_device, read_texture, CLEAR};
use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::mesh::{self, Mesh};
use voltra_render::target::RenderTarget;
use voltra_render::texture::{self, Filter, Texture};
use voltra_render::{pass, pipeline, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[test]
fn target_reports_the_size_it_was_built_with() {
    let Some((device, _queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let target = RenderTarget::new(&device, "viewport", FORMAT, 320, 160, Filter::Linear);

    assert_eq!(target.width(), 320);
    assert_eq!(target.height(), 160);
    assert_eq!(target.format(), FORMAT);
    assert!((target.aspect() - 2.0).abs() < 1e-6);
}

#[test]
fn resizing_to_the_same_size_does_not_reallocate() {
    let Some((device, _queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let mut target = RenderTarget::new(&device, "viewport", FORMAT, 64, 64, Filter::Linear);

    // A docked viewport calls resize with its panel rect every single frame.
    // Reallocating on each of those would churn a texture per frame for
    // nothing, and would invalidate any bind group holding the old view.
    assert!(!target.resize(&device, 64, 64), "same size reallocated");
    assert!(
        target.resize(&device, 64, 32),
        "new size did not reallocate"
    );
    assert_eq!((target.width(), target.height()), (64, 32));
}

#[test]
fn zero_size_is_clamped_to_one_pixel() {
    let Some((device, _queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // wgpu rejects a zero-extent texture outright, and a collapsed or dragged
    // panel reports exactly that. Clamping keeps it a non-event.
    let target = RenderTarget::new(&device, "collapsed", FORMAT, 0, 0, Filter::Linear);
    assert_eq!((target.width(), target.height()), (1, 1));

    let mut target = RenderTarget::new(&device, "viewport", FORMAT, 32, 32, Filter::Linear);
    assert!(target.resize(&device, 0, 24));
    assert_eq!((target.width(), target.height()), (1, 24));
}

#[test]
fn the_scene_ends_up_in_the_target_texture() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let size = 64;
    let target = RenderTarget::new(&device, "viewport", FORMAT, size, size, Filter::Linear);

    let camera_binding = CameraBinding::new(&device);
    camera_binding.upload(&queue, &Camera2D::default());
    let texture_layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &texture_layout);
    let render_pipeline =
        pipeline::create_flat_color(&device, FORMAT, camera_binding.layout(), &texture_layout);
    let quad = Mesh::indexed(&device, "quad", &mesh::QUAD, &mesh::QUAD_INDICES);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("target-encoder"),
    });
    pass::draw_mesh(
        &mut encoder,
        target.view(),
        &render_pipeline,
        camera_binding.bind_group(),
        &white,
        Some(&quad),
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    let pixels = read_texture(&device, &queue, target.texture(), size, size);
    let survivors = pixels.iter().filter(|p| p.is_clear_ish()).count();
    assert_eq!(
        survivors, 0,
        "{survivors} pixels still hold the clear colour; the target was not drawn into"
    );
}

#[test]
fn the_target_can_be_sampled_afterwards() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    // The viewport panel samples the target as an ordinary texture. Missing
    // TEXTURE_BINDING usage only shows up here, as a validation failure when
    // the bind group is built.
    let target = RenderTarget::new(&device, "viewport", FORMAT, 8, 8, Filter::Linear);
    let layout = texture::bind_group_layout(&device);
    let _bind_group = target.create_bind_group(&device, &layout);

    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");
    let _ = &queue;
}
