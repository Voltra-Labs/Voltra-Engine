use voltra_assets::{Atlases, Handle, Silhouettes};
use voltra_ecs::World;
use voltra_render::glam::{UVec2, Vec2};
use voltra_render::Texture;
use voltra_testkit::scratch_root;

use super::*;
use crate::hierarchy::WorldTransforms;
use crate::shadow_caster::occluders;
use crate::{ShadowCaster2D, Transform};

/// Every texture is 8×8.
struct Eight;

impl voltra_assets::TextureSizes for Eight {
    fn size(&self, _texture: Handle<Texture>) -> Option<UVec2> {
        Some(UVec2::splat(8))
    }

    fn placeholder(&self) -> Handle<Texture> {
        Handle::forge(0, 0)
    }
}

/// An 8×8 texture opaque in its left half, a sprite showing it, and a store
/// that has traced it.
fn half_sprite(flip_x: bool) -> (World, Silhouettes, Atlases) {
    let root = scratch_root();
    // Left half opaque: four columns of eight.
    let pixels: Vec<u8> = (0..64u32)
        .flat_map(|i| [9, 9, 9, if i % 8 < 4 { 255 } else { 0 }])
        .collect();
    let path = root.join("sprites/half.png");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    {
        use image::ImageEncoder;
        let file = std::fs::File::create(&path).expect("file");
        image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
            .write_image(&pixels, 8, 8, image::ExtendedColorType::Rgba8)
            .expect("png");
    }
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(Vec2::new(10.0, 0.0)));
    world.insert(
        entity,
        Sprite {
            texture: Some(AssetPath::new("sprites/half.png").expect("valid")),
            texture_handle: Some(Handle::forge(7, 0)),
            flip_x,
            ..Sprite::default()
        },
    );
    world.insert(
        entity,
        ShadowCaster2D {
            shape: CasterShape::Sprite {
                alpha: voltra_assets::silhouette::ALPHA_THRESHOLD,
            },
            self_shadow: false,
        },
    );
    (world, Silhouettes::new(root), Atlases::new(scratch_root()))
}

fn traced_bounds(flip_x: bool) -> (Vec2, Vec2) {
    let (world, mut silhouettes, atlases) = half_sprite(flip_x);
    let sheets = Sheets::new(&atlases, &Eight);
    prepare(&world, sheets, &mut silhouettes);
    let shapes = SpriteShapes {
        sheets,
        silhouettes: &silhouettes,
    };
    let found = occluders(&world, &WorldTransforms::from_world(&world), Some(shapes));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].closed);
    found[0].points.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    )
}

#[test]
fn a_sprite_casts_the_outline_of_its_opaque_texels_where_it_is_drawn() {
    // The unit quad around (10, 0); its left half is opaque.
    let (lo, hi) = traced_bounds(false);
    assert!((lo - Vec2::new(9.5, -0.5)).length() < 1e-4, "{lo}");
    assert!((hi - Vec2::new(10.0, 0.5)).length() < 1e-4, "{hi}");
}

#[test]
fn a_flipped_sprite_casts_its_mirrored_outline() {
    let (lo, hi) = traced_bounds(true);
    assert!((lo - Vec2::new(10.0, -0.5)).length() < 1e-4, "{lo}");
    assert!((hi - Vec2::new(10.5, 0.5)).length() < 1e-4, "{hi}");
}

#[test]
fn without_silhouettes_or_a_texture_a_sprite_caster_casts_nothing() {
    let (mut world, _, _) = half_sprite(false);
    assert!(occluders(&world, &WorldTransforms::from_world(&world), None).is_empty());
    let atlases = Atlases::new(scratch_root());
    let silhouettes = Silhouettes::new(scratch_root());
    let entity = world
        .query::<Sprite>()
        .map(|(e, _)| e)
        .next()
        .expect("the sprite");
    if let Some(sprite) = world.get_mut::<Sprite>(entity) {
        sprite.texture = None;
    }
    let shapes = SpriteShapes {
        sheets: Sheets::new(&atlases, &Eight),
        silhouettes: &silhouettes,
    };
    assert!(occluders(&world, &WorldTransforms::from_world(&world), Some(shapes)).is_empty());
}

#[test]
fn a_frame_that_is_not_there_casts_nothing_rather_than_the_whole_sheet() {
    let (mut world, mut silhouettes, atlases) = half_sprite(false);
    let entity = world
        .query::<Sprite>()
        .map(|(e, _)| e)
        .next()
        .expect("the sprite");
    if let Some(sprite) = world.get_mut::<Sprite>(entity) {
        // Far outside the 8×8 texture.
        sprite.region = Some(voltra_assets::Frame {
            x: 100,
            y: 100,
            width: 4,
            height: 4,
        });
    }
    let sheets = Sheets::new(&atlases, &Eight);
    prepare(&world, sheets, &mut silhouettes);
    let shapes = SpriteShapes {
        sheets,
        silhouettes: &silhouettes,
    };
    assert!(occluders(&world, &WorldTransforms::from_world(&world), Some(shapes)).is_empty());
}
