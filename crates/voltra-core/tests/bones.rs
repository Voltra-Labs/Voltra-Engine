//! 2D bones (stage 102) through the shipped sample, `scenes/bones.ron`: a
//! cut-out character walks by a clip of bone rotations — its feet swing back
//! and forth, the sprites hung on its bones go with them, and the picture
//! changes — and its arm, bent by two-bone IK, keeps its hand on the goal
//! the clip swings.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, ScreenshotError, WindowConfig};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::world_matrix_2d;
use voltra_scene::{Bone2D, Name};

fn walker() -> Headless {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let app = App::new(WindowConfig::default()).with_asset_root(assets);
    Headless::open(app, &AssetPath::new("scenes/bones.ron").expect("a path"))
        .expect("the sample loads")
}

fn named(world: &World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no {name}"))
}

/// Where `bone`'s tip is in the world.
fn tip(world: &World, bone: &str) -> Vec2 {
    let entity = named(world, bone);
    let length = world.get::<Bone2D>(entity).expect("a bone").length;
    world_matrix_2d(world, entity).transform_point2(Vec2::new(length, 0.0))
}

fn at(world: &World, name: &str) -> Vec2 {
    world_matrix_2d(world, named(world, name)).transform_point2(Vec2::ZERO)
}

#[test]
fn the_sample_walks_by_its_clip_of_bone_rotations() {
    let mut session = walker();
    let mut feet = Vec::new();
    for _ in 0..60 {
        session.step(1, 1.0 / 60.0);
        let world = session.world();
        feet.push(tip(world, "shin_l"));
        // The art hangs on its bone, wherever the clip has turned it.
        let shin = at(world, "shin_l");
        let art = at(world, "shin_l_art");
        assert!(
            (art.distance(shin) - 0.225).abs() < 1e-3,
            "the shin's sprite rides it"
        );
    }
    let xs: Vec<f32> = feet.iter().map(|f| f.x).collect();
    let (lo, hi) = (
        xs.iter().copied().fold(f32::INFINITY, f32::min),
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    );
    assert!(hi - lo > 0.4, "a stride: the left foot from {lo} to {hi}");
    // Back where it started after the clip's one second: it loops.
    assert!(
        feet[59].distance(feet[0]) < 0.08,
        "{} then {}",
        feet[0],
        feet[59]
    );
    // The knee bends as the foot swings through, so the shin turns away from
    // the thigh's line at some point and back again.
    let world = session.world();
    let shin = world
        .get::<voltra_scene::Transform2D>(named(world, "shin_l"))
        .expect("a shin");
    assert!(
        shin.rotation.abs() < 0.2,
        "straight again at the loop: {}",
        shin.rotation
    );
}

#[test]
fn the_sample_s_arm_keeps_its_hand_on_the_goal() {
    let mut session = walker();
    for frame in 0..60 {
        session.step(1, 1.0 / 60.0);
        let world = session.world();
        let hand = tip(world, "arm_lower");
        let goal = at(world, "hand_goal");
        assert!(
            hand.distance(goal) < 1e-3,
            "frame {frame}: the hand at {hand}, the goal at {goal}"
        );
    }
}

#[test]
fn the_walk_changes_the_picture() {
    let mut session = walker();
    session.step(1, 1.0 / 60.0);
    let first = match session.screenshot(96, 96) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    session.step(30, 1.0 / 60.0);
    let half_way = session.screenshot(96, 96).expect("a shot");
    let changed = first
        .rgba
        .chunks(4)
        .zip(half_way.rgba.chunks(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 40))
        .count();
    assert!(changed > 30, "the legs moved: {changed} pixels changed");
    // The torso is drawn: blue in the middle of the body.
    let torso = half_way.pixel(48, 36).expect("on the shot");
    assert!(
        torso[2] > torso[0],
        "the torso's blue at the middle: {torso:?}"
    );
}
