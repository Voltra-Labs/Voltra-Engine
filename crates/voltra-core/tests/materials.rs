//! The shipped materials drawn through the engine's own frame: three coins
//! — plain, hit-flashed white, outlined green — and a bar whose shader
//! only WebGPU refuses, in `scenes/materials.ron`, screenshotted headless.
//! Skips without a GPU adapter.

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, ScreenshotError};

const W: u32 = 480;
const H: u32 = 160;

/// Where each coin's centre lands, in pixels across: the camera shows 9
/// units across 480 pixels, and the coins are at -2, 0 and 2.
const CENTRES: [u32; 3] = [133, 240, 347];

/// How many pixels within 52 of coin `coin`'s centre are close to `rgb`.
fn count(shot: &voltra_core::Screenshot, coin: usize, rgb: [u8; 3]) -> usize {
    let mut n = 0;
    for y in 0..H {
        for x in CENTRES[coin] - 52..CENTRES[coin] + 52 {
            let p = shot.pixel(x, y).expect("inside");
            let near = (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() < 60);
            n += usize::from(near);
        }
    }
    n
}

#[test]
fn a_hit_flash_and_an_outline_draw_and_a_plain_coin_does_not() {
    let path = AssetPath::new("scenes/materials.ron").expect("a path");
    let mut session = Headless::open(App::default(), &path).expect("the scene loads");
    session.step(1, 1.0 / 60.0);
    let shot = match session.screenshot(W, H) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let white = [255, 255, 255];
    // Linear (0.1, 1.0, 0.2) written to an sRGB target.
    let green = [89, 255, 124];
    let (plain_white, flash_white, outline_white) = (
        count(&shot, 0, white),
        count(&shot, 1, white),
        count(&shot, 2, white),
    );
    let (plain_green, flash_green, outline_green) = (
        count(&shot, 0, green),
        count(&shot, 1, green),
        count(&shot, 2, green),
    );
    println!("white {plain_white} {flash_white} {outline_white}; green {plain_green} {flash_green} {outline_green}");
    assert!(
        flash_white > 1000,
        "the flashed coin is white: {flash_white}"
    );
    assert!(plain_white < flash_white / 10, "the plain coin is not");
    assert!(outline_green > 100, "the outline is drawn: {outline_green}");
    assert!(plain_green < 10 && flash_green < 10, "and only there");
    // The branchy bar under the middle coin: its shader samples inside a
    // branch, which naga allows, so natively it is its own blue — WebGPU
    // draws it plainly, in its orange tint (materials.mjs).
    let blue = [89, 149, 255];
    let orange = [255, 188, 89];
    let (bar_blue, bar_orange) = (count(&shot, 1, blue), count(&shot, 1, orange));
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        shot.save_png(&std::path::PathBuf::from(dir).join("materials2d.png"))
            .expect("the picture is written");
    }
    assert!(
        bar_blue > 200,
        "the branchy bar drawn by its own shader: {bar_blue}"
    );
    assert!(bar_orange < 10, "not plainly: {bar_orange}");
}
