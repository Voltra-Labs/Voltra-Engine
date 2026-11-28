//! Which mesh instances a view sees: gathered from the world, then each
//! one's world bounds tested against the frustum.

use std::collections::HashSet;

use voltra_assets::{meshes::MISSING_TINT, Handle, Material, MaterialDomain, Materials, Meshes};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::glam::Affine3A;
use voltra_render::render3d::{
    Aabb3, AlphaClass, Instance3D, InstanceRefused, Mesh3D, View3D, Winding,
};

use super::PARALLEL_FROM;
use crate::hierarchy::WorldTransforms3D;
use crate::mesh_instance3d::MeshInstance3D;

/// One mesh instance on its way to the list.
#[derive(Debug, Clone)]
pub(super) struct Candidate {
    pub entity: Entity,
    pub mesh: Handle<Mesh3D>,
    /// Its surface material, `None` for the default white standard.
    pub material: Option<Handle<Material>>,
    /// What the material's alpha does: opaque and masked front to back,
    /// blended back to front.
    pub alpha: AlphaClass,
    /// Whether the material shows both faces.
    pub double_sided: bool,
    pub model: Affine3A,
    pub tint: [f32; 4],
    pub bounds: Aabb3,
    /// Filled by [`cull`]: `None` when it is not drawn.
    pub instance: Option<Instance3D>,
    /// Filled by [`cull`]: why a seen candidate has no instance.
    pub refused: Option<InstanceRefused>,
    /// Which way its model winds the mesh's fronts on screen.
    pub winding: Winding,
    /// How far in front of the eye its world bounds' centre is.
    pub depth: f32,
}

/// Every mesh instance in `world`, placed: into `out`, emptied first.
/// A model that is not finite is left out and warned about once per
/// entity in `warned`. Each instance's material is found in `materials`
/// (loaded before the frame: see the frame's material resolve); one that
/// is not loaded, will not draw or is not a surface material draws as the
/// default, the last two warned about once per entity in
/// `warned_materials`.
pub(super) fn gather(
    world: &World,
    meshes: &mut Meshes,
    materials: Option<&Materials>,
    out: &mut Vec<Candidate>,
    warned: &mut HashSet<Entity>,
    warned_materials: &mut HashSet<Entity>,
) {
    out.clear();
    let transforms = WorldTransforms3D::from_world(world);
    for (entity, instance) in world.query::<MeshInstance3D>() {
        let (material, alpha, double_sided) =
            surface_of(entity, instance, materials, warned_materials);
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
            material,
            alpha,
            double_sided,
            model,
            tint: if resolved.missing {
                MISSING_TINT
            } else {
                instance.color
            },
            bounds,
            instance: None,
            refused: None,
            winding: Winding::of(&model),
            depth: 0.0,
        });
    }
}

/// `instance`'s material, if it draws through one, and what picks its
/// pipeline: its alpha class and sidedness. The default white standard
/// (opaque, one side) for none, for one not loaded, and — warned about once
/// for `entity` — for one that will not draw or is a sprite's.
fn surface_of(
    entity: Entity,
    instance: &MeshInstance3D,
    materials: Option<&Materials>,
    warned: &mut HashSet<Entity>,
) -> (Option<Handle<Material>>, AlphaClass, bool) {
    let default = (None, AlphaClass::Opaque, false);
    let (Some(path), Some(materials)) = (&instance.material, materials) else {
        return default;
    };
    let Some(handle) = materials.by_path_handle(path) else {
        return default;
    };
    let Some(material) = materials.get(handle) else {
        return default;
    };
    match material.surface() {
        Some(surface) => (
            Some(handle),
            AlphaClass::of(surface.standard.alpha),
            surface.standard.double_sided,
        ),
        None => {
            if warned.insert(entity) {
                let why = match (&material.error, material.domain) {
                    (Some(error), _) => format!("it cannot be drawn with: {error}"),
                    (None, MaterialDomain::Sprite) => "it is a sprite material".to_owned(),
                    (None, MaterialDomain::Surface) => "it has no surface".to_owned(),
                };
                log::warn!(
                    "{entity:?}'s mesh names the material {}, but {why}; drawn as the default \
                     white standard",
                    path.as_str()
                );
            }
            default
        }
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
