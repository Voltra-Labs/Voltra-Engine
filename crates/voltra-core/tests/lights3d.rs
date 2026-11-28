//! Lights and the environment through the engine's own frame, headless:
//! the shipped `scenes/lights3d.ron` — a room corner lit by red, green and
//! blue point lights, a spot light's cone on the floor, two suns, a sphere
//! among ten small lights, pillars fading into fog toward a gradient sky.
//! Each skips without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Vec3, Vec4};
use voltra_render::render3d::{Aabb3, SkyGradient, Tonemap};
use voltra_scene::{lighting3d, Background3D, Environment3D, Name};

const WIDTH: u32 = 480;
const HEIGHT: u32 = 270;

fn assets() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn open() -> Headless {
    let app = App::default().with_asset_root(assets());
    Headless::open(app, &AssetPath::new("scenes/lights3d.ron").expect("valid"))
        .expect("the lights sample loads")
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
        shot.save_png(&PathBuf::from(dir).join(format!("{name}.png")))
            .expect("the picture is written");
    }
    Some(shot)
}

/// Linear to the 8-bit sRGB the screenshot holds, through AgX.
fn agx(linear: Vec3) -> [u8; 3] {
    Tonemap::AgX.apply(linear.to_array()).map(|c| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

fn near(p: [u8; 4], rgb: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= by)
}

fn at(shot: &Screenshot, (x, y): (u32, u32)) -> [u8; 4] {
    shot.pixel(x, y).expect("on the picture")
}

fn sum(p: [u8; 4]) -> i32 {
    i32::from(p[0]) + i32::from(p[1]) + i32::from(p[2])
}

fn view(world: &World) -> voltra_render::render3d::View3D {
    voltra_scene::camera3d::active_view(world, WIDTH as f32 / HEIGHT as f32)
        .expect("the scene has a 3D camera")
}

/// The pixel `point` lands on through the scene's camera.
fn pixel_of(world: &World, point: Vec3) -> (u32, u32) {
    let clip = view(world).view_projection() * point.extend(1.0);
    let ndc = clip / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * WIDTH as f32;
    let y = (0.5 - ndc.y * 0.5) * HEIGHT as f32;
    (x as u32, y as u32)
}

/// The way the eye looks through pixel `(x, y)`'s centre.
fn ray(world: &World, (x, y): (u32, u32)) -> Vec3 {
    let view = view(world);
    let ndc_x = (x as f32 + 0.5) / WIDTH as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (y as f32 + 0.5) / HEIGHT as f32 * 2.0;
    let near = view.view_projection().inverse() * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    (near.truncate() / near.w - view.eye).normalize()
}

fn entity(world: &World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{name} is in the scene"))
}

fn position(world: &World, name: &str) -> Vec3 {
    voltra_scene::hierarchy::world_matrix_3d(world, entity(world, name))
        .translation
        .into()
}

fn sky(world: &World) -> SkyGradient {
    let environment = world
        .query::<Environment3D>()
        .next()
        .map(|(_, e)| *e)
        .expect("an environment");
    match environment.background {
        Background3D::Gradient { .. } => environment.background.sky().expect("a sky"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_room_is_lit_by_its_coloured_lamps_and_the_floor_by_the_spot() {
    let mut session = open();
    let Some(shot) = shoot(&mut session, "lights3d") else {
        return;
    };
    let world = session.world();
    // On the back wall, just behind each lamp: its colour leads.
    for (lamp, channel) in [("Red Lamp", 0), ("Green Lamp", 1), ("Blue Lamp", 2)] {
        let on_wall = position(world, lamp) * Vec3::new(1.0, 1.0, 0.0) + Vec3::new(0.0, 0.0, -2.0);
        let p = at(&shot, pixel_of(world, on_wall));
        let lead = p[channel];
        let rest = (0..3)
            .filter(|&i| i != channel)
            .map(|i| p[i])
            .max()
            .unwrap_or(0);
        assert!(lead > rest + 30, "{lamp}: {p:?} on the wall");
    }
    // The spot's cone on the floor: inside it brighter than outside.
    let spot = position(world, "Spot");
    let under = at(&shot, pixel_of(world, Vec3::new(spot.x, 0.0, spot.z)));
    let height = spot.y;
    let outside = Vec3::new(
        spot.x + height * 30f32.to_radians().tan() + 0.4,
        0.0,
        spot.z,
    );
    let beyond = at(&shot, pixel_of(world, outside));
    assert!(
        sum(under) > sum(beyond) + 100,
        "under {under:?}, outside {beyond:?}"
    );
}

#[test]
fn the_sky_is_its_gradient_and_the_far_pillar_fades_into_it() {
    let mut session = open();
    let Some(shot) = shoot(&mut session, "lights3d-sky") else {
        return;
    };
    let world = session.world();
    let gradient = sky(world);
    // Sky pixels along the top row and down the left edge above the
    // walls: the gradient along each ray through AgX, unfogged.
    let mut checked = 0;
    for (x, y) in [(20, 4), (240, 4), (460, 4), (240, 30), (400, 60), (470, 90)] {
        let p = at(&shot, (x, y));
        let want = agx(gradient.color(ray(world, (x, y))));
        assert!(near(p, want, 2), "sky at {x}, {y}: {p:?}, want {want:?}");
        checked += 1;
    }
    assert_eq!(checked, 6);
    // The furthest pillar is all but the sky behind it; the nearest is not.
    let far = pixel_of(world, position(world, "Pillar 3"));
    let p = at(&shot, far);
    let behind = agx(gradient.color(ray(world, far)));
    assert!(
        near(p, behind, 6),
        "the far pillar {p:?}, the sky {behind:?}"
    );
    let close = pixel_of(world, position(world, "Pillar 0"));
    let p = at(&shot, close);
    let behind = agx(gradient.color(ray(world, close)));
    assert!(
        !near(p, behind, 20),
        "the near pillar {p:?} is not the sky {behind:?}"
    );
}

#[test]
fn the_crowded_sphere_is_lit_by_its_eight_most_important_lights() {
    let mut session = open();
    let Some(before) = shoot(&mut session, "lights3d-crowded") else {
        return;
    };
    let world = session.world();
    let sphere = position(world, "Crowded");
    let bounds = Aabb3::new(sphere - Vec3::splat(0.6), sphere + Vec3::splat(0.6));
    let camera = voltra_scene::camera3d::active(world).expect("a camera");
    let gathered = lighting3d::gather(world, camera, &view(world));
    let picked = lighting3d::pick(&gathered.punctual, &gathered.directional_layers, &bounds, 1);
    assert!(picked.reached > 8, "{} lights reach it", picked.reached);
    let chosen: Vec<u32> = picked
        .list
        .punctual()
        .map(|i| gathered.punctual[usize::from(i)].entity)
        .collect();
    assert_eq!(chosen.len(), 8);
    let dimmest = entity(world, "Cluster 0");
    let brightest = entity(world, "Cluster 9");
    assert!(
        !chosen.contains(&dimmest.index()),
        "the dimmest is not chosen"
    );
    assert!(chosen.contains(&brightest.index()));
    // The sphere's pixels, as a box around its centre on screen.
    let (cx, cy) = pixel_of(world, sphere);
    let region: Vec<(u32, u32)> = (cy - 8..cy + 8)
        .flat_map(|y| (cx - 8..cx + 8).map(move |x| (x, y)))
        .collect();
    // Despawned between frames: the dimmest was not lighting the sphere,
    // so its pixels do not change at all; the brightest was.
    session.world_mut().despawn(dimmest);
    let Some(without_dimmest) = shoot(&mut session, "lights3d-without-dimmest") else {
        return;
    };
    for &xy in &region {
        assert_eq!(at(&before, xy), at(&without_dimmest, xy), "at {xy:?}");
    }
    session.world_mut().despawn(brightest);
    let Some(without_brightest) = shoot(&mut session, "lights3d-without-brightest") else {
        return;
    };
    let changed = region
        .iter()
        .filter(|&&xy| at(&before, xy) != at(&without_brightest, xy))
        .count();
    assert!(
        changed > region.len() / 2,
        "{changed} of {} changed",
        region.len()
    );
}

#[test]
fn each_sun_adds_to_the_floor() {
    let mut session = open();
    let Some(both) = shoot(&mut session, "lights3d-suns") else {
        return;
    };
    let world = session.world();
    let floor = pixel_of(world, Vec3::new(1.0, 0.0, -4.0));
    let cool = entity(world, "Sun Cool");
    session.world_mut().despawn(cool);
    let Some(warm_only) = shoot(&mut session, "lights3d-warm-sun") else {
        return;
    };
    let (a, b) = (at(&both, floor), at(&warm_only, floor));
    assert!(a[2] > b[2] + 5, "the cool sun's blue: {a:?} over {b:?}");
}
