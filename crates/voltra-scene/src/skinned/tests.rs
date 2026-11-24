use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::generate::from_outline;
use super::pose::{bone_segments, deformed, skin_pose};
use super::weights::automatic;
use super::*;
use crate::hierarchy::{set_parent, world_matrix};
use crate::{Bone2D, ComponentRegistry, Name, SceneId, Skeleton2D, Transform};

fn named(world: &mut World, name: &str, transform: Transform) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, transform);
    e
}

/// A skeleton at `at` holding an arm hanging down: an upper bone and a
/// lower bone each a unit long, and the skinned arm — a quarter wide, two
/// long — on the root, weighted automatically.
fn arm(world: &mut World, at: Vec2) -> (Entity, Entity, Entity) {
    let root = named(world, "Rig", Transform::from_translation(at));
    world.insert(root, Skeleton2D::default());
    let down = Transform::default().with_rotation(-std::f32::consts::FRAC_PI_2);
    let upper = named(world, "upper", down);
    world.insert(upper, Bone2D::new(1.0, down));
    set_parent(world, upper, root).expect("parented");
    let below = Transform::from_translation(Vec2::X);
    let lower = named(world, "lower", below);
    world.insert(lower, Bone2D::new(1.0, below));
    set_parent(world, lower, upper).expect("parented");
    let outline = [
        Vec2::new(-0.125, 0.0),
        Vec2::new(-0.125, -2.0),
        Vec2::new(0.125, -2.0),
        Vec2::new(0.125, 0.0),
    ];
    let mut skinned = SkinnedSprite {
        bones: vec!["upper".to_owned(), "upper/lower".to_owned()],
        mesh: from_outline(&outline, 0.125, |p| p),
        ..SkinnedSprite::default()
    };
    let segments = bone_segments(world, root, &skinned);
    automatic(&mut skinned.mesh, &segments);
    world.insert(root, skinned);
    (root, upper, lower)
}

fn pose_of(world: &World, entity: Entity) -> (SkinnedSprite, super::pose::SkinPose) {
    let skinned = world.get::<SkinnedSprite>(entity).expect("skinned").clone();
    let pose = skin_pose(world, entity, &skinned, &|e| world_matrix(world, e));
    (skinned, pose)
}

#[test]
fn at_rest_every_bone_is_the_mesh_s_own_placement() {
    let mut world = World::new();
    let (root, _, _) = arm(&mut world, Vec2::new(3.0, 1.0));
    let (skinned, pose) = pose_of(&world, root);
    for bone in &pose.bones {
        assert!(bone.abs_diff_eq(pose.rigid, 1e-5), "{bone:?}");
    }
    let drawn = deformed(&skinned, &pose);
    for (point, at) in skinned.mesh.vertices.iter().zip(drawn) {
        assert!(at.distance(point.position + Vec2::new(3.0, 1.0)) < 1e-4);
    }
}

#[test]
fn bending_the_lower_bone_carries_the_forearm_and_shares_the_elbow() {
    let mut world = World::new();
    let (root, _, lower) = arm(&mut world, Vec2::ZERO);
    if let Some(t) = world.get_mut::<Transform>(lower) {
        t.rotation = std::f32::consts::FRAC_PI_2;
    }
    let (skinned, pose) = pose_of(&world, root);
    let drawn = deformed(&skinned, &pose);
    let near = |p: Vec2| {
        skinned
            .mesh
            .vertices
            .iter()
            .position(|v| v.position.distance(p) < 1e-4)
            .expect("a vertex there")
    };
    // The hand, two units down at rest, swings round the elbow at (0, -1):
    // a quarter turn counter-clockwise puts it a unit to the right.
    let hand = drawn[near(Vec2::new(0.125, -2.0))];
    assert!(hand.distance(Vec2::new(1.0, -0.875)) < 0.02, "{hand}");
    // The shoulder does not move.
    let shoulder = drawn[near(Vec2::new(0.125, 0.0))];
    assert!(
        shoulder.distance(Vec2::new(0.125, 0.0)) < 1e-3,
        "{shoulder}"
    );
    // The elbow's middle is half on each bone: between where each would
    // put it alone.
    let elbow = near(Vec2::new(-0.125, -1.0));
    let held: Vec<f32> = skinned.mesh.vertices[elbow]
        .weights
        .iter()
        .map(|w| w.1)
        .collect();
    assert!(
        held.len() >= 2 && held.iter().all(|w| (w - 0.5).abs() < 0.05),
        "{held:?}"
    );
}

#[test]
fn a_missing_bone_holds_its_vertices_where_the_mesh_is() {
    let mut world = World::new();
    let (root, _, _) = arm(&mut world, Vec2::new(1.0, 0.0));
    if let Some(s) = world.get_mut::<SkinnedSprite>(root) {
        s.bones[1] = "nowhere".to_owned();
    }
    let (_, pose) = pose_of(&world, root);
    assert_eq!(pose.bones[1], Mat3::from_translation(Vec2::new(1.0, 0.0)));
}

#[test]
fn the_gpu_mesh_keeps_four_weights_and_drops_bad_ones() {
    let skinned = SkinnedSprite {
        bones: vec!["a".into()],
        mesh: SkinMesh {
            vertices: vec![
                SkinPoint {
                    position: Vec2::ZERO,
                    uv: Vec2::ZERO,
                    weights: vec![(0, 1.0), (5, 1.0)],
                },
                SkinPoint {
                    position: Vec2::X,
                    uv: Vec2::X,
                    weights: Vec::new(),
                },
                SkinPoint {
                    position: Vec2::Y,
                    uv: Vec2::Y,
                    weights: vec![(0, 0.5)],
                },
            ],
            triangles: vec![[0, 1, 2]],
        },
        ..SkinnedSprite::default()
    };
    let (vertices, indices) = skinned.gpu_mesh();
    assert_eq!(indices, vec![0, 1, 2]);
    assert_eq!(
        vertices[0].weights,
        [1.0, 0.0, 0.0, 0.0],
        "bone 5 is not in the list"
    );
    assert_eq!(
        vertices[1].bones[0], RIGID_SLOT,
        "held by none: the mesh's own"
    );
    let broken = SkinnedSprite {
        mesh: SkinMesh {
            triangles: vec![[0, 1, 9]],
            ..skinned.mesh.clone()
        },
        ..skinned
    };
    assert!(broken.gpu_mesh().1.is_empty());
}

#[test]
fn painted_weights_round_trip_through_a_scene_file() {
    let mut world = World::new();
    let (root, _, _) = arm(&mut world, Vec2::ZERO);
    if let Some(s) = world.get_mut::<SkinnedSprite>(root) {
        super::weights::paint(&mut s.mesh, Vec2::new(0.0, -0.5), 0.4, 1, 0.3);
    }
    let painted = world.get::<SkinnedSprite>(root).expect("skinned").clone();
    let registry = ComponentRegistry::with_defaults();
    let path = std::env::temp_dir().join(format!(
        "voltra-skinned-{}-{:?}.ron",
        std::process::id(),
        std::thread::current().id()
    ));
    crate::format::save(&world, &registry, &path).expect("saved");
    let text = std::fs::read_to_string(&path).expect("the file");
    assert!(text.contains("weights:"), "the weights are in the file");
    let mut loaded = World::new();
    crate::format::load(&path, &registry, &mut loaded).expect("loaded");
    let _ = std::fs::remove_file(&path);
    let (_, back) = loaded
        .query::<SkinnedSprite>()
        .next()
        .expect("the skinned sprite came back");
    assert_eq!(back.mesh, painted.mesh, "every weight, as painted");
    assert_eq!(back.bones, painted.bones);
}
