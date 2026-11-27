//! Skinned sprite meshes (stage 103), screenshotted headless: an arm's mesh
//! made from its texture's traced outline, weighted by distance to two
//! bones, bends smoothly round its elbow when the lower bone turns — the
//! outside of the bend is filled in, where two cut-out parts hinged at the
//! elbow leave a square gap, and a mesh held wholly by the nearest bone a
//! sharp chamfer — and its hand lands where the lower bone carries it.
//! Skips without a GPU adapter.

use std::path::Path;

use voltra_assets::{AssetPath, Silhouettes};
use voltra_core::{App, Headless, Screenshot, ScreenshotError, WindowConfig};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::set_parent;
use voltra_scene::skinned::{generate, pose, weights};
use voltra_scene::{Bone2D, Camera, Name, SceneId, Skeleton2D, SkinnedSprite, Transform};

const SHOT: u32 = 256;
const PER_UNIT: f32 = 64.0;
const PPU: f32 = 32.0;

/// A 32 × 64 arm — a unit wide, two long — orange, its corners cut away
/// so the trace is not just the texture's box.
fn arm_texture(root: &Path) {
    let mut image = image::RgbaImage::new(32, 64);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let corner = !(3..=28).contains(&x) && !(3..=60).contains(&y);
        *pixel = if corner {
            image::Rgba([0, 0, 0, 0])
        } else {
            image::Rgba([255, 140, 40, 255])
        };
    }
    image.save(root.join("arm.png")).expect("the arm");
}

fn named(world: &mut World, name: &str, transform: Transform) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, transform);
    e
}

/// The skeleton — shoulder at (0, 1), elbow at the origin, an arm two
/// units long hanging between — with the lower bone turned by `bend`, and
/// the arm's mesh traced from its texture and weighted, smoothly or held
/// wholly by the nearest bone.
fn arm(root_dir: &Path, bend: f32, smooth: bool) -> Headless {
    let mut app = App::new(WindowConfig::default()).with_asset_root(root_dir.to_path_buf());
    let world = &mut app.world;
    let camera = world.spawn();
    world.insert(camera, Transform::default());
    world.insert(camera, Camera::new(2.0));
    let root = named(
        world,
        "Rig",
        Transform::from_translation(Vec2::new(0.0, 1.0)),
    );
    world.insert(root, Skeleton2D::default());
    let down = Transform::default().with_rotation(-std::f32::consts::FRAC_PI_2);
    let upper = named(world, "upper", down);
    world.insert(upper, Bone2D::new(1.0, down));
    set_parent(world, upper, root).expect("parented");
    let rest = Transform::from_translation(Vec2::X);
    let lower = named(world, "lower", rest.with_rotation(bend));
    world.insert(lower, Bone2D::new(1.0, rest));
    set_parent(world, lower, upper).expect("parented");
    // The art centred half-way down the arm, as a sprite would hang.
    let art = named(
        world,
        "arm",
        Transform::from_translation(Vec2::new(0.0, -1.0)),
    );
    set_parent(world, art, root).expect("parented");

    let path = AssetPath::new("arm.png").expect("a path");
    let mut silhouettes = Silhouettes::new(root_dir);
    let rect = [0, 0, 32, 64];
    silhouettes.ensure(&path, rect, voltra_assets::silhouette::ALPHA_THRESHOLD);
    let traced = silhouettes
        .get(&path, rect, voltra_assets::silhouette::ALPHA_THRESHOLD)
        .and_then(|outlines| outlines.first().cloned())
        .expect("the arm's outline");
    let size = Vec2::new(32.0, 64.0);
    let outline = generate::outline_in_units(&traced, size, PPU);
    let mut skinned = SkinnedSprite {
        texture: Some(path),
        pixels_per_unit: PPU,
        bones: vec!["upper".to_owned(), "upper/lower".to_owned()],
        mesh: generate::from_outline(&outline, 0.125, |p| generate::uv_at(p, size, PPU)),
        ..SkinnedSprite::default()
    };
    let segments = pose::bone_segments(world, art, &skinned);
    weights::automatic(&mut skinned.mesh, &segments);
    if !smooth {
        for point in &mut skinned.mesh.vertices {
            if let Some(&(bone, _)) = point.weights.iter().max_by(|a, b| a.1.total_cmp(&b.1)) {
                point.weights = vec![(bone, 1.0)];
            }
        }
    }
    world.insert(art, skinned);
    Headless::new(app)
}

fn shoot(session: &mut Headless) -> Option<Screenshot> {
    match session.screenshot(SHOT, SHOT) {
        Ok(shot) => Some(shot),
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// Whether the arm's orange covers world point `at`.
fn arm_at(shot: &Screenshot, at: Vec2) -> bool {
    let x = (SHOT as f32 / 2.0 + at.x * PER_UNIT) as u32;
    let y = (SHOT as f32 / 2.0 - at.y * PER_UNIT) as u32;
    shot.pixel(x, y)
        .is_some_and(|p| p[0] > 200 && p[1] > 90 && p[1] < 190 && p[2] < 90)
}

fn root() -> voltra_testkit::Scratch {
    let root = voltra_testkit::scratch_root();
    arm_texture(&root);
    root
}

#[test]
fn a_skinned_arm_hangs_straight_at_rest() {
    let root = root();
    let Some(shot) = shoot(&mut arm(&root, 0.0, true)) else {
        return;
    };
    for y in [0.9, 0.0, -0.9] {
        assert!(arm_at(&shot, Vec2::new(0.0, y)), "the arm at y = {y}");
    }
    assert!(!arm_at(&shot, Vec2::new(0.8, 0.0)), "and only there");
}

#[test]
fn a_skinned_arm_bends_smoothly_round_its_elbow() {
    let root = root();
    let bend = std::f32::consts::FRAC_PI_2;
    let Some(smooth) = shoot(&mut arm(&root, bend, true)) else {
        return;
    };
    let rigid = shoot(&mut arm(&root, bend, false)).expect("a shot");
    // The forearm swung a quarter turn round the elbow: the hand is a unit
    // to the right of it, and nothing is left hanging below.
    for shot in [&smooth, &rigid] {
        assert!(arm_at(shot, Vec2::new(0.85, 0.0)), "the hand swung right");
        assert!(arm_at(shot, Vec2::new(0.0, 0.7)), "the upper arm stays");
        assert!(!arm_at(shot, Vec2::new(0.0, -0.8)), "nothing hangs below");
    }
    // The outside of the bend, below and left of the elbow: two cut-out
    // rectangles hinged at the elbow cover nothing there (the upper one
    // stops at y = 0, the forearm starts at x = 0) — a square notch half a
    // width deep. The blended mesh fills the corner out most of the way to
    // its apex, a quarter unit along the diagonal, and further than a mesh
    // held wholly by the nearest bone, whose triangles across the joint
    // only stretch.
    let reach = |shot: &Screenshot| {
        (0..40)
            .map(|i| i as f32 * 0.0125)
            .take_while(|t| arm_at(shot, Vec2::splat(-t)))
            .last()
            .unwrap_or(0.0)
    };
    let (rounded, chamfered) = (reach(&smooth), reach(&rigid));
    assert!(
        rounded >= 0.2,
        "the smooth elbow is filled out to {rounded}"
    );
    assert!(
        rounded > chamfered,
        "rounder than a hard-weighted one: {rounded} against {chamfered}"
    );
}
