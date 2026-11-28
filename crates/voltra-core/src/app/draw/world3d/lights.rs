//! What lights a 3D shot, gathered from the world, and what is said once
//! about what is left out of it.

use voltra_ecs::{Entity, World};
use voltra_render::render3d::{View3D, LIGHTS_PER_INSTANCE, MAX_DIRECTIONAL, MAX_PUNCTUAL};
use voltra_scene::draw3d::Crowded3D;
use voltra_scene::lighting3d::{self, Gathered3D};

use super::World3D;

impl World3D {
    /// What lights `camera`'s view of `world` through `view`, saying once
    /// what is left out: a light or an environment that cannot draw,
    /// lights past the caps, environments past the one used.
    pub(super) fn lighting(&mut self, world: &World, camera: Entity, view: &View3D) -> Gathered3D {
        let gathered = lighting3d::gather(world, camera, view);
        for &(entity, problem) in &gathered.broken {
            if self.light_warned.insert((entity, problem)) {
                log::warn!("the 3D light on {entity:?}: {problem}");
            }
        }
        if gathered.dropped_directional > 0 && self.light_warned.insert((camera, "directional")) {
            log::warn!(
                "{} DirectionalLight3D more than the {MAX_DIRECTIONAL} a view is lit by: the \
                 dimmest are left out of the view of the camera on {camera:?}",
                gathered.dropped_directional
            );
        }
        if gathered.dropped_punctual > 0 && self.light_warned.insert((camera, "punctual")) {
            log::warn!(
                "{} point and spot lights in view more than the {MAX_PUNCTUAL} a view holds: \
                 those mattering least to the camera on {camera:?} are left out",
                gathered.dropped_punctual
            );
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
            let problem = world
                .get::<voltra_scene::Environment3D>(used)
                .and_then(|env| env.problem());
            if let Some(problem) = problem {
                if self.light_warned.insert((used, problem)) {
                    log::warn!("the Environment3D on {used:?}: {problem}");
                }
            }
        }
        gathered
    }

    /// Says once a session, with the count, that some meshes are reached
    /// by more point and spot lights than one is lit by.
    pub(super) fn warn_crowded(&mut self, crowded: Crowded3D) {
        if crowded.instances > 0 && !self.crowded_warned {
            self.crowded_warned = true;
            log::warn!(
                "{} meshes are reached by more than {LIGHTS_PER_INSTANCE} point and spot lights \
                 (up to {}): each is lit by the {LIGHTS_PER_INSTANCE} that matter most to it, the \
                 rest dropped",
                crowded.instances,
                crowded.most
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Affine3A;
    use voltra_scene::{DirectionalLight3D, SceneId};

    use super::*;

    #[test]
    fn what_is_dropped_over_a_cap_is_said_once_with_its_count() {
        let mut world = World::new();
        let camera = world.spawn();
        world.insert(camera, SceneId::new());
        for _ in 0..6 {
            let sun = world.spawn();
            world.insert(sun, SceneId::new());
            world.insert(sun, DirectionalLight3D::default());
        }
        let view = View3D::perspective(Affine3A::IDENTITY, 1.0, 1.0, 0.1, None);
        let mut frame = World3D::default();
        let first = frame.lighting(&world, camera, &view);
        assert_eq!(first.dropped_directional, 2);
        assert!(frame.light_warned.contains(&(camera, "directional")));
        let said = frame.light_warned.len();
        frame.lighting(&world, camera, &view);
        assert_eq!(frame.light_warned.len(), said, "said once");
        let crowded = Crowded3D {
            instances: 3,
            most: 11,
        };
        frame.warn_crowded(crowded);
        assert!(frame.crowded_warned);
        frame.warn_crowded(crowded);
        frame.warn_crowded(Crowded3D::default());
        assert!(frame.crowded_warned);
    }
}
