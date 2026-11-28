//! Custom surface materials through the engine's own frame, headless: the
//! shipped `scenes/custom3d.ron` — a floor tinted by world position, a
//! sphere dissolving into noise with a glowing cut, stripes sliding across
//! another, a data texture's bytes shown unlit — and the dissolve's shader
//! broken and mended under a running session, as the editor's file watch
//! reloads it. Each skips without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::World;
use voltra_render::glam::Vec3;
use voltra_testkit::scratch_root;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 270;

/// What the sample draws from, copied so a test may edit it.
const FILES: [&str; 10] = [
    "scenes/custom3d.ron",
    "materials/world_tint.material.ron",
    "materials/world_tint.wgsl",
    "materials/dissolve.material.ron",
    "materials/dissolve.wgsl",
    "materials/scroll.material.ron",
    "materials/scroll.wgsl",
    "materials/bands.material.ron",
    "textures/noise.png",
    "textures/stripes.png",
];

fn sample_root() -> voltra_testkit::Scratch {
    let root = scratch_root();
    for file in FILES {
        voltra_testkit::copy_workspace_asset(&root, file);
    }
    root
}

fn open(root: &Path) -> Headless {
    let app = App::default().with_asset_root(root.to_path_buf());
    Headless::open(app, &AssetPath::new("scenes/custom3d.ron").expect("valid"))
        .expect("the custom materials sample loads")
}

fn shoot(session: &mut Headless, name: &str) -> Option<Screenshot> {
    session.step(1, 1.0 / 60.0);
    let shot = match session.screenshot(WIDTH, HEIGHT) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return None;
        }
        Err(e) => panic!("{e}"),
    };
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        let path = PathBuf::from(dir).join(format!("{name}.png"));
        shot.save_png(&path).expect("the picture is written");
    }
    Some(shot)
}

fn at(shot: &Screenshot, (x, y): (u32, u32)) -> [u8; 4] {
    shot.pixel(x, y).expect("on the picture")
}

/// The pixel `point` lands on through the scene's 3D camera.
fn pixel_of(world: &World, point: Vec3) -> (u32, u32) {
    let view = voltra_scene::camera3d::active_view(world, WIDTH as f32 / HEIGHT as f32)
        .expect("the scene has a 3D camera");
    let clip = view.view_projection() * point.extend(1.0);
    let ndc = clip / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * WIDTH as f32;
    let y = (0.5 - ndc.y * 0.5) * HEIGHT as f32;
    (x as u32, y as u32)
}

fn centre_of(world: &World, name: &str) -> Vec3 {
    let entity = world
        .query::<voltra_scene::Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{name} is in the scene"));
    voltra_scene::hierarchy::world_matrix_3d(world, entity)
        .translation
        .into()
}

/// Linear to the 8-bit sRGB the screenshot holds.
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

fn decode(byte: u8) -> f32 {
    let s = f32::from(byte) / 255.0;
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn near(p: [u8; 4], rgb: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= by)
}

/// `stripes.png`'s four bands, sRGB bytes.
const STRIPES: [[u8; 3]; 4] = [
    [230, 80, 60],
    [250, 210, 90],
    [70, 170, 120],
    [60, 110, 220],
];

/// The pixels within `r` of `centre` that look like the dissolve's glowing
/// rim: orange, brighter than anything the sun gives a grey.
fn glow(shot: &Screenshot, (cx, cy): (u32, u32), r: u32) -> usize {
    let mut count = 0;
    for y in cy.saturating_sub(r)..(cy + r).min(HEIGHT) {
        for x in cx.saturating_sub(r)..(cx + r).min(WIDTH) {
            let p = at(shot, (x, y));
            if p[0] > 200 && i32::from(p[0]) - i32::from(p[2]) > 70 {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn the_custom_sample_draws_each_material_as_its_shader_says() {
    let root = sample_root();
    let mut session = open(&root);
    let Some(shot) = shoot(&mut session, "custom3d") else {
        return;
    };
    let world = session.world();
    // The floor's tint is the world's: warm a quarter of the way through a
    // two-metre band, cool three quarters through.
    let warm = at(&shot, pixel_of(world, Vec3::new(0.5, 0.0, 2.0)));
    let cool = at(&shot, pixel_of(world, Vec3::new(1.5, 0.0, 2.0)));
    assert!(warm[0] > warm[2] + 20, "warm {warm:?}");
    assert!(cool[2] > cool[0] + 20, "cool {cool:?}");
    // The bands sphere shows its texture's bytes as linear numbers, unlit,
    // through AgX — not the sRGB colours they are as a picture.
    let bands = at(&shot, pixel_of(world, centre_of(world, "Bands")));
    let agx = voltra_render::render3d::Tonemap::AgX;
    let as_data = STRIPES.map(|b| encode(agx.apply(b.map(|c| f32::from(c) / 255.0))));
    let as_colour = STRIPES.map(|b| encode(agx.apply(b.map(decode))));
    assert!(
        as_data.iter().any(|want| near(bands, *want, 2)),
        "bands {bands:?}, the data through AgX {as_data:?}"
    );
    assert!(!as_colour.iter().any(|want| near(bands, *want, 6)));
    // The dissolve glows along its cut; the scroll is its stripes.
    let dissolve = pixel_of(world, centre_of(world, "Dissolve"));
    assert!(
        glow(&shot, dissolve, 40) > 100,
        "glow {}",
        glow(&shot, dissolve, 40)
    );
    let scroll = at(&shot, pixel_of(world, centre_of(world, "Scroll")));
    let spread = scroll[..3].iter().max().copied().unwrap_or(0)
        - scroll[..3].iter().min().copied().unwrap_or(0);
    assert!(spread > 40, "coloured, not the standard's grey: {scroll:?}");
}

#[test]
fn a_broken_shader_draws_the_standard_block_and_mends_on_reload() {
    voltra_core::logging::init(false);
    let root = sample_root();
    let mut session = open(&root);
    let Some(before) = shoot(&mut session, "custom3d-before") else {
        return;
    };
    let dissolve = pixel_of(session.world(), centre_of(session.world(), "Dissolve"));
    let wgsl = AssetPath::new("materials/dissolve.wgsl").expect("valid");
    assert!(glow(&before, dissolve, 40) > 100);
    session.take_logs();
    // Broken under the running session, and reloaded as the watch would.
    std::fs::write(
        root.join(wgsl.as_str()),
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.alpha = material.nope;\n    return s;\n}\n",
    )
    .expect("written");
    session.reload(&wgsl);
    let Some(broken) = shoot(&mut session, "custom3d-broken") else {
        return;
    };
    let _ = shoot(&mut session, "custom3d-broken-again");
    // Its standard block: whole — no cut, no glow — and grey.
    assert_eq!(glow(&broken, dissolve, 40), 0);
    let centre = at(&broken, dissolve);
    assert!(
        (i32::from(centre[0]) - i32::from(centre[2])).abs() < 16 && centre[1] > 80,
        "the standard block's grey: {centre:?}"
    );
    // Said once, with what is wrong, however many frames draw it.
    let said: Vec<_> = session
        .take_logs()
        .into_iter()
        .filter(|l| {
            l.level == log::Level::Warn && l.message.contains("materials/dissolve.material.ron")
        })
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    let message = &said[0].message;
    assert!(
        message.contains("nope") && message.contains("standard block"),
        "{message}"
    );
    // At the line of the material's own file: its source comes first.
    assert!(message.contains("wgsl:3:"), "{message}");
    // Mended: drawn by the new shader the next frame.
    std::fs::write(
        root.join(wgsl.as_str()),
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = vec3<f32>(1.0, 0.0, 0.0);\n    s.alpha = 1.0;\n    return s;\n}\n",
    )
    .expect("written");
    session.reload(&wgsl);
    let Some(mended) = shoot(&mut session, "custom3d-mended") else {
        return;
    };
    let centre = at(&mended, dissolve);
    assert!(
        centre[0] > 120 && centre[1] < 60 && centre[2] < 60,
        "red: {centre:?}"
    );
}
