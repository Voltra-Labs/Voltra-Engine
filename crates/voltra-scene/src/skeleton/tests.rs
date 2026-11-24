use voltra_render::glam::Vec2;

use super::ik::{apply, problem, IkProblem};
use super::*;
use crate::hierarchy::{set_parent, world_matrix};
use crate::{Name, Transform};

/// A skeleton at `at` with an arm: an upper bone one unit long and a lower
/// bone one unit long hung at its tip, the lower one reaching for a target
/// named "target" placed at `goal` under the root.
fn arm(world: &mut World, at: Vec2, goal: Vec2) -> (Entity, Entity, Entity, Entity) {
    let root = named(world, "Rig", Transform::from_translation(at));
    world.insert(root, Skeleton2D::default());
    let upper = named(world, "upper", Transform::default());
    world.insert(upper, Bone2D::new(1.0, Transform::default()));
    set_parent(world, upper, root).expect("parented");
    let lower = named(world, "lower", Transform::from_translation(Vec2::X));
    world.insert(
        lower,
        Bone2D::new(1.0, Transform::from_translation(Vec2::X)),
    );
    set_parent(world, lower, upper).expect("parented");
    world.insert(lower, TwoBoneIk::default());
    let target = named(world, "target", Transform::from_translation(goal));
    set_parent(world, target, root).expect("parented");
    (root, upper, lower, target)
}

fn named(world: &mut World, name: &str, transform: Transform) -> Entity {
    let e = world.spawn();
    world.insert(e, crate::SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, transform);
    e
}

fn tip(world: &World, lower: Entity) -> Vec2 {
    world_matrix(world, lower).transform_point2(Vec2::X)
}

fn elbow(world: &World, lower: Entity) -> Vec2 {
    world_matrix(world, lower).transform_point2(Vec2::ZERO)
}

#[test]
fn the_tip_reaches_a_target_in_reach() {
    let mut world = World::new();
    let goal = Vec2::new(1.2, 0.9);
    let (root, _, lower, _) = arm(&mut world, Vec2::new(3.0, -1.0), goal);
    apply(&mut world);
    let reached = tip(&world, lower);
    let wanted = Vec2::new(3.0, -1.0) + goal;
    assert!(reached.distance(wanted) < 1e-4, "{reached} for {wanted}");
    // The elbow bends to the left of the reach by default.
    let at = world_matrix(&world, root).transform_point2(Vec2::ZERO);
    assert!((wanted - at).perp_dot(elbow(&world, lower) - at) > 0.0);
    // Solved, a second pass changes nothing.
    let before = elbow(&world, lower);
    apply(&mut world);
    assert!(elbow(&world, lower).distance(before) < 1e-5);
}

#[test]
fn flipped_it_bends_the_other_way() {
    let mut world = World::new();
    let goal = Vec2::new(0.5, 1.2);
    let (_, _, lower, _) = arm(&mut world, Vec2::ZERO, goal);
    if let Some(ik) = world.get_mut::<TwoBoneIk>(lower) {
        ik.flip = true;
    }
    apply(&mut world);
    assert!(tip(&world, lower).distance(goal) < 1e-4);
    assert!(goal.perp_dot(elbow(&world, lower)) < 0.0, "on the right");
}

#[test]
fn a_target_out_of_reach_gets_the_limb_straight_towards_it() {
    let mut world = World::new();
    let (_, _, lower, _) = arm(&mut world, Vec2::ZERO, Vec2::new(0.0, 5.0));
    apply(&mut world);
    assert!(tip(&world, lower).distance(Vec2::new(0.0, 2.0)) < 1e-4);
    assert!(elbow(&world, lower).distance(Vec2::new(0.0, 1.0)) < 1e-4);
}

#[test]
fn a_mirrored_skeleton_still_reaches() {
    let mut world = World::new();
    let goal = Vec2::new(1.0, 1.0);
    let (root, _, lower, target) = arm(&mut world, Vec2::ZERO, goal);
    if let Some(t) = world.get_mut::<Transform>(root) {
        t.scale = Vec2::new(-1.0, 1.0);
    }
    apply(&mut world);
    let wanted = world_matrix(&world, target).transform_point2(Vec2::ZERO);
    assert!(tip(&world, lower).distance(wanted) < 1e-4);
}

#[test]
fn a_limb_that_cannot_be_solved_is_left_and_says_why() {
    let mut world = World::new();
    let (_, upper, lower, target) = arm(&mut world, Vec2::ZERO, Vec2::ONE);
    if let Some(ik) = world.get_mut::<TwoBoneIk>(lower) {
        ik.target = "nowhere".to_owned();
    }
    assert_eq!(problem(&world, lower), Some(IkProblem::NoTarget));
    let before = elbow(&world, lower);
    apply(&mut world);
    assert_eq!(elbow(&world, lower), before);
    world.remove::<Bone2D>(upper);
    assert_eq!(problem(&world, lower), Some(IkProblem::NoUpperBone));
    let _ = target;
}

#[test]
fn bones_are_found_placed_picked_and_reset() {
    let mut world = World::new();
    let (root, upper, lower, _) = arm(&mut world, Vec2::ZERO, Vec2::ONE);
    assert_eq!(bones_of(&world, root), vec![upper, lower]);
    assert_eq!(skeleton_of(&world, lower), Some(root));
    assert_eq!(bone_at(&world, Vec2::new(1.5, 0.05), 0.1), Some(lower));
    assert_eq!(bone_at(&world, Vec2::new(0.5, 0.5), 0.1), None);
    if let Some(t) = world.get_mut::<Transform>(upper) {
        t.rotation = 1.0;
    }
    reset_to_rest(&mut world, root);
    assert_eq!(world.get::<Transform>(upper).map(|t| t.rotation), Some(0.0));
}

#[test]
fn bones_and_limbs_round_trip_and_old_bones_read() {
    let bone = Bone2D::new(0.7, Transform::from_translation(Vec2::X).with_rotation(0.3));
    let text = ron::to_string(&bone).expect("writes");
    assert_eq!(ron::from_str::<Bone2D>(&text).expect("reads"), bone);
    let ik: TwoBoneIk = ron::from_str("(target:\"goal\")").expect("defaults fill in");
    assert!(ik.enabled && !ik.flip);
}
