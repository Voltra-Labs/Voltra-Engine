//! The 3D draw list: ten thousand mesh instances culled against a view and
//! grouped into instanced draws, alone and across the frame's job pool.

use voltra_assets::{BuiltinMesh, Meshes};
use voltra_ecs::World;
use voltra_jobs::JobPool;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::View3D;
use voltra_scene::{DrawList3D, MeshInstance3D, SceneId, Transform3D};

use super::Bench;
use crate::harness::{iter, measure, Config};

const INSTANCES: usize = 10_000;

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("draw3d/cull-group/10k", 30.0, |config| {
            cull_and_group(config, None)
        }),
        Bench::new("draw3d/cull-group-jobs/10k", 30.0, |config| {
            let jobs = JobPool::with_available_parallelism();
            cull_and_group(config, Some(&jobs))
        }),
    ]
}

/// A 100 × 100 field of meshes, every built-in one in turn, each turned
/// and sized its own way, seen by a camera standing at one edge: about
/// half are outside its view, so culling has work either way.
fn field() -> World {
    let mut world = World::new();
    let side = (INSTANCES as f32).sqrt().ceil() as usize;
    for i in 0..INSTANCES {
        let (x, z) = ((i % side) as f32, (i / side) as f32);
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(
            entity,
            Transform3D::from_translation(Vec3::new(x * 2.0 - side as f32, 0.0, -z * 2.0))
                .with_rotation(Quat::from_rotation_y(i as f32 * 0.37))
                .with_scale(Vec3::splat(0.5 + (i % 7) as f32 * 0.1)),
        );
        let mesh = BuiltinMesh::ALL[i % BuiltinMesh::ALL.len()];
        world.insert(entity, MeshInstance3D::new(mesh));
    }
    world
}

fn cull_and_group(config: &Config, jobs: Option<&JobPool>) -> super::Outcome {
    let world = field();
    let mut meshes = Meshes::new();
    let placement = Affine3A::from_translation(Vec3::new(0.0, 4.0, 8.0));
    let view = View3D::perspective(placement, 1.0, 16.0 / 9.0, 0.1, Some(150.0));
    let mut list = DrawList3D::default();
    let stats = measure(
        config,
        1,
        iter(|| list.rebuild(&world, &view, &mut meshes, jobs)),
    );
    debug_assert!(!list.is_empty() && list.culled() > 0);
    stats.into()
}
