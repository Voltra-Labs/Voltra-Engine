//! What lights a 3D shot, gathered from the world, and what is said once
//! about what is left out of it.

use voltra_ecs::{Entity, World};
use voltra_scene::{lighting3d, DirectionalLight3D};

use super::World3D;

impl World3D {
    /// What lights `camera`'s view of `world`, saying once what is left
    /// out: a light that cannot light, lights and environments past the one
    /// used.
    fn lighting(&mut self, world: &World, camera: Entity) -> voltra_render::render3d::Lighting3D {
        let gathered = lighting3d::gather(world, camera);
        for (entity, light) in world.query::<DirectionalLight3D>() {
            if let Some(problem) = light.problem() {
                if self.light_warned.insert((entity, problem)) {
                    log::warn!("the DirectionalLight3D on {entity:?}: {problem}");
                }
            }
        }
        if let Some(used) = gathered.light {
            if gathered.unused_lights > 0 && self.light_warned.insert((used, "lights")) {
                log::warn!(
                    "{} more DirectionalLight3D in the world: only the one on {used:?} lights \
                     for now (one directional light a view)",
                    gathered.unused_lights
                );
            }
        }
        if let Some(used) = gathered.environment {
            if gathered.unused_environments > 0 && self.light_warned.insert((used, "environments"))
            {
                log::warn!(
                    "{} more world Environment3D: the one on {used:?} is used, the lowest entity \
                     index",
                    gathered.unused_environments
                );
            }
        }
        gathered.lighting
    }
}
