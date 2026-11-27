//! Particle emitters, advanced with the frame.

use voltra_scene::{hierarchy, particles};

use super::App;

impl App {
    /// Ages, moves and emits every emitter's particles, and despawns the
    /// one-shot emitters that have finished.
    ///
    /// Gated on the simulation switch, as animation is: particles are part
    /// of a world that is running, and an editor that is authoring shows the
    /// emitter, not a fountain playing under the cursor.
    pub(super) fn advance_particles(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        let gpu = self.gpu_particles_available();
        for entity in particles::advance(&mut self.world, delta, gpu) {
            if self.world.is_alive(entity) {
                hierarchy::despawn_recursive(&mut self.world, entity);
            }
        }
        // Trails are the same kind of thing — a picture of motion, made as
        // the world runs — and age with it.
        voltra_scene::trail2d::advance(&mut self.world, delta);
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;
    use voltra_scene::{ParticleEmitter, Transform2D};

    use crate::app::App;

    #[test]
    fn a_finished_one_shot_emitter_is_despawned_by_the_loop() {
        let mut app = App::default().with_simulation();
        let entity = app.world.spawn();
        app.world
            .insert(entity, Transform2D::from_translation(Vec2::ZERO));
        let mut emitter = ParticleEmitter {
            emitting: false,
            one_shot: true,
            lifetime: (0.1, 0.1),
            ..ParticleEmitter::default()
        };
        emitter.burst(3);
        app.world.insert(entity, emitter);

        for _ in 0..30 {
            app.advance(1.0 / 60.0);
        }

        assert!(!app.world.is_alive(entity));
    }

    #[test]
    fn particles_hold_still_while_the_world_is_not_running() {
        let mut app = App::default();
        let entity = app.world.spawn();
        app.world.insert(entity, Transform2D::default());
        app.world.insert(entity, ParticleEmitter::default());
        app.advance(0.5);
        let alive = app
            .world
            .get::<ParticleEmitter>(entity)
            .map(ParticleEmitter::alive);
        assert_eq!(alive, Some(0));
    }
}
