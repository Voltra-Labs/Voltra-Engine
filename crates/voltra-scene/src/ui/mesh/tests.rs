//! A text field as drawn: its caret, its selection, its text scrolled and
//! cut to its box, its placeholder.

use voltra_assets::{Fonts, Handle, DEFAULT_FONT};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_testkit::{copy_bundled_font, scratch_root};

use crate::batch::SpriteBatch;
use crate::sprite::sheets::Sheets;
use crate::text::{prepare, resolve_fonts};
use crate::ui::{mesh, UiControls, UiInput, UiLayout, UiRect, UiTextField};
use crate::{SceneId, Text};

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

fn fonts() -> Fonts {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    Fonts::new(root)
}

/// A field `width` points wide at (100, 100), holding `text`.
fn field(world: &mut World, width: f32, text: &str) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::new(width, 30.0)),
    );
    world.insert(
        e,
        Text {
            size: 20.0,
            ..Text::new("")
        },
    );
    world.insert(e, UiTextField::with_text(text));
    e
}

/// One frame of the controls with `focus`, then the batch as drawn.
fn drawn(world: &mut World, fonts: &mut Fonts, focus: Option<Entity>) -> SpriteBatch {
    resolve_fonts(world, fonts);
    let mut controls = UiControls::default();
    controls.focus(focus);
    let layout = UiLayout::from_world(world, SCREEN);
    controls.update(world, &layout, Some(fonts), &UiInput::default());
    prepare(world, fonts, 1.0);
    fonts.forge_texture(Handle::forge(7, 0));
    let sheets = Sheets {
        fonts: Some(fonts),
        ..Sheets::default()
    };
    let layout = UiLayout::from_world(world, SCREEN);
    mesh::batch(world, sheets, &layout, 1.0)
}

/// The quads drawn untextured — the caret and the selection — as
/// (min, max) boxes, in the order drawn.
fn flat(batch: &SpriteBatch) -> Vec<(Vec2, Vec2)> {
    batch
        .ranges
        .iter()
        .filter(|range| range.texture.is_none())
        .flat_map(|range| {
            let first = range.indices.start as usize / 6;
            let last = range.indices.end as usize / 6;
            (first..last).map(|quad| {
                let corners = &batch.vertices[quad * 4..quad * 4 + 4];
                let min = corners.iter().fold(Vec2::splat(f32::INFINITY), |m, v| {
                    m.min(Vec2::from(v.position))
                });
                let max = corners.iter().fold(Vec2::splat(f32::NEG_INFINITY), |m, v| {
                    m.max(Vec2::from(v.position))
                });
                (min, max)
            })
        })
        .collect()
}

#[test]
fn a_focused_field_draws_a_caret_after_its_text_and_an_unfocused_one_none() {
    let mut world = World::new();
    let mut fonts = fonts();
    let name = field(&mut world, 200.0, "Hello");
    let unfocused = drawn(&mut world, &mut fonts, None);
    assert!(flat(&unfocused).is_empty());
    assert_eq!(unfocused.sprite_count(), 5, "five letters");

    let focused = drawn(&mut world, &mut fonts, Some(name));
    let carets = flat(&focused);
    assert_eq!(carets.len(), 1);
    let (min, max) = carets[0];
    assert!(max.x - min.x >= 1.0 && max.x - min.x <= 2.0, "a thin bar");
    let glyphs_end = focused
        .vertices
        .iter()
        .map(|v| v.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        min.x > 140.0 && min.x <= glyphs_end + 1.0,
        "after the o: {min:?}"
    );
    assert!(max.y - min.y > 15.0, "a line tall");
}

#[test]
fn a_selection_is_drawn_under_its_letters() {
    let mut world = World::new();
    let mut fonts = fonts();
    let name = field(&mut world, 200.0, "Hello");
    let f = world.get_mut::<UiTextField>(name).expect("a field");
    f.anchor = 1;
    f.caret = 4;
    let batch = drawn(&mut world, &mut fonts, Some(name));
    let boxes = flat(&batch);
    assert_eq!(boxes.len(), 2, "the selection and the caret");
    let (selection, caret) = (boxes[0], boxes[1]);
    assert!(selection.0.x > 100.0 && selection.1.x > selection.0.x + 20.0);
    assert!(
        (caret.0.x - selection.1.x).abs() <= 1.0,
        "the caret at its end"
    );
    // The selection is the batch's first quad: under the glyphs.
    assert!(batch.ranges[0].texture.is_none());
}

#[test]
fn a_long_text_scrolls_to_its_caret_and_is_cut_to_the_box() {
    let mut world = World::new();
    let mut fonts = fonts();
    let name = field(&mut world, 60.0, "A rather long name to type");
    let batch = drawn(&mut world, &mut fonts, Some(name));
    assert!(world.get::<UiTextField>(name).expect("a field").scroll > 0.0);
    for v in &batch.vertices {
        assert!(
            (100.0..=160.0).contains(&v.position[0]),
            "inside the box: {v:?}"
        );
    }
    let caret = flat(&batch)[0];
    assert!(
        caret.1.x <= 160.0 && caret.0.x > 140.0,
        "at the right edge: {caret:?}"
    );
}

#[test]
fn the_placeholder_is_fainter() {
    let mut world = World::new();
    let mut fonts = fonts();
    let name = field(&mut world, 200.0, "");
    world
        .get_mut::<UiTextField>(name)
        .expect("a field")
        .placeholder = "Name".to_owned();
    let batch = drawn(&mut world, &mut fonts, None);
    assert_eq!(batch.sprite_count(), 4);
    assert!(batch.vertices.iter().all(|v| v.color[3] == 0.5));
}
