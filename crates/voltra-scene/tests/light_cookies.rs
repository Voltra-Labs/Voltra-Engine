//! A light's cookie through the real texture store: resolved from its path,
//! worn in the frame by index, each image bound once, and left off when it
//! will not load or there is no store.
//!
//! Skips itself when no GPU adapter is available.

use std::path::Path;

use voltra_assets::{AssetPath, Textures};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::texture;
use voltra_scene::{light, Camera2D, PointLight2D, Transform2D};
use voltra_testkit::{headless_device, scratch_root};

/// A 2×1 PNG at `path`: black on the left, white on the right.
fn write_halves(path: &Path) {
    use image::ImageEncoder;
    let file = std::fs::File::create(path).expect("creating the cookie PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(
            &[0, 0, 0, 255, 255, 255, 255, 255],
            2,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .expect("encoding the cookie PNG");
}

fn lit_world() -> World {
    let mut world = World::new();
    let camera = world.spawn();
    world.insert(camera, Transform2D::default());
    world.insert(
        camera,
        Camera2D {
            ambient: Some([0.1; 3]),
            ..Camera2D::default()
        },
    );
    world
}

fn lamp(world: &mut World, at: Vec2, rotation: f32) -> voltra_ecs::Entity {
    let lamp = world.spawn();
    world.insert(
        lamp,
        Transform2D {
            rotation,
            ..Transform2D::from_translation(at)
        },
    );
    world.insert(lamp, PointLight2D::default());
    lamp
}

const VIEW: (Vec2, Vec2) = (Vec2::new(-10.0, -10.0), Vec2::new(10.0, 10.0));

#[test]
fn lights_wearing_one_image_share_its_index_and_a_missing_one_shines_plain() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    write_halves(&root.join("halves.png"));
    let layout = texture::bind_group_layout(&device);
    let mut textures = Textures::new(&device, &queue, &layout, &root);

    let mut world = lit_world();
    let upright = lamp(&mut world, Vec2::new(-2.0, 0.0), 0.0);
    let turned = lamp(&mut world, Vec2::new(2.0, 0.0), std::f32::consts::FRAC_PI_2);
    let missing = lamp(&mut world, Vec2::new(0.0, 3.0), 0.0);
    let plain = lamp(&mut world, Vec2::new(0.0, -3.0), 0.0);
    let halves = AssetPath::new("halves.png").expect("valid");
    for (entity, path) in [
        (upright, Some(halves.clone())),
        (turned, Some(halves)),
        (
            missing,
            Some(AssetPath::new("no_such_cookie.png").expect("valid")),
        ),
        (plain, None),
    ] {
        let light = world.get_mut::<PointLight2D>(entity).expect("a light");
        light.set_cookie(path, &mut textures, &device, &queue);
    }

    let lighting = light::gather(&world, VIEW.0, VIEW.1, None, Some(&textures));
    assert_eq!(lighting.cookies.len(), 1, "one image, bound once");
    let worn = |at: Vec2| {
        lighting
            .lights
            .iter()
            .find(|l| (l.position - at).length() < 1e-4)
            .expect("gathered")
            .cookie
    };
    let up = worn(Vec2::new(-2.0, 0.0)).expect("worn");
    let side = worn(Vec2::new(2.0, 0.0)).expect("worn");
    assert_eq!((up.index, side.index), (0, 0));
    assert!((up.x - Vec2::X).length() < 1e-5 && (up.y - Vec2::Y).length() < 1e-5);
    assert!(
        (side.x - Vec2::Y).length() < 1e-5,
        "turned with its entity: {side:?}"
    );
    assert_eq!(worn(Vec2::new(0.0, 3.0)), None, "would not load");
    assert_eq!(worn(Vec2::new(0.0, -3.0)), None, "has none");

    // Without a store, every light shines plain.
    let headless = light::gather(&world, VIEW.0, VIEW.1, None, None);
    assert!(headless.cookies.is_empty());
    assert!(headless.lights.iter().all(|l| l.cookie.is_none()));
}

#[test]
fn a_cookie_is_saved_by_path_and_left_out_when_absent() {
    let plain = ron::to_string(&PointLight2D::default()).expect("writes");
    assert!(!plain.contains("cookie"), "{plain}");
    let worn = PointLight2D {
        cookie: Some(AssetPath::new("panes.png").expect("valid")),
        ..PointLight2D::default()
    };
    let text = ron::to_string(&worn).expect("writes");
    let back: PointLight2D = ron::from_str(&text).expect("reads");
    assert_eq!(back.cookie, worn.cookie);
    assert_eq!(
        back.cookie_handle, None,
        "a handle is resolved, never saved"
    );
}
