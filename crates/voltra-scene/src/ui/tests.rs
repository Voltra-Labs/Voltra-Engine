use voltra_assets::{Fonts, Handle, DEFAULT_FONT};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_testkit::{copy_bundled_font, scratch_root};

use super::*;
use crate::batch::SpriteBatch;
use crate::pick::entity_at;
use crate::sprite::sheets::Sheets;
use crate::text::{prepare, resolve_fonts, ui_px};
use crate::{hierarchy, Align, SceneId, Sprite, Text, Transform};

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

fn element(world: &mut World, rect: UiRect) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, rect);
    entity
}

fn fonts() -> Fonts {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    Fonts::new(root.keep())
}

/// Resolved, prepared at `scale` and given a stand-in atlas texture.
fn ready(world: &mut World, fonts: &mut Fonts, scale: f32) {
    resolve_fonts(world, fonts);
    prepare(world, fonts, scale);
    fonts.forge_texture(Handle::forge(7, 0));
}

fn sheets(fonts: &Fonts) -> Sheets<'_> {
    Sheets {
        fonts: Some(fonts),
        ..Sheets::default()
    }
}

/// The smallest box holding every vertex of `batch`.
fn extent(batch: &SpriteBatch) -> (Vec2, Vec2) {
    batch.quad_vertices().iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(min, max), v| {
            let p = Vec2::from(v.position);
            (min.min(p), max.max(p))
        },
    )
}

#[test]
fn a_sprite_fills_its_rect_in_physical_pixels() {
    let mut world = World::new();
    let panel = element(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(100.0, 50.0)),
    );
    world.insert(panel, Sprite::default());

    let layout = UiLayout::from_world(&world, SCREEN);
    let batch = mesh::batch(&world, Sheets::default(), &layout, 2.0);

    assert_eq!(batch.sprite_count(), 1);
    assert_eq!(extent(&batch), (Vec2::splat(20.0), Vec2::new(220.0, 120.0)));
}

#[test]
fn ui_elements_are_not_drawn_in_the_world() {
    let mut world = World::new();
    let panel = element(&mut world, UiRect::default());
    world.insert(panel, Transform::default());
    world.insert(panel, Sprite::default());
    let world_sprite = world.spawn();
    world.insert(world_sprite, Transform::default());
    world.insert(world_sprite, Sprite::default());

    let batch = SpriteBatch::from_world(&world, Sheets::default());
    assert_eq!(batch.sprite_count(), 1, "only the world sprite");
}

#[test]
fn ui_elements_are_not_picked_in_the_world() {
    let mut world = World::new();
    let panel = element(&mut world, UiRect::default());
    world.insert(panel, Transform::default());
    world.insert(panel, Sprite::default());
    assert_eq!(entity_at(&world, Vec2::ZERO, Sheets::default()), None);
}

#[test]
fn text_sits_inside_its_rect_below_the_top() {
    let mut fonts = fonts();
    let mut world = World::new();
    let label = element(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::new(16.0, 16.0), Vec2::new(300.0, 40.0)),
    );
    world.insert(
        label,
        Text {
            size: 24.0,
            ..Text::new("Coins 0")
        },
    );
    ready(&mut world, &mut fonts, 1.0);

    let layout = UiLayout::from_world(&world, SCREEN);
    let batch = mesh::batch(&world, sheets(&fonts), &layout, 1.0);
    let (min, max) = extent(&batch);

    assert!(batch.sprite_count() >= 6, "one quad per inked glyph");
    assert!(
        min.x >= 16.0 && min.x < 20.0,
        "starts at the left edge: {min}"
    );
    assert!(min.y >= 16.0, "no glyph pokes above the rect: {min}");
    assert!(max.y < 16.0 + 32.0, "and the line fits in it: {max}");
}

#[test]
fn right_aligned_text_ends_at_the_right_edge() {
    let mut fonts = fonts();
    let mut world = World::new();
    let label = element(
        &mut world,
        UiRect::pinned(
            Vec2::new(1.0, 0.0),
            Vec2::new(-16.0, 16.0),
            Vec2::new(300.0, 40.0),
        ),
    );
    world.insert(
        label,
        Text {
            size: 24.0,
            align: Align::Right,
            ..Text::new("HH")
        },
    );
    ready(&mut world, &mut fonts, 1.0);

    let layout = UiLayout::from_world(&world, SCREEN);
    let (_, max) = extent(&mesh::batch(&world, sheets(&fonts), &layout, 1.0));
    assert!(max.x <= 784.0 && max.x > 770.0, "{max}");
}

#[test]
fn text_is_rasterized_at_its_on_screen_size() {
    let text = Text {
        size: 20.0,
        pixels_per_em: 48,
        ..Text::default()
    };
    assert_eq!(ui_px(&text, 1.0), 20);
    assert_eq!(
        ui_px(&text, 1.5),
        30,
        "the scale factor is pixels per point"
    );
    assert_eq!(
        ui_px(
            &Text {
                size: 0.0,
                ..text.clone()
            },
            1.0
        ),
        1,
        "never zero"
    );
}

#[test]
fn a_ui_text_prepares_glyphs_at_its_screen_size_not_pixels_per_em() {
    let mut fonts = fonts();
    let mut world = World::new();
    let label = element(&mut world, UiRect::default());
    world.insert(
        label,
        Text {
            size: 20.0,
            pixels_per_em: 48,
            ..Text::new("H")
        },
    );
    resolve_fonts(&mut world, &mut fonts);
    prepare(&world, &mut fonts, 1.5);

    let font = fonts.try_get(fonts.default_font()).expect("resolves");
    let face = font.face().expect("a face");
    let key = voltra_assets::fonts::GlyphKey {
        font: fonts.default_font().index(),
        glyph: face.lookup_glyph_index('H'),
        px: 30,
        embolden: false,
    };
    assert!(fonts.atlas().contains(key));
    assert_eq!(fonts.atlas().len(), 1, "and not at 48 as well");
}

#[test]
fn a_childs_sprite_draws_over_its_panels_and_text_over_its_own_sprite() {
    let mut fonts = fonts();
    let mut world = World::new();
    let button = element(&mut world, UiRect::stretched(0.0));
    world.insert(button, Sprite::default());
    world.insert(button, Text::new("H"));
    let panel = element(&mut world, UiRect::stretched(0.0));
    world.insert(panel, Sprite::new([0.0, 0.0, 1.0, 1.0]));
    hierarchy::set_parent(&mut world, button, panel).expect("parents");
    ready(&mut world, &mut fonts, 1.0);

    let layout = UiLayout::from_world(&world, SCREEN);
    let batch = mesh::batch(&world, sheets(&fonts), &layout, 1.0);

    let textures: Vec<_> = batch.ranges.iter().map(|r| r.texture).collect();
    assert_eq!(textures, vec![None, Some(Handle::forge(7, 0))]);
    let first = batch.quad_vertices()[0].color;
    assert_eq!(
        first,
        [0.0, 0.0, 1.0, 1.0],
        "the panel is first, under the rest"
    );
}

#[test]
fn the_topmost_element_under_a_point_is_picked() {
    let mut world = World::new();
    let panel = element(&mut world, UiRect::stretched(0.0));
    world.insert(panel, Sprite::default());
    let badge = element(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::splat(50.0)),
    );
    world.insert(badge, Sprite::default().with_sort_order(1));
    let layout = UiLayout::from_world(&world, SCREEN);

    assert_eq!(element_at(&world, &layout, Vec2::splat(10.0)), Some(badge));
    assert_eq!(element_at(&world, &layout, Vec2::splat(100.0)), Some(panel));
    assert_eq!(element_at(&world, &layout, Vec2::splat(-1.0)), None);
}

#[test]
fn a_rect_with_nothing_to_draw_is_not_picked() {
    let mut world = World::new();
    element(&mut world, UiRect::stretched(0.0));
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(element_at(&world, &layout, Vec2::splat(10.0)), None);
}

#[test]
fn the_shipped_sandbox_hud_loads_as_a_ui_rect() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenes/sandbox.ron");
    let mut world = World::new();
    crate::format::load(
        &path,
        &crate::ComponentRegistry::with_defaults(),
        &mut world,
    )
    .expect("the sandbox loads");

    let (hint, rect) = world.query::<UiRect>().next().expect("one HUD element");
    assert_eq!(rect.anchor_min, Vec2::new(1.0, 0.0));
    assert!(
        world.get::<crate::UnknownComponents>(hint).is_none(),
        "every component in it is registered"
    );
}

#[test]
fn a_ui_rect_survives_a_save_and_load() {
    let registry = crate::ComponentRegistry::with_defaults();
    let mut world = World::new();
    let rect = UiRect::pinned(
        Vec2::new(0.5, 1.0),
        Vec2::new(0.0, -8.0),
        Vec2::new(120.0, 30.0),
    );
    element(&mut world, rect);

    let file = crate::format::to_scene_file(&world, &registry).expect("saves");
    let text = ron::to_string(&file).expect("serialises");
    let file: crate::format::SceneFile = ron::from_str(&text).expect("parses");
    let mut loaded = World::new();
    crate::format::from_scene_file(&file, &registry, &mut loaded).expect("loads");

    let (_, back) = loaded.query::<UiRect>().next().expect("the rect came back");
    assert_eq!(*back, rect);
}
