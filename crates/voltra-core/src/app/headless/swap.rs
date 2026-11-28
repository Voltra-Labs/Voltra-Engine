//! A headless run's game swapped for a rebuilt one, the world kept.

use voltra_scene::{ComponentRegistry, SceneError};

use super::Headless;

impl Headless {
    /// Replaces the running game with a fresh one made by `library`, keeping
    /// the world: written out through the old game's registry, read back
    /// through the new one's — what the editor does when a game's code is
    /// rebuilt mid-play. The old game is dropped first; its library may be
    /// closed once this returns. A scene the new types cannot read is left
    /// as it was, with the old game running, and the error answered.
    pub fn swap_game(
        &mut self,
        library: &crate::game::library::GameLibrary,
    ) -> Result<(), SceneError> {
        let mut registry = ComponentRegistry::with_defaults();
        library.register(&mut registry);
        let old = self.app.game.take();
        if let Err(error) =
            crate::game::swap::rebuild(&mut self.app.world, &self.app.registry, &registry)
        {
            self.app.game = old;
            return Err(error);
        }
        drop(old);
        self.app.registry = registry;
        self.app.game = Some(crate::game::Running::new(library.new_game()));
        // Contacts, sleep and pending events all name entities the rebuild
        // replaced; every handle is resolved again for the new ones.
        self.app.physics_world.reset();
        self.app.pending_physics.clear();
        self.app.spawned.extend(self.app.world.entities());
        self.app.resolve_spawned();
        Ok(())
    }
}
