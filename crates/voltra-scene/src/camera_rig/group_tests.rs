//! A follower framing several targets: the middle of their box, a zoom that
//! fits them, and members dropping out.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::tests::{at, near, placed, DT};
use super::*;
use crate::scene_id::SceneId;
use crate::{Camera, ComponentRegistry, Transform};

fn id(world: &World, entity: Entity) -> SceneId {
    *world.get::<SceneId>(entity).expect("an id")
}

/// A camera of size 2 framing `targets`, snapping (no smoothing).
fn group_rig(world: &mut World, targets: &[Entity], zoom: Option<FollowZoom>) -> Entity {
    let ids: Vec<SceneId> = targets.iter().map(|&t| id(world, t)).collect();
    let camera = placed(world, Vec2::ZERO);
    world.insert(camera, Camera::new(2.0));
    world.insert(
        camera,
        CameraFollow2D {
            smoothing: 0.0,
            zoom,
            ..CameraFollow2D::framing(ids)
        },
    );
    camera
}

fn size(world: &World, camera: Entity) -> f32 {
    world.get::<Camera>(camera).expect("a camera").size
}

#[test]
fn two_targets_are_framed_at_their_middle_and_zoomed_to_fit() {
    let mut world = World::new();
    let left = placed(&mut world, Vec2::new(-6.0, 1.0));
    let right = placed(&mut world, Vec2::new(10.0, 3.0));
    let zoom = FollowZoom {
        margin: Vec2::splat(1.0),
        min_size: 2.0,
        max_size: 20.0,
    };
    let camera = group_rig(&mut world, &[left, right], Some(zoom));
    update(&mut world, DT, 2.0, None);
    assert!(near(at(&world, camera), Vec2::new(2.0, 2.0)));
    // Sixteen wide plus a unit each side: half 9, over an aspect of 2.
    assert!(
        (size(&world, camera) - 4.5).abs() < 1e-4,
        "{}",
        size(&world, camera)
    );

    // They close in: it zooms in, down to its least size.
    world.get_mut::<Transform>(right).expect("t").translation = Vec2::new(-5.0, 1.0);
    update(&mut world, DT, 2.0, None);
    assert!(near(at(&world, camera), Vec2::new(-5.5, 1.0)));
    assert_eq!(size(&world, camera), 2.0);

    // Far apart, out to its greatest.
    world.get_mut::<Transform>(right).expect("t").translation = Vec2::new(200.0, 1.0);
    update(&mut world, DT, 2.0, None);
    assert_eq!(size(&world, camera), 20.0);
}

#[test]
fn the_zoom_eases_at_the_followers_smoothing() {
    let mut world = World::new();
    let a = placed(&mut world, Vec2::new(-4.0, 0.0));
    let b = placed(&mut world, Vec2::new(4.0, 0.0));
    let camera = group_rig(&mut world, &[a, b], Some(FollowZoom::default()));
    world
        .get_mut::<CameraFollow2D>(camera)
        .expect("f")
        .smoothing = 5.0;
    update(&mut world, DT, 1.0, None);
    let landed = size(&world, camera);
    world.get_mut::<Transform>(b).expect("t").translation = Vec2::new(10.0, 0.0);
    update(&mut world, DT, 1.0, None);
    let eased = size(&world, camera);
    assert!(eased > landed && eased < 9.0, "{landed} → {eased}");
}

#[test]
fn a_despawned_target_drops_out_and_all_gone_holds_still() {
    let mut world = World::new();
    let a = placed(&mut world, Vec2::new(-4.0, 0.0));
    let b = placed(&mut world, Vec2::new(4.0, 2.0));
    let camera = group_rig(&mut world, &[a, b], None);
    world
        .get_mut::<CameraFollow2D>(camera)
        .expect("f")
        .look_ahead = 1.0;
    update(&mut world, DT, 1.0, None);
    assert!(near(at(&world, camera), Vec2::new(0.0, 1.0)));

    world.despawn(b);
    update(&mut world, DT, 1.0, None);
    // On the one left — and the middle's jump is not a velocity to lead by.
    assert!(
        near(at(&world, camera), Vec2::new(-4.0, 0.0)),
        "{}",
        at(&world, camera)
    );

    world.despawn(a);
    world.get_mut::<Transform>(camera).expect("c").translation = Vec2::new(7.0, 7.0);
    update(&mut world, DT, 1.0, None);
    update(&mut world, DT, 1.0, None);
    assert_eq!(at(&world, camera), Vec2::new(7.0, 7.0));
}

#[test]
fn a_turned_camera_zooms_to_fit_the_group_turned() {
    let mut world = World::new();
    let low = placed(&mut world, Vec2::new(0.0, -8.0));
    let high = placed(&mut world, Vec2::new(0.0, 8.0));
    let zoom = FollowZoom {
        margin: Vec2::ZERO,
        min_size: 0.5,
        max_size: 50.0,
    };
    let camera = group_rig(&mut world, &[low, high], Some(zoom));
    world.get_mut::<Transform>(camera).expect("t").rotation = std::f32::consts::FRAC_PI_2;
    update(&mut world, DT, 2.0, None);
    // Sixteen tall in the world is sixteen across the turned view: half 8
    // over an aspect of 2.
    assert!(
        (size(&world, camera) - 4.0).abs() < 1e-4,
        "{}",
        size(&world, camera)
    );
}

#[test]
fn every_target_is_remapped_on_a_copy() {
    let mut world = World::new();
    let a = placed(&mut world, Vec2::ZERO);
    let b = placed(&mut world, Vec2::ONE);
    let camera = group_rig(&mut world, &[a, b], None);
    let (old_a, old_b) = (id(&world, a), id(&world, b));
    let (new_a, new_b) = (SceneId::new(), SceneId::new());
    ComponentRegistry::with_defaults().remap_refs(&mut world, camera, &|id| {
        if id == old_a {
            Some(new_a)
        } else if id == old_b {
            Some(new_b)
        } else {
            None
        }
    });
    let rig = world.get::<CameraFollow2D>(camera).expect("f");
    assert_eq!(rig.group(), vec![new_a, new_b]);
}

#[test]
fn an_old_single_target_file_loads_as_a_group_of_one() {
    let target = SceneId::new();
    let old = format!(
        "(target: Some({}), offset: (0.0, 1.0), smoothing: 4.0, dead_zone: (1.0, 0.5), look_ahead: 0.0)",
        ron::to_string(&target).expect("an id writes")
    );
    let rig: CameraFollow2D = ron::from_str(&old).expect("an old file reads");
    assert_eq!(rig.group(), vec![target]);
    assert!(rig.targets.is_empty() && rig.zoom.is_none());
    // And written back, it is the same file: nothing new appears.
    let written = ron::to_string(&rig).expect("writes");
    assert!(
        !written.contains("targets") && !written.contains("zoom"),
        "{written}"
    );
}

#[test]
fn a_target_named_twice_counts_once() {
    let one = SceneId::new();
    let rig = CameraFollow2D {
        targets: vec![one, one],
        ..CameraFollow2D::new(one)
    };
    assert_eq!(rig.group(), vec![one]);
}

#[test]
fn a_split_screen_camera_fits_its_half_not_the_window() {
    let mut world = World::new();
    let left = placed(&mut world, Vec2::new(-6.0, 0.0));
    let right = placed(&mut world, Vec2::new(6.0, 0.0));
    let zoom = FollowZoom {
        margin: Vec2::ZERO,
        min_size: 0.5,
        max_size: 50.0,
    };
    let camera = group_rig(&mut world, &[left, right], Some(zoom));
    world.get_mut::<Camera>(camera).expect("c").viewport = crate::camera::Viewport::LEFT;
    // A 2:1 window, the camera drawing into its left half: square, so
    // twelve wide needs a size of six, not three.
    update(&mut world, DT, 2.0, None);
    assert!(
        (size(&world, camera) - 6.0).abs() < 1e-4,
        "{}",
        size(&world, camera)
    );
}

#[test]
fn the_fit_makes_room_for_the_dead_zone_and_offset() {
    let mut world = World::new();
    let a = placed(&mut world, Vec2::new(-4.0, 0.0));
    let b = placed(&mut world, Vec2::new(4.0, 0.0));
    let zoom = FollowZoom {
        margin: Vec2::ZERO,
        min_size: 0.5,
        max_size: 50.0,
    };
    let camera = group_rig(&mut world, &[a, b], Some(zoom));
    {
        let rig = world.get_mut::<CameraFollow2D>(camera).expect("f");
        rig.dead_zone = Vec2::new(1.0, 0.0);
        rig.offset = Vec2::new(-2.0, 0.0);
    }
    update(&mut world, DT, 1.0, None);
    // Half the box, 4, plus a dead zone of 1 and an offset of 2 either way.
    assert!(
        (size(&world, camera) - 7.0).abs() < 1e-4,
        "{}",
        size(&world, camera)
    );
    // Wherever in the dead zone it sits, both stay in view.
    let x = at(&world, camera).x;
    assert!(x - 7.0 <= -4.0 && x + 7.0 >= 4.0);
}

#[test]
fn a_follower_naming_itself_leaves_itself_out() {
    let mut world = World::new();
    let player = placed(&mut world, Vec2::new(3.0, 0.0));
    let camera = group_rig(&mut world, &[player], None);
    let own = id(&world, camera);
    world
        .get_mut::<CameraFollow2D>(camera)
        .expect("f")
        .targets
        .push(own);
    update(&mut world, DT, 1.0, None);
    assert!(near(at(&world, camera), Vec2::new(3.0, 0.0)));
}
