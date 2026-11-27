//! A camera drawing into a render texture frames by the texture's shape,
//! not the window's. Skips itself when no GPU adapter is available.

use voltra_assets::{AssetPath, Textures};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::texture;
use voltra_scene::camera::{self, CameraTarget};
use voltra_scene::{camera_rig, Camera2D, CameraFollow2D, FollowZoom, SceneId, Transform2D};
use voltra_testkit::{headless_device, scratch_root};

#[test]
fn a_render_texture_camera_zooms_to_fit_its_texture() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    std::fs::write(
        root.join("map.rendertexture.ron"),
        "(version: 1, width: 64, height: 16, nearest: true)",
    )
    .expect("the render texture");
    let layout = texture::bind_group_layout(&device);
    let mut textures = Textures::new(&device, &queue, &layout, &root);

    let mut world = World::new();
    let mut ids = Vec::new();
    for x in [-8.0, 8.0] {
        let e = world.spawn();
        let id = SceneId::new();
        world.insert(e, id);
        world.insert(e, Transform2D::from_translation(Vec2::new(x, 0.0)));
        ids.push(id);
    }
    let eye = world.spawn();
    world.insert(eye, SceneId::new());
    world.insert(eye, Transform2D::default());
    world.insert(eye, Camera2D::new(1.0));
    world.insert(
        eye,
        CameraTarget::new(AssetPath::new("map.rendertexture.ron").expect("a path")),
    );
    world.insert(
        eye,
        CameraFollow2D {
            smoothing: 0.0,
            zoom: Some(FollowZoom {
                margin: Vec2::ZERO,
                min_size: 0.5,
                max_size: 50.0,
            }),
            ..CameraFollow2D::framing(ids)
        },
    );
    camera::target::resolve_world(&mut world, &mut textures, &device, &queue);

    assert_eq!(camera::aspect(&world, eye, 1.0, Some(&textures)), 4.0);
    // A square window, a 4:1 texture: sixteen wide needs a size of two.
    camera_rig::update(&mut world, 1.0 / 60.0, 1.0, Some(&textures));
    let size = world.get::<Camera2D>(eye).expect("a camera").size;
    assert!((size - 2.0).abs() < 1e-4, "{size}");
}
