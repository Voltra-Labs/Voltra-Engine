//! The selected emitter plays while the scene is being authored.
//!
//! Unity plays a selected particle system in the Scene view without
//! entering Play; Godot's emitters run in the editor whenever they are on.
//! Here only the primary selection runs — a level full of fountains stays
//! still while one is tuned — and only its live particles change, which a
//! scene never saves, so the preview is never an edit. Deselected, it is
//! cleared, rather than left frozen mid-air.

use voltra_ecs::{Entity, World};
use voltra_scene::particles;
use voltra_scene::ParticleEmitter;

/// Which emitter is being previewed.
#[derive(Debug, Default)]
pub(crate) struct ParticlePreview {
    playing: Option<Entity>,
}

impl ParticlePreview {
    /// Steps `selected`'s emitter by `dt`, if it has one; clears the one
    /// previewed before when the selection moved off it. `gpu` as
    /// [`particles::advance`] takes it. A one-shot that finishes is kept:
    /// despawning what is being authored is not a preview's to do.
    pub(crate) fn update(
        &mut self,
        world: &mut World,
        selected: Option<Entity>,
        dt: f32,
        gpu: bool,
    ) {
        let selected = selected.filter(|&e| world.get::<ParticleEmitter>(e).is_some());
        if self.playing != selected {
            self.stop(world);
            self.playing = selected;
        }
        if let Some(entity) = selected {
            particles::advance_where(world, dt, gpu, |e| e == entity);
        }
    }

    /// Ends the preview — the selection gone, or play starting from the
    /// authored scene — clearing what it emitted.
    pub(crate) fn stop(&mut self, world: &mut World) {
        if let Some(emitter) = self
            .playing
            .take()
            .and_then(|entity| world.get_mut::<ParticleEmitter>(entity))
        {
            emitter.restart();
        }
    }
}

#[cfg(test)]
mod tests {
    use voltra_scene::Transform2D;

    use super::*;

    fn emitter(world: &mut World) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform2D::default());
        world.insert(
            entity,
            ParticleEmitter {
                rate: 60.0,
                lifetime: (10.0, 10.0),
                ..ParticleEmitter::default()
            },
        );
        entity
    }

    fn alive(world: &World, entity: Entity) -> usize {
        world
            .get::<ParticleEmitter>(entity)
            .map_or(0, ParticleEmitter::alive)
    }

    #[test]
    fn only_the_selected_emitter_plays_and_a_deselected_one_is_cleared() {
        let mut world = World::new();
        let (a, b) = (emitter(&mut world), emitter(&mut world));
        let mut preview = ParticlePreview::default();
        for _ in 0..30 {
            preview.update(&mut world, Some(a), 1.0 / 60.0, false);
        }
        assert!(alive(&world, a) > 20, "the selection plays");
        assert_eq!(alive(&world, b), 0, "the rest hold still");
        preview.update(&mut world, Some(b), 1.0 / 60.0, false);
        assert_eq!(alive(&world, a), 0, "cleared once deselected");
        preview.update(&mut world, None, 1.0 / 60.0, false);
        assert_eq!(alive(&world, b), 0);
    }

    #[test]
    fn a_finished_one_shot_is_kept() {
        let mut world = World::new();
        let entity = emitter(&mut world);
        if let Some(emitter) = world.get_mut::<ParticleEmitter>(entity) {
            emitter.one_shot = true;
            emitter.emitting = false;
        }
        let mut preview = ParticlePreview::default();
        for _ in 0..10 {
            preview.update(&mut world, Some(entity), 1.0 / 60.0, false);
        }
        assert!(world.is_alive(entity));
    }
}
