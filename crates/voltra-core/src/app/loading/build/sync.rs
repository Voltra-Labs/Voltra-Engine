//! A load's prefab instances brought in step with their prefabs, as a
//! scene change brings the whole world's.

use voltra_ecs::Entity;
use voltra_scene::{hierarchy, prefab, PrefabInstance};

use crate::app::spawning::prefab_store;
use crate::app::App;

impl App {
    /// The prefab instances among `entities` not inside another of them:
    /// what syncing walks, as a scene change syncs the whole world.
    pub(super) fn instance_roots(&self, entities: &[Entity]) -> Vec<Entity> {
        entities
            .iter()
            .copied()
            .filter(|&entity| self.world.get::<PrefabInstance>(entity).is_some())
            .filter(|&entity| {
                hierarchy::ancestors(&self.world, entity)
                    .iter()
                    .all(|ancestor| self.world.get::<PrefabInstance>(*ancestor).is_none())
            })
            .collect()
    }

    /// Brings the instance at `root` in step with its prefab, resolving
    /// whatever that changed.
    pub(super) fn sync_instance(&mut self, root: Entity) {
        let Some(source) = self
            .world
            .get::<PrefabInstance>(root)
            .map(|link| link.source.clone())
        else {
            return;
        };
        let prefabs = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry);
        match prefab::sync_in(&mut self.world, &self.registry, prefabs, root) {
            Ok(report) => {
                self.spawned.extend(report.touched);
                self.resolve_spawned();
            }
            Err(error) => log::error!(
                "could not bring an instance of {} in step with it: {error}",
                source.as_str()
            ),
        }
    }
}
