//! The screen-space pass in pixels: drawn over the world without clearing
//! it, origin top-left, y down, one unit per physical pixel.
//!
//! Skips itself when no GPU adapter is available.

use voltra_ecs::World;
use voltra_render::camera::{screen_projection, Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{ui, Sprite, SpriteBatch, Transform, UiLayout, UiRect};
use voltra_testkit::{headless_device, read_texture, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[test]
fn the_hud_lands_in_pixels_over_a_world_it_does_not_clear() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // A green sprite filling the frame, and a red panel over the top-left
    // quarter — at 2 pixels per point, so 16 points is 32 pixels.
    let mut world = World::new();
    let ground = world.spawn();
    world.insert(ground, Transform::default().with_scale(Vec2::splat(4.0)));
    world.insert(ground, Sprite::new([0.0, 1.0, 0.0, 1.0]));
    let panel = world.spawn();
    world.insert(
        panel,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::splat(16.0)),
    );
    world.insert(panel, Sprite::new([1.0, 0.0, 0.0, 1.0]));

    let scale = 2.0;
    let world_batch = SpriteBatch::from_world(&world, Sheets::default());
    let layout = UiLayout::from_world(&world, Vec2::splat(SIZE as f32) / scale);
    let hud = ui::mesh::batch(&world, Sheets::default(), &layout, scale);
    assert_eq!(
        world_batch.sprite_count(),
        1,
        "the panel is not in the world"
    );
    assert_eq!(hud.sprite_count(), 1);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-ui-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let world_camera = CameraBinding::new(&device);
    world_camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    // A second buffer against the same layout, as `Renderer` keeps one.
    let ui_camera = world_camera.sharing_layout(&device);
    ui_camera.upload_matrix(&queue, screen_projection(SIZE as f32, SIZE as f32));

    let layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let render_pipeline =
        pipeline::create_flat_color(&device, FORMAT, world_camera.layout(), &layout);
    let draws = |batch: &SpriteBatch| -> Vec<MeshDraw<'_>> {
        batch
            .ranges
            .iter()
            .map(|range| MeshDraw {
                texture: &white,
                normal: None,
                light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
                primitives: range.primitives.clone(),
                material: None,
            })
            .collect()
    };

    let world_mesh = world_batch.upload(&device);
    let hud_mesh = hud.upload(&device);
    let world_draws = draws(&world_batch);
    let hud_draws = draws(&hud);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-ui-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        world_camera.bind_group(),
        world_mesh.as_ref(),
        &world_draws,
        CLEAR,
    );
    pass::draw_mesh_batches_over(
        &mut encoder,
        &view,
        &render_pipeline,
        ui_camera.bind_group(),
        hud_mesh.as_ref(),
        &hud_draws,
    );
    queue.submit(Some(encoder.finish()));
    let pixels = read_texture(&device, &queue, &target, SIZE, SIZE);
    let at = |x: u32, y: u32| pixels[(y * SIZE + x) as usize];

    let inside = at(8, 8);
    assert!(
        inside.r > 200 && inside.g < 50,
        "the panel, top-left: {inside:?}"
    );
    let below = at(8, 48);
    assert!(
        below.g > 200 && below.r < 50,
        "the world, untouched below it: {below:?}"
    );
    let right = at(48, 8);
    assert!(right.g > 200 && right.r < 50, "and right of it: {right:?}");
    let edge = at(31, 31);
    assert!(
        edge.r > 200,
        "32 pixels, so the panel's last pixel is 31: {edge:?}"
    );
    let past = at(33, 33);
    assert!(past.g > 200, "and 33 is the world: {past:?}");
}
