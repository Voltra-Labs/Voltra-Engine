//! Rich text in pixels: a line in two colours and two sizes with an inline
//! icon, matched against a reference image, and a coloured run that keeps
//! its colour across a wrap.
//!
//! Skips itself when no GPU adapter is available.

#[path = "support/text_shot.rs"]
mod text_shot;

use text_shot::{matches_reference, Shot};
use voltra_assets::DEFAULT_FONT;
use voltra_render::glam::Vec2;
use voltra_scene::Text;
use voltra_testkit::{copy_workspace_asset, scratch_root, Rgba};

fn shot(width: u32, height: u32) -> Shot {
    let root = scratch_root();
    for asset in [DEFAULT_FONT, "sprites/coin.png", "sprites/coin.atlas.ron"] {
        copy_workspace_asset(&root, asset);
    }
    Shot::new(root.keep(), &[], width, height)
}

fn red(p: &Rgba) -> bool {
    p.r > 150 && p.g < 110 && p.b < 110
}

fn green(p: &Rgba) -> bool {
    p.g > 150 && p.r < 110 && p.b < 110
}

/// The rows (top, bottom) where `pixels` has a pixel `is`.
fn rows(pixels: &[Rgba], width: u32, is: fn(&Rgba) -> bool) -> Option<(u32, u32)> {
    let hit: Vec<u32> = pixels
        .chunks(width as usize)
        .enumerate()
        .filter(|(_, row)| row.iter().any(is))
        .map(|(y, _)| y as u32)
        .collect();
    Some((*hit.first()?, *hit.last()?))
}

#[test]
fn a_line_in_two_colours_and_sizes_with_an_icon() {
    const W: u32 = 288;
    const H: u32 = 56;
    let mut shot = shot(W, H);
    shot.text(
        Vec2::new(8.0, 8.0),
        Vec2::new(272.0, 44.0),
        Text {
            size: 18.0,
            ..Text::new(
                "[color=#f00]Red[/color] and [size=32][color=#0f0]Big[/color][/size] \
                 [icon=sprites/coin.atlas.ron/0] coins",
            )
        },
    );
    let Some(pixels) = shot.draw() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    matches_reference("rich_text_line", &pixels, W, H);
    let (red_top, red_bottom) = rows(&pixels, W, red).expect("red letters");
    let (green_top, green_bottom) = rows(&pixels, W, green).expect("green letters");
    assert!(
        green_bottom - green_top > red_bottom - red_top + 4,
        "the [size=32] run is taller: red {red_top}..{red_bottom}, green {green_top}..{green_bottom}"
    );
    // The coin: yellow ink, neither the text's white nor a run's colour.
    let coin = pixels
        .iter()
        .filter(|p| p.r > 150 && p.g > 110 && p.b < 90)
        .count();
    assert!(coin > 20, "the icon draws: {coin} coin pixels");
}

#[test]
fn a_coloured_run_keeps_its_colour_across_a_wrap() {
    const W: u32 = 160;
    const H: u32 = 80;
    let mut shot = shot(W, H);
    shot.text(
        Vec2::new(4.0, 4.0),
        Vec2::new(120.0, 72.0),
        Text {
            size: 18.0,
            max_width: Some(10_000.0),
            ..Text::new("plain [color=#0f0]green words that wrap[/color] end")
        },
    );
    let Some(pixels) = shot.draw() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let (top, bottom) = rows(&pixels, W, green).expect("green letters");
    assert!(
        bottom - top > 30,
        "green on more than one line: rows {top}..{bottom}"
    );
}
