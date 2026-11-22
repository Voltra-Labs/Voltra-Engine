//! A camera following a moving target, and shaking.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::hierarchy::{resolve_parents, set_parent};
use crate::scene_id::SceneId;
use crate::{Camera, ComponentRegistry, Transform};

const DT: f32 = 1.0 / 60.0;

fn placed(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::from_translation(at));
    entity
}

fn rig(world: &mut World, target: Entity, follow: CameraFollow2D) -> Entity {
    let camera = placed(world, Vec2::ZERO);
    world.insert(camera, Camera::new(2.0));
    let id = *world.get::<SceneId>(target).expect("an id");
    world.insert(
        camera,
        CameraFollow2D {
            target: Some(id),
            ..follow
        },
    );
    camera
}

fn at(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).expect("placed").translation
}

fn near(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-4
}

#[test]
fn it_lands_on_its_target_then_eases_after_it_at_any_frame_rate() {
    let follow = CameraFollow2D {
        offset: Vec2::new(0.0, 0.5),
        ..CameraFollow2D::default()
    };
    // The same scene twice: one run at one frame of 2·DT, one at two of DT.
    let scene = || {
        let mut world = World::new();
        let player = placed(&mut world, Vec2::new(5.0, 1.0));
        let camera = rig(&mut world, player, follow.clone());
        update(&mut world, DT, 1.0);
        assert!(
            near(at(&world, camera), Vec2::new(5.0, 1.5)),
            "snapped: {}",
            at(&world, camera)
        );
        world.get_mut::<Transform>(player).expect("p").translation = Vec2::new(7.0, 1.0);
        (world, camera)
    };
    let (mut once, camera) = scene();
    let (mut twice, _) = scene();
    update(&mut once, 2.0 * DT, 1.0);
    update(&mut twice, DT, 1.0);
    update(&mut twice, DT, 1.0);
    assert!(
        near(at(&once, camera), at(&twice, camera)),
        "{} vs {}",
        at(&once, camera),
        at(&twice, camera)
    );
    let moved = at(&once, camera).x - 5.0;
    assert!(moved > 0.0 && moved < 2.0, "part of the way: {moved}");
}

#[test]
fn a_dead_zone_holds_until_the_target_leaves_it() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::ZERO);
    let follow = CameraFollow2D {
        smoothing: 0.0,
        dead_zone: Vec2::new(1.0, 0.5),
        ..CameraFollow2D::default()
    };
    let camera = rig(&mut world, player, follow);
    update(&mut world, DT, 1.0);
    world.get_mut::<Transform>(player).expect("p").translation = Vec2::new(0.8, -0.3);
    update(&mut world, DT, 1.0);
    assert!(
        near(at(&world, camera), Vec2::ZERO),
        "inside: {}",
        at(&world, camera)
    );
    world.get_mut::<Transform>(player).expect("p").translation = Vec2::new(3.0, 0.0);
    update(&mut world, DT, 1.0);
    assert!(
        near(at(&world, camera), Vec2::new(2.0, 0.0)),
        "to the edge: {}",
        at(&world, camera)
    );
}

#[test]
fn bounds_keep_the_view_inside_and_centre_a_view_too_big_for_them() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::new(-50.0, 0.0));
    let follow = CameraFollow2D {
        bounds: Some(FollowBounds {
            min: Vec2::new(-10.0, -1.0),
            max: Vec2::new(10.0, 1.0),
        }),
        ..CameraFollow2D::default()
    };
    let camera = rig(&mut world, player, follow);
    // A size-2 camera at aspect 1.5 shows 3 either way across, 2 up and down.
    update(&mut world, DT, 1.5);
    assert!(
        near(at(&world, camera), Vec2::new(-7.0, 0.0)),
        "{}",
        at(&world, camera)
    );
}

#[test]
fn looking_ahead_leads_a_moving_target() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::ZERO);
    let follow = CameraFollow2D {
        look_ahead: 0.5,
        smoothing: 0.0,
        ..CameraFollow2D::default()
    };
    let camera = rig(&mut world, player, follow);
    for i in 1..=30 {
        world.get_mut::<Transform>(player).expect("p").translation =
            Vec2::new(i as f32 * 4.0 * DT, 0.0);
        update(&mut world, DT, 1.0);
    }
    let lead = at(&world, camera).x - at(&world, player).x;
    assert!((lead - 2.0).abs() < 1e-3, "half a second at 4 u/s: {lead}");
}

#[test]
fn a_missing_target_holds_it_still() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::new(3.0, 0.0));
    let camera = rig(&mut world, player, CameraFollow2D::default());
    update(&mut world, DT, 1.0);
    world.despawn(player);
    world.get_mut::<Transform>(camera).expect("c").translation = Vec2::new(1.0, 1.0);
    update(&mut world, DT, 1.0);
    assert_eq!(at(&world, camera), Vec2::new(1.0, 1.0));
}

#[test]
fn a_parented_follower_lands_in_the_world_where_it_should() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::new(4.0, 0.0));
    let holder = placed(&mut world, Vec2::new(1.0, 1.0));
    world.get_mut::<Transform>(holder).expect("h").scale = Vec2::splat(2.0);
    let camera = rig(&mut world, player, CameraFollow2D::default());
    set_parent(&mut world, camera, holder).expect("a plain reparent");
    resolve_parents(&mut world);
    update(&mut world, DT, 1.0);
    assert!(near(
        super::world_position(&world, camera),
        Vec2::new(4.0, 0.0)
    ));
}

#[test]
fn a_shake_fades_and_leaves_the_camera_where_it_rests() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::new(2.0, 0.0));
    let camera = rig(&mut world, player, CameraFollow2D::default());
    world.insert(camera, CameraShake::default());
    update(&mut world, DT, 1.0);
    world
        .get_mut::<CameraShake>(camera)
        .expect("s")
        .add_trauma(1.0);
    let mut moved = 0.0f32;
    for _ in 0..120 {
        update(&mut world, DT, 1.0);
        moved = moved.max(at(&world, camera).distance(Vec2::new(2.0, 0.0)));
    }
    assert!(moved > 0.05, "it shook: {moved}");
    assert_eq!(world.get::<CameraShake>(camera).expect("s").trauma, 0.0);
    assert!(
        near(at(&world, camera), Vec2::new(2.0, 0.0)),
        "back at rest: {}",
        at(&world, camera)
    );
}

#[test]
fn a_copy_of_a_camera_and_its_target_follows_the_copied_target() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::ZERO);
    let camera = rig(&mut world, player, CameraFollow2D::default());
    let old = *world.get::<SceneId>(player).expect("an id");
    let new = SceneId::new();
    ComponentRegistry::with_defaults()
        .remap_refs(&mut world, camera, &|id| (id == old).then_some(new));
    assert_eq!(
        world.get::<CameraFollow2D>(camera).expect("f").target,
        Some(new)
    );
}
