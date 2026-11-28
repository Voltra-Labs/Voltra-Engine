//! Standard surface materials through the engine's own frame, headless:
//! the shipped `scenes/materials3d.ron` — four spheres from a rough
//! dielectric to a smooth metal under one directional light, an unlit one
//! beside them, a normal-mapped floor, a grate cut out by its alpha, a
//! glass pane — and two blended quads spawned in either order. Each skips
//! without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

use std::path::PathBuf;

use voltra_assets::{AssetPath, BuiltinMesh};
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::World;
use voltra_render::glam::{Quat, Vec3};
use voltra_render::render3d::Tonemap;
use voltra_scene::{Camera3D, CameraClear, Environment3D, MeshInstance3D, SceneId, Transform3D};
use voltra_testkit::scratch_root;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 270;

fn assets() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
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

fn near(p: [u8; 4], rgb: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= by)
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

/// The brightest pixel, summed over its channels, within `r` of `centre`.
fn peak(shot: &Screenshot, (cx, cy): (u32, u32), r: u32) -> u32 {
    let mut best = 0;
    for y in cy.saturating_sub(r)..(cy + r).min(HEIGHT) {
        for x in cx.saturating_sub(r)..(cx + r).min(WIDTH) {
            let p = at(shot, (x, y));
            best = best.max(u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]));
        }
    }
    best
}

fn sphere_at(world: &World, name: &str) -> Vec3 {
    let entity = world
        .query::<voltra_scene::Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{name} is in the scene"));
    voltra_scene::hierarchy::world_matrix_3d(world, entity)
        .translation
        .into()
}

#[test]
fn the_material_sample_lights_each_surface_as_its_material_says() {
    let app = App::default().with_asset_root(assets());
    let mut session = Headless::open(
        app,
        &AssetPath::new("scenes/materials3d.ron").expect("valid"),
    )
    .expect("the materials sample loads");
    let Some(shot) = shoot(&mut session, "materials3d") else {
        return;
    };
    let world = session.world();
    // The unlit sphere is its colour through AgX, light or none.
    let unlit = pixel_of(world, sphere_at(world, "Unlit"));
    let agx = Tonemap::AgX.apply([0.2, 0.9, 0.4]);
    let want = encode(agx);
    assert!(
        near(at(&shot, unlit), want, 2),
        "unlit {:?}, AgX of its colour {want:?}",
        at(&shot, unlit)
    );
    // Every lit sphere is shaded: brighter toward the light (upper right)
    // than away from it (lower left).
    for name in ["Clay", "Plastic", "Iron", "Gold"] {
        let centre = sphere_at(world, name);
        let lit = at(&shot, pixel_of(world, centre + Vec3::new(0.25, 0.3, 0.4)));
        let dark = at(&shot, pixel_of(world, centre + Vec3::new(-0.3, -0.3, 0.3)));
        let sum = |p: [u8; 4]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
        assert!(
            sum(lit) > sum(dark) + 30,
            "{name}: lit {lit:?}, away {dark:?}"
        );
    }
    // The smooth metal's highlight is a small bright spot; the rough
    // metal's is dim and wide.
    let gold = peak(&shot, pixel_of(world, sphere_at(world, "Gold")), 30);
    let iron = peak(&shot, pixel_of(world, sphere_at(world, "Iron")), 30);
    assert!(gold > iron + 100, "gold {gold}, iron {iron}");
    // The glass pane over the red box: the box shows through, paled.
    let through = at(&shot, pixel_of(world, Vec3::new(2.2, 0.8, -2.2)));
    let red = Tonemap::AgX.apply([0.9, 0.15, 0.1]);
    let box_red = encode(red);
    assert!(
        through[0] > through[1] && !near(through, box_red, 6),
        "the box through the glass {through:?}, bare {box_red:?}"
    );
    // The grate: its bars drawn, its holes show the lamp's glow behind.
    let mut bars = 0;
    let mut holes = 0;
    let (gx, gy) = pixel_of(world, Vec3::new(-2.0, 1.1, -2.5));
    for y in gy - 12..gy + 12 {
        for x in gx - 12..gx + 12 {
            let p = at(&shot, (x, y));
            let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
            if (r - g).abs() < 12 && (g - b).abs() < 16 && r > 60 {
                bars += 1;
            } else {
                holes += 1;
            }
        }
    }
    assert!(bars > 60 && holes > 60, "bars {bars}, holes {holes}");
}

/// Two overlapping blended quads and the opaque wall behind them, flat
/// colours with no curve, spawned in `order`.
fn quads(root: &std::path::Path, reverse: bool) -> App {
    std::fs::create_dir_all(root.join("m")).expect("a folder");
    std::fs::write(
        root.join("m/glass.material.ron"),
        "(version: 1, domain: Surface, standard: (unlit: true, alpha: Blend))",
    )
    .expect("the glass");
    voltra_testkit::copy_workspace_asset(root, "materials/unlit.material.ron");
    let glass = AssetPath::new("m/glass.material.ron").expect("a path");
    let unlit = AssetPath::new("materials/unlit.material.ron").expect("a path");
    let mut app = App::default().with_asset_root(root);
    let world = &mut app.world;
    let camera = world.spawn();
    world.insert(camera, SceneId::new());
    world.insert(
        camera,
        Transform3D::from_translation(Vec3::new(0.0, 0.0, 5.0)),
    );
    world.insert(
        camera,
        Camera3D {
            tonemap: Tonemap::None,
            clear: CameraClear::Color([0.0, 0.0, 0.0, 1.0]),
            ..Camera3D::default()
        },
    );
    world.insert(camera, Environment3D { ambient: [0.0; 3] });
    let facing = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let mut spawned = vec![
        // The near quad, blue, a quarter opaque; the far, green, half.
        (
            Vec3::new(-0.4, 0.0, 1.0),
            [0.0, 0.0, 1.0, 0.25],
            glass.clone(),
        ),
        (Vec3::new(0.4, 0.0, -1.0), [0.0, 1.0, 0.0, 0.5], glass),
        (Vec3::new(0.0, 0.0, -3.0), [1.0, 0.0, 0.0, 1.0], unlit),
    ];
    if reverse {
        spawned.reverse();
    }
    for (at, color, material) in spawned {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(
            e,
            Transform3D::from_translation(at)
                .with_rotation(facing)
                .with_scale(Vec3::new(3.0, 1.0, 3.0)),
        );
        world.insert(
            e,
            MeshInstance3D::new(BuiltinMesh::Plane)
                .with_color(color)
                .with_material(material),
        );
    }
    app
}

#[test]
fn blended_quads_draw_back_to_front_whatever_order_they_were_spawned_in() {
    let mut shots = Vec::new();
    for reverse in [false, true] {
        let root = scratch_root();
        let mut session = Headless::new(quads(&root, reverse));
        let Some(shot) = shoot(&mut session, &format!("blend-order-{reverse}")) else {
            return;
        };
        shots.push(shot);
    }
    assert_eq!(shots[0].rgba, shots[1].rgba, "the same picture either way");
    let shot = &shots[0];
    // Where both quads cover the wall: blue over (green over red).
    let middle = (WIDTH / 2, HEIGHT / 2);
    let under = [0.5, 0.5, 0.0];
    let over = [under[0] * 0.75, under[1] * 0.75, 0.25];
    let want = encode(over);
    assert!(
        near(at(shot, middle), want, 2),
        "{:?}, the CPU's {want:?}",
        at(shot, middle)
    );
    // Drawn front to back instead, the green would land over the blue:
    // (0.25 blue over red) then green at 0.5.
    let wrong = encode([0.375, 0.5, 0.125]);
    assert!(!near(at(shot, middle), wrong, 2));
}
