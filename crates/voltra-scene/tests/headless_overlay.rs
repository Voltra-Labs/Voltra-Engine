//! A dropdown's open list and a tooltip in pixels: drawn over every
//! element, even one sorted above everything else.
//!
//! Skips itself when no GPU adapter is available.

#[path = "support/text_shot.rs"]
mod text_shot;

use text_shot::Shot;
use voltra_assets::DEFAULT_FONT;
use voltra_render::glam::Vec2;
use voltra_scene::{SceneId, Sprite2D, Text, UiButton, UiDropdown, UiRect, UiTooltip};
use voltra_testkit::{copy_workspace_asset, scratch_root};

const W: u32 = 200;
const H: u32 = 200;

#[test]
fn an_open_list_and_a_tooltip_draw_over_everything() {
    let root = scratch_root();
    copy_workspace_asset(&root, DEFAULT_FONT);
    let mut shot = Shot::new(root.to_path_buf(), &[], W, H);

    let dropdown = shot.text(
        Vec2::new(10.0, 10.0),
        Vec2::new(120.0, 24.0),
        Text {
            size: 14.0,
            ..Text::new("One")
        },
    );
    shot.world.insert(dropdown, Sprite2D::default());
    shot.world.insert(dropdown, UiButton::default());
    shot.world.insert(
        dropdown,
        UiDropdown {
            open: true,
            highlighted: 1,
            highlight_color: [0.0, 0.0, 1.0, 1.0],
            list_color: [0.0, 1.0, 0.0, 1.0],
            ..UiDropdown::default()
        },
    );
    // A red panel where the list opens, sorted above everything.
    let panel = shot.world.spawn();
    shot.world.insert(panel, SceneId::new());
    shot.world.insert(
        panel,
        UiRect::pinned(Vec2::ZERO, Vec2::new(0.0, 30.0), Vec2::new(200.0, 80.0)),
    );
    shot.world.insert(
        panel,
        Sprite2D {
            sort_order: 10_000,
            ..Sprite2D::new([1.0, 0.0, 0.0, 1.0])
        },
    );
    // And a tooltip shown on a button lower down.
    let helped = shot.world.spawn();
    shot.world.insert(helped, SceneId::new());
    shot.world.insert(
        helped,
        UiRect::pinned(Vec2::ZERO, Vec2::new(10.0, 130.0), Vec2::new(60.0, 20.0)),
    );
    shot.world.insert(helped, Sprite2D::default());
    shot.world.insert(
        helped,
        UiTooltip {
            shown: true,
            background: [1.0, 1.0, 0.0, 1.0],
            ..UiTooltip::new("Help")
        },
    );

    let Some(pixels) = shot.draw() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let at = |x: u32, y: u32| pixels[(y * W + x) as usize];
    // Items are the box's height, 24 points, from y = 34.
    let first = at(110, 40);
    assert!(
        first.g > 200 && first.r < 60,
        "the list, over the red panel: {first:?}"
    );
    let highlight = at(110, 34 + 24 + 12);
    assert!(
        highlight.b > 200 && highlight.r < 60,
        "the highlighted item: {highlight:?}"
    );
    let beside = at(160, 40);
    assert!(
        beside.r > 200 && beside.g < 60,
        "the panel beside the list: {beside:?}"
    );
    let tip = at(14, 156);
    assert!(
        tip.r > 200 && tip.g > 200 && tip.b < 60,
        "the tooltip under its element: {tip:?}"
    );
}
