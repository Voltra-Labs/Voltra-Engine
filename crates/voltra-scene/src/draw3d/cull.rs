//! Which mesh instances a view sees: gathered from the world, then each
//! one's world bounds tested against the frustum.

use std::collections::HashSet;

use voltra_assets::{meshes::MISSING_TINT, Handle, Meshes};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::glam::Affine3A;
use voltra_render::render3d::{Aabb3, Instance3D, InstanceRefused, Mesh3D, View3D};

use super::PARALLEL_FROM;
use crate::hierarchy::WorldTransforms3D;
use crate::mesh_instance3d::MeshInstance3D;

/// One mesh instance on its way to the list.
#[derive(Debug, Clone)]
pub(super) struct Candidate {
    pub entity: Entity,
    pub mesh: Handle<Mesh3D>,
    pub model: Affine3A,
    pub tint: [f32; 4],
    pub bounds: Aabb3,
    /// Filled by [`cull`]: `None` when it is not drawn.
    pub instance: Option<Instance3D>,
    /// Filled by [`cull`]: why a seen candidate has no instance.
    pub refused: Option<InstanceRefused>,
    /// Whether its model mirrors space: drawn with clockwise fronts.
    pub mirrored: bool,
    /// How far in front of the eye its world bounds' centre is.
    pub depth: f32,
}

/// Every mesh instance in `world`, placed: into `out`, emptied first.
/// A model that is not finite is left out and warned about once per
/// entity in `warned`.
pub(super) fn gather(
    world: &World,
    meshes: &mut Meshes,
    out: &mut Vec<Candidate>,
    warned: &mut HashSet<Entity>,
) {
    out.clear();
    let transforms = WorldTransforms3D::from_world(world);
    for (entity, instance) in world.query::<MeshInstance3D>() {
        let resolved = meshes.resolve(&instance.mesh);
        let Some(bounds) = meshes.bounds(resolved.handle) else {
            continue;
        };
        let model = transforms
            .get(entity)
            .unwrap_or_else(|| crate::hierarchy::world_matrix_3d(world, entity));
        if !model.is_finite() {
            if warned.insert(entity) {
                log::warn!("{entity:?}'s 3D transform is not finite; its mesh is not drawn");
            }
            continue;
        }
        out.push(Candidate {
            entity,
            mesh: resolved.handle,
            model,
            tint: if resolved.missing {
                MISSING_TINT
            } else {
                instance.color
            },
            bounds,
            instance: None,
            refused: None,
            mirrored: Instance3D::mirrored(&model),
            depth: 0.0,
        });
    }
}

/// Decides, for each of `candidates`, whether `view` sees it, and makes
/// its instance if so — across `jobs` when there are many.
pub(super) fn cull(candidates: &mut [Candidate], view: &View3D, jobs: Option<&JobPool>) {
    let frustum = view.frustum();
    let one = |candidate: &mut Candidate| {
        let world = candidate.bounds.transformed(&candidate.model);
        (candidate.instance, candidate.refused) = if frustum.intersects(&world) {
            match Instance3D::new(candidate.model, candidate.tint) {
                Ok(instance) => (Some(instance), None),
                Err(why) => (None, Some(why)),
            }
        } else {
            (None, None)
        };
        candidate.depth = view.depth(world.center());
    };
    match jobs {
        Some(jobs) if candidates.len() >= PARALLEL_FROM => {
            jobs.for_each_chunk_mut(candidates, PARALLEL_FROM / 4, |_, chunk| {
                chunk.iter_mut().for_each(&one);
            });
        }
        _ => candidates.iter_mut().for_each(&one),
    }
}

/// Warns once per entity in `warned` about each candidate whose model was
/// seen but would not make an instance for a reason worth saying: one
/// whose normal matrix overflows. One collapsed to a line or a point
/// covers no pixel and is left out quietly.
pub(super) fn report(candidates: &[Candidate], warned: &mut HashSet<Entity>) {
    for candidate in candidates {
        if candidate.refused == Some(InstanceRefused::NotFinite) && warned.insert(candidate.entity)
        {
            log::warn!(
                "{:?}'s 3D transform is too large to draw (its normals overflow); its mesh is \
                 not drawn",
                candidate.entity
            );
        }
    }
}
