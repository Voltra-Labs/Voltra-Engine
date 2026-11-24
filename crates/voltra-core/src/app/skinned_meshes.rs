//! Skinned meshes on the GPU, one per entity with a `SkinnedSprite`: kept
//! in step with the world, uploaded again when the mesh or its weights
//! change, and posed — one matrix per bone — before every drawn frame.

use voltra_ecs::hash::FastMap;
use voltra_ecs::Entity;
use voltra_render::{SkinPipeline, SkinnedMesh};
use voltra_scene::hierarchy::WorldTransforms;
use voltra_scene::skinned::{pose, SkinMesh};
use voltra_scene::SkinnedSprite;

use super::App;

/// The renderer-side half of every skinned sprite.
#[derive(Default)]
pub(super) struct SkinnedMeshes {
    /// Each entity's GPU mesh, and the mesh and bone count it was made
    /// from — what a change is noticed against.
    meshes: FastMap<Entity, (SkinnedMesh, SkinMesh, usize)>,
    /// The renderer's pipeline, shared, so a draw list can name it while
    /// the renderer is borrowed to draw.
    pipeline: Option<SkinPipeline>,
}

impl SkinnedMeshes {
    /// `entity`'s mesh and the pipeline it draws through, if it has one.
    pub(super) fn get(&self, entity: Entity) -> Option<(&SkinnedMesh, &SkinPipeline)> {
        let (mesh, _, _) = self.meshes.get(&entity)?;
        Some((mesh, self.pipeline.as_ref()?))
    }
}

impl App {
    /// Makes, remakes and drops the GPU meshes to match the world, and
    /// poses each from its skeleton as the batch will see it — before the
    /// frame that draws them is recorded.
    pub(super) fn update_skinned_meshes(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let skins = &mut self.skinned_meshes;
        let pipeline = skins
            .pipeline
            .get_or_insert_with(|| renderer.skin_pipeline().clone());
        let device = renderer.context().device();
        let queue = renderer.context().queue();
        let transforms = WorldTransforms::seen_from(&self.world, renderer.camera.position);
        let mut alive = Vec::new();
        for (entity, skinned) in self.world.query::<SkinnedSprite>() {
            let stale = skins.meshes.get(&entity).is_none_or(|(_, made, bones)| {
                *made != skinned.mesh || *bones != skinned.bones.len()
            });
            if stale {
                let (vertices, indices) = skinned.gpu_mesh();
                match SkinnedMesh::new(device, pipeline, &vertices, &indices) {
                    Some(mesh) => {
                        skins
                            .meshes
                            .insert(entity, (mesh, skinned.mesh.clone(), skinned.bones.len()));
                    }
                    None => {
                        skins.meshes.remove(&entity);
                        continue;
                    }
                }
            }
            if let Some((mesh, _, _)) = skins.meshes.get(&entity) {
                let posed =
                    pose::skin_pose(&self.world, entity, skinned, &|e| transforms.matrix(e));
                mesh.set_pose(queue, &posed.bones, posed.rigid, skinned.color);
                alive.push(entity);
            }
        }
        // A despawned entity, or one whose component went.
        skins.meshes.retain(|entity, _| alive.contains(entity));
    }
}
