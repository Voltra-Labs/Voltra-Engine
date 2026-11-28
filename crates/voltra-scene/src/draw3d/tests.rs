//! The draw list: what is culled, what is grouped, and in what order.

use voltra_assets::{AssetPath, BuiltinMesh, MeshPath, Meshes};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::glam::{Affine3A, Vec3};
use voltra_render::render3d::{View3D, Winding};

use super::{DrawList3D, PARALLEL_FROM};
use crate::{MeshInstance3D, SceneId, Transform3D};

/// At the origin looking down −Z, 90° high, square.
fn view(far: Option<f32>) -> View3D {
    View3D::perspective(
        Affine3A::IDENTITY,
        std::f32::consts::FRAC_PI_2,
        1.0,
        0.1,
        far,
    )
}

fn mesh(world: &mut World, mesh: BuiltinMesh, at: Vec3) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform3D::from_translation(at));
    world.insert(e, MeshInstance3D::new(mesh));
    e
}

fn drawn(list: &DrawList3D) -> usize {
    list.instances.len()
}

#[test]
fn a_mesh_behind_the_camera_and_one_past_the_far_plane_are_culled() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -5.0));
    mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, 5.0));
    mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -200.0));
    let infinite = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(drawn(&infinite), 2, "only the one behind is culled");
    assert_eq!(infinite.culled(), 1);
    let finite = DrawList3D::from_world(&world, &view(Some(100.0)), &mut meshes, None, None);
    assert_eq!(drawn(&finite), 1);
    assert_eq!(finite.culled(), 2);
}

#[test]
fn instances_of_a_mesh_are_one_draw_front_to_back() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let far = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -9.0));
    mesh(&mut world, BuiltinMesh::Sphere, Vec3::new(1.0, 0.0, -2.0));
    let near = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -3.0));
    mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -6.0));
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(list.draws.len(), 2, "cubes and the sphere");
    // The sphere is nearest, so its draw goes first.
    assert_eq!(list.draws[0].mesh, meshes.builtin(BuiltinMesh::Sphere));
    assert_eq!(list.draws[0].instances, 0..1);
    assert_eq!(list.draws[1].mesh, meshes.builtin(BuiltinMesh::Cube));
    assert_eq!(list.draws[1].instances, 1..4);
    let z = |i: usize| list.instances[i].model[2][3];
    assert_eq!([z(1), z(2), z(3)], [-3.0, -6.0, -9.0], "front to back");
    // The same world spawned in another order draws the same list.
    let mut other = World::new();
    for (m, at) in [
        (BuiltinMesh::Cube, -6.0),
        (BuiltinMesh::Cube, -3.0),
        (BuiltinMesh::Cube, -9.0),
    ] {
        mesh(&mut other, m, Vec3::new(0.0, 0.0, at));
    }
    mesh(&mut other, BuiltinMesh::Sphere, Vec3::new(1.0, 0.0, -2.0));
    let again = DrawList3D::from_world(&other, &view(None), &mut meshes, None, None);
    assert_eq!(again.instances, list.instances);
    assert_eq!(again.draws, list.draws);
    let _ = (far, near);
}

#[test]
fn zero_scale_despawned_and_broken_instances_are_not_drawn() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let point = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -5.0));
    world.insert(
        point,
        Transform3D::from_translation(Vec3::new(0.0, 0.0, -5.0)).with_scale(Vec3::ZERO),
    );
    let gone = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -4.0));
    let broken = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -3.0));
    world.insert(broken, Transform3D::from_translation(Vec3::splat(f32::NAN)));
    let kept = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -6.0));
    let mut list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(
        drawn(&list),
        2,
        "the collapsed one is culled, the broken one left out"
    );
    assert!(list.warned.contains(&broken), "and warned about");
    world.despawn(gone);
    list.rebuild(&world, &view(None), &mut meshes, None, None);
    assert_eq!(drawn(&list), 1);
    assert_eq!(list.instances[0].model[2][3], -6.0);
    world.despawn(kept);
    world.despawn(point);
    world.despawn(broken);
    list.rebuild(&world, &view(None), &mut meshes, None, None);
    assert!(list.is_empty() && list.instances.is_empty());
}

#[test]
fn a_mesh_that_will_not_load_is_a_magenta_cube() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let e = mesh(&mut world, BuiltinMesh::Sphere, Vec3::new(0.0, 0.0, -4.0));
    world.insert(
        e,
        MeshInstance3D::new(MeshPath::File(
            AssetPath::new("models/gone.glb").expect("a path"),
        ))
        .with_color([0.0, 1.0, 0.0, 1.0]),
    );
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(list.draws[0].mesh, meshes.builtin(BuiltinMesh::Cube));
    assert_eq!(list.instances[0].tint, voltra_assets::meshes::MISSING_TINT);
}

#[test]
fn a_child_mesh_is_placed_through_its_parent() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let parent = world.spawn();
    world.insert(parent, SceneId::new());
    world.insert(
        parent,
        Transform3D::from_translation(Vec3::new(0.0, 0.0, -10.0)),
    );
    let child = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 2.0, 0.0));
    crate::hierarchy::set_parent(&mut world, child, parent).expect("3D under 3D");
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    let m = list.instances[0].model;
    assert_eq!([m[0][3], m[1][3], m[2][3]], [0.0, 2.0, -10.0]);
}

#[test]
fn a_long_list_culls_the_same_across_jobs() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let side = 72;
    assert!(side * side >= PARALLEL_FROM);
    for i in 0..side * side {
        let (x, z) = ((i % side) as f32 - side as f32 / 2.0, -((i / side) as f32));
        let builtin = BuiltinMesh::ALL[i % BuiltinMesh::ALL.len()];
        mesh(&mut world, builtin, Vec3::new(x * 2.0, 0.0, z * 2.0 - 1.0));
    }
    let serial = DrawList3D::from_world(&world, &view(Some(80.0)), &mut meshes, None, None);
    let jobs = JobPool::new(3);
    let parallel =
        DrawList3D::from_world(&world, &view(Some(80.0)), &mut meshes, None, Some(&jobs));
    assert!(serial.culled() > 0 && drawn(&serial) > 0);
    assert_eq!(serial.instances, parallel.instances);
    assert_eq!(serial.draws, parallel.draws);
    assert_eq!(serial.draws.len(), BuiltinMesh::ALL.len());
}

#[test]
fn a_flattened_or_tiny_mesh_is_drawn_and_a_line_is_not() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    for (scale, at) in [
        (Vec3::new(10.0, 0.0, 10.0), -5.0),
        (Vec3::new(1.0, 0.0, 1.0), -6.0),
        (Vec3::splat(1.0e-4), -7.0),
        (Vec3::new(1.0, 0.0, 0.0), -8.0),
    ] {
        let e = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, at));
        world.insert(
            e,
            Transform3D::from_translation(Vec3::new(0.0, 0.0, at)).with_scale(scale),
        );
    }
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(drawn(&list), 3, "the line covers nothing");
    assert!(list.warned.is_empty(), "and none is worth a warning");
    let huge = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -9.0));
    world.insert(
        huge,
        Transform3D::from_translation(Vec3::new(0.0, 0.0, -9.0)).with_scale(Vec3::splat(1.0e20)),
    );
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(drawn(&list), 3);
    assert!(
        list.warned.contains(&huge),
        "an overflowing one is said once"
    );
}

#[test]
fn mirrored_instances_of_a_mesh_are_a_draw_of_their_own() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    for (x, flip) in [(-2.0, 1.0), (0.0, -1.0), (2.0, 1.0)] {
        let e = mesh(&mut world, BuiltinMesh::Cube, Vec3::new(x, 0.0, -5.0));
        world.insert(
            e,
            Transform3D::from_translation(Vec3::new(x, 0.0, -5.0))
                .with_scale(Vec3::new(flip, 1.0, 1.0)),
        );
    }
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    assert_eq!(list.draws.len(), 2);
    let mirrored: Vec<_> = list
        .draws
        .iter()
        .filter(|d| d.winding == Winding::Clockwise)
        .collect();
    assert_eq!(mirrored.len(), 1);
    assert_eq!(mirrored[0].instances.len(), 1);
    let x = list.instances[mirrored[0].instances.start as usize].model[0][3];
    assert_eq!(x, 0.0, "the flipped one");
}

#[test]
fn a_flattened_instance_is_a_flat_draw_of_its_own() {
    let mut world = World::new();
    let mut meshes = Meshes::new();
    let at = Vec3::new(0.0, 0.0, -5.0);
    let flat = mesh(&mut world, BuiltinMesh::Cube, at);
    world.insert(
        flat,
        Transform3D::from_translation(at)
            .with_rotation(voltra_render::glam::Quat::from_rotation_y(0.7))
            .with_scale(Vec3::new(1.0, 1.0, 0.0)),
    );
    mesh(&mut world, BuiltinMesh::Cube, Vec3::new(2.0, 0.0, -5.0));
    let list = DrawList3D::from_world(&world, &view(None), &mut meshes, None, None);
    let windings: Vec<Winding> = list.draws.iter().map(|d| d.winding).collect();
    assert_eq!(windings.len(), 2);
    assert!(windings.contains(&Winding::Flat), "{windings:?}");
    assert!(
        windings.contains(&Winding::CounterClockwise),
        "{windings:?}"
    );
}
