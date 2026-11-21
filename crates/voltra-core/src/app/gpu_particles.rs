//! GPU particle rings, one per emitter simulated on the GPU, kept in step
//! with the world and dispatched once per drawn frame.

use voltra_ecs::hash::FastMap;
use voltra_ecs::Entity;
use voltra_render::{GpuParticles, ParticleLook, SimStep};
use voltra_scene::hierarchy::WorldTransforms;
use voltra_scene::ParticleEmitter;

use super::App;

/// The renderer-side half of every GPU emitter.
#[derive(Default)]
pub(super) struct GpuParticleRings {
    rings: FastMap<Entity, GpuParticles>,
}

impl GpuParticleRings {
    pub(super) fn get(&self, entity: Entity) -> Option<&GpuParticles> {
        self.rings.get(&entity)
    }
}

impl App {
    /// Whether emitters set to the GPU can run there.
    pub(super) fn gpu_particles_available(&self) -> bool {
        self.renderer
            .as_ref()
            .is_some_and(|renderer| renderer.particle_pipelines().is_some())
    }

    /// Makes, resizes and drops rings to match the emitters, then runs the
    /// step each owes and sets how each is drawn — before the frame that
    /// draws them is recorded.
    ///
    /// An emitter with no step owed (the world paused) still has its look
    /// set: an emitter dragged in the editor draws where it now is.
    pub(super) fn step_gpu_particles(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let Some(pipelines) = renderer.particle_pipelines() else {
            return;
        };
        let transforms = WorldTransforms::seen_from(&self.world, renderer.camera.position);
        let mut owed: Vec<(Entity, SimStep, ParticleLook)> = Vec::new();
        for (entity, emitter) in self.world.query_mut::<ParticleEmitter>() {
            let Some(capacity) = emitter.gpu_capacity() else {
                continue;
            };
            let step = emitter.take_gpu_step().unwrap_or_default();
            owed.push((entity, step, emitter.gpu_look(transforms.matrix(entity))));
            let device = renderer.context().device();
            let ring = self
                .gpu_rings
                .rings
                .entry(entity)
                .or_insert_with(|| GpuParticles::new(device, pipelines, capacity));
            if ring.capacity() != capacity {
                *ring = GpuParticles::new(device, pipelines, capacity);
            }
        }
        // A despawned emitter, or one switched back to the CPU.
        self.gpu_rings
            .rings
            .retain(|entity, _| owed.iter().any(|(owner, _, _)| owner == entity));

        let rings = &self.gpu_rings;
        renderer.step_particles(
            owed.iter().filter_map(|(entity, step, look)| {
                rings.get(*entity).map(|ring| (ring, step, look))
            }),
        );
    }
}
