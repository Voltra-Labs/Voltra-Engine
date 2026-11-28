//! The web tests' refusal fixture drawn natively, headless:
//! `crates/voltra-player/web/test/fixtures/assets/scenes/refusals.ron` — a
//! sphere and a sprite bar whose shaders sample inside a branch, beside
//! plain ones. naga lifts WGSL's fragment uniformity rule, so natively
//! (as on WebGL2) both run as written; `refusals.mjs` pins WebGPU, where
//! Tint refuses them and their fallbacks draw. Skips without a GPU
//! adapter. Set `VOLTRA_SHOT_DIR` to keep the picture.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_render::glam::Vec3;
use voltra_render::render3d::Tonemap;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 270;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../voltra-player/web/test/fixtures/assets")
}

fn encode(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

/// How many pixels within `r` of `(cx, cy)` are within 3 of `rgb`.
fn count(shot: &Screenshot, (cx, cy): (u32, u32), r: u32, rgb: [u8; 3]) -> usize {
    let mut n = 0;
    for y in cy.saturating_sub(r)..(cy + r).min(HEIGHT) {
        for x in cx.saturating_sub(r)..(cx + r).min(WIDTH) {
            let p = shot.pixel(x, y).expect("on the picture");
            n += usize::from((0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= 3));
        }
    }
    n
}

/// The pixel `point` lands on through the fixture's 3D camera.
fn pixel_of(session: &Headless, point: Vec3) -> (u32, u32) {
    let view = voltra_scene::camera3d::active_view(session.world(), WIDTH as f32 / HEIGHT as f32)
        .expect("a 3D camera");
    let clip = view.view_projection() * point.extend(1.0);
    let ndc = clip / clip.w;
    (
        ((ndc.x * 0.5 + 0.5) * WIDTH as f32) as u32,
        ((0.5 - ndc.y * 0.5) * HEIGHT as f32) as u32,
    )
}

#[test]
fn natively_both_branchy_shaders_run_as_written() {
    let app = App::default().with_asset_root(fixture());
    let mut session = Headless::open(app, &AssetPath::new("scenes/refusals.ron").expect("valid"))
        .expect("the fixture loads");
    session.step(1, 1.0 / 60.0);
    let shot = match session.screenshot(WIDTH, HEIGHT) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        shot.save_png(&PathBuf::from(dir).join("refusals.png"))
            .expect("the picture is written");
    }
    let agx = |rgb: [f32; 3]| encode(Tonemap::AgX.apply(rgb));
    let branchy = pixel_of(&session, Vec3::new(-1.2, 0.6, 0.0));
    let plain = pixel_of(&session, Vec3::new(1.2, 0.6, 0.0));
    // The 3D ones: the branchy sphere its own yellow (WebGPU: its
    // standard block's red), the plain one its green, unlit through AgX.
    let yellow = agx([0.9, 0.75, 0.1]);
    let red = agx([0.8, 0.08, 0.06]);
    let green = agx([0.1, 0.8, 0.2]);
    assert!(
        count(&shot, branchy, 12, yellow) > 300,
        "the branchy sphere runs"
    );
    assert_eq!(count(&shot, branchy, 12, red), 0);
    assert!(count(&shot, plain, 12, green) > 300, "the plain sphere");
    // The 2D ones, a 2D camera 1.5 units half-height over the 270 pixels:
    // the branchy bar its own blue (WebGPU: plainly, its orange tint), the
    // plain bar its cyan.
    let unit = HEIGHT as f32 / 3.0;
    let bar = |x: f32| {
        (
            (WIDTH as f32 / 2.0 + x * unit) as u32,
            (HEIGHT as f32 / 2.0 + 0.9 * unit) as u32,
        )
    };
    let (blue, orange, cyan) = (
        encode([0.1, 0.3, 1.0]),
        encode([1.0, 0.5, 0.1]),
        encode([0.1, 0.9, 1.0]),
    );
    assert!(
        count(&shot, bar(-1.0), 10, blue) > 200,
        "the branchy bar runs"
    );
    assert_eq!(count(&shot, bar(-1.0), 10, orange), 0);
    assert!(count(&shot, bar(1.0), 10, cyan) > 200, "the plain bar");
}
