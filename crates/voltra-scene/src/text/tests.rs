use voltra_assets::{AssetPath, Atlases, Fonts, Handle, DEFAULT_FONT};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_testkit::{copy_bundled_font, scratch_root};

use super::shape::{bounds, glyphs};
use super::*;
use crate::batch::SpriteBatch;
use crate::pick::entity_at;
use crate::sprite::sheets::Sheets;
use crate::{Sprite, Transform};

fn fonts() -> Fonts {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    Fonts::new(root)
}

/// A text resolved against `fonts` and prepared, as a frame would leave it.
fn text(fonts: &mut Fonts, content: &str) -> Text {
    let mut text = Text::new(content);
    text.set_font(None, fonts);
    if let Some(handle) = text.font_handle {
        fonts.prepare(handle, content, text.px());
    }
    text
}

#[test]
fn no_font_resolves_to_the_default() {
    let mut fonts = fonts();
    let text = text(&mut fonts, "hi");
    assert_eq!(text.font_handle, Some(fonts.default_font()));
    assert_eq!(text.font, None);
}

#[test]
fn naming_a_font_by_path_keeps_the_path() {
    let mut fonts = fonts();
    let mut text = Text::new("hi");
    let path = AssetPath::new(DEFAULT_FONT).expect("valid");
    text.set_font(Some(path.clone()), &mut fonts);
    assert_eq!(text.font, Some(path));
    assert_eq!(text.font_handle, Some(fonts.default_font()));
}

#[test]
fn a_zero_raster_size_is_clamped_rather_than_divided_by() {
    let text = Text {
        pixels_per_em: 0,
        ..Text::default()
    };
    assert_eq!(text.px(), 1);
}

#[test]
fn one_glyph_quad_per_inked_character() {
    let mut fonts = fonts();
    let text = text(&mut fonts, "a b");
    let (quads, _) = glyphs(&text, &fonts);
    assert_eq!(quads.len(), 2, "the space has no ink");
}

#[test]
fn glyphs_sit_on_the_baseline_and_start_near_the_origin() {
    let mut fonts = fonts();
    let text = text(&mut fonts, "H");
    let (quads, _) = glyphs(&text, &fonts);
    let [top_left, bottom_left, ..] = quads[0].corners;

    assert!(bottom_left.y.abs() < 0.05, "an H stands on the baseline");
    assert!(
        top_left.y > 0.5 && top_left.y < 1.0,
        "and is under an em tall"
    );
    assert!(top_left.x >= 0.0 && top_left.x < 0.2);
}

#[test]
fn size_scales_the_glyphs_and_raster_resolution_does_not() {
    let mut fonts = fonts();
    let small = text(&mut fonts, "H");
    let mut large = small.clone();
    large.size = 2.0;
    let mut crisp = small.clone();
    crisp.pixels_per_em = 96;
    fonts.prepare(fonts.default_font(), "H", 96);

    let height = |t: &Text| {
        let (quads, _) = glyphs(t, &fonts);
        quads[0].corners[0].y - quads[0].corners[1].y
    };
    assert!((height(&large) - 2.0 * height(&small)).abs() < 0.05);
    assert!((height(&crisp) - height(&small)).abs() < 0.05);
}

#[test]
fn a_second_line_is_below_the_first() {
    let mut fonts = fonts();
    let text = text(&mut fonts, "H\nH");
    let (quads, _) = glyphs(&text, &fonts);
    assert!(quads[1].corners[1].y < quads[0].corners[1].y - 0.5);
    assert_eq!(quads[1].corners[0].x, quads[0].corners[0].x);
}

#[test]
fn alignment_moves_lines_about_the_origin() {
    let mut fonts = fonts();
    let left = text(&mut fonts, "HH");
    let box_of = |align| {
        let mut t = left.clone();
        t.align = align;
        bounds(&t, &fonts).expect("a face")
    };

    let l = box_of(Align::Left);
    let c = box_of(Align::Center);
    let r = box_of(Align::Right);
    assert_eq!(l.min.x, 0.0);
    assert!((c.min.x + c.max.x).abs() < 1e-4, "centred on the origin");
    assert_eq!(r.max.x, 0.0);
}

#[test]
fn centred_lines_of_different_widths_share_a_middle() {
    let mut fonts = fonts();
    let mut t = text(&mut fonts, "H\nHHH");
    t.align = Align::Center;
    let (quads, _) = glyphs(&t, &fonts);
    let first = quads[0].corners[0].x;
    let second_line_start = quads[1].corners[0].x;
    assert!(
        second_line_start < first,
        "the longer line starts further left"
    );
}

#[test]
fn bounds_cover_ascent_to_the_last_descent() {
    let mut fonts = fonts();
    let one = bounds(&text(&mut fonts, "H"), &fonts).expect("a face");
    let two = bounds(&text(&mut fonts, "H\nH"), &fonts).expect("a face");
    assert!(one.max.y > 0.5);
    assert!(one.min.y < 0.0);
    assert_eq!(two.max.y, one.max.y);
    assert!(two.min.y < one.min.y - 0.5);
}

#[test]
fn empty_content_has_a_box_that_contains_nothing() {
    let mut fonts = fonts();
    let b = bounds(&text(&mut fonts, ""), &fonts).expect("a face");
    assert!(!b.contains(Vec2::ZERO));
}

#[test]
fn unprepared_glyphs_are_skipped_not_drawn_wrong() {
    let mut fonts = fonts();
    let mut t = Text::new("xyz");
    t.set_font(None, &mut fonts);
    let (quads, _) = glyphs(&t, &fonts);
    assert!(quads.is_empty());
}

#[test]
fn prepare_rasterizes_what_the_world_holds() {
    let mut fonts = fonts();
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(e, Text::new("ab"));
    resolve_fonts(&mut world, &mut fonts);

    prepare(&world, &mut fonts, 1.0);

    let atlas = fonts
        .try_get(fonts.default_font())
        .expect("resolves")
        .atlas();
    assert_eq!(atlas.len(), 2);
}

mod in_the_world {
    use super::*;

    fn spawn<T: 'static>(world: &mut World, component: T, at: Vec2) -> Entity {
        let e = world.spawn();
        world.insert(e, Transform::from_translation(at));
        world.insert(e, component);
        e
    }

    fn ready(world: &mut World, fonts: &mut Fonts) {
        resolve_fonts(world, fonts);
        prepare(world, fonts, 1.0);
        // A texture handle from nowhere: the batch only compares it.
        fonts.forge_texture(fonts.default_font(), Handle::forge(7, 0));
    }

    #[test]
    fn text_is_drawn_between_sprites_by_sort_order() {
        let mut fonts = fonts();
        let atlases = Atlases::new(scratch_root());
        let mut world = World::new();
        spawn(&mut world, Sprite::default().with_sort_order(2), Vec2::ZERO);
        let mut label = Text::new("H");
        label.sort_order = 1;
        spawn(&mut world, label, Vec2::ZERO);
        spawn(&mut world, Sprite::default().with_sort_order(0), Vec2::ZERO);
        ready(&mut world, &mut fonts);

        let sheets = Sheets {
            atlases: Some(&atlases),
            textures: None,
            fonts: Some(&fonts),
            camera: voltra_render::glam::Vec2::ZERO,
            visible: None,
            time: 0.0,
            layers: None,
        };
        let batch = SpriteBatch::from_world(&world, sheets);

        let textures: Vec<_> = batch.ranges.iter().map(|r| r.texture).collect();
        assert_eq!(
            textures,
            vec![None, Some(Handle::forge(7, 0)), None],
            "sprite at 0, the text at 1, the sprite at 2"
        );
    }

    #[test]
    fn without_fonts_nothing_is_drawn() {
        let mut fonts = fonts();
        let mut world = World::new();
        spawn(&mut world, Text::new("H"), Vec2::ZERO);
        ready(&mut world, &mut fonts);

        let batch = SpriteBatch::from_world(&world, Sheets::default());
        assert!(batch.is_empty());
    }

    #[test]
    fn a_text_is_picked_inside_its_box_through_its_transform() {
        let mut fonts = fonts();
        let mut world = World::new();
        let label = spawn(&mut world, Text::new("HHHH"), Vec2::new(10.0, 0.0));
        ready(&mut world, &mut fonts);
        let sheets = Sheets {
            fonts: Some(&fonts),
            ..Sheets::default()
        };

        assert_eq!(entity_at(&world, Vec2::new(11.0, 0.3), sheets), Some(label));
        assert_eq!(entity_at(&world, Vec2::new(9.0, 0.3), sheets), None);
    }

    #[test]
    fn a_text_in_front_of_a_sprite_is_picked_over_it() {
        let mut fonts = fonts();
        let mut world = World::new();
        spawn(&mut world, Sprite::default(), Vec2::new(0.5, 0.0));
        let mut front = Text::new("HHHH");
        front.sort_order = 1;
        let label = spawn(&mut world, front, Vec2::ZERO);
        ready(&mut world, &mut fonts);
        let sheets = Sheets {
            fonts: Some(&fonts),
            ..Sheets::default()
        };

        assert_eq!(entity_at(&world, Vec2::new(0.5, 0.2), sheets), Some(label));
    }
}
