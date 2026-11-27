//! The parts of a screen spawned from a prefab, found by their names: what
//! the options and lobby screens fill in, so a game restyles either by
//! editing its file and keeping the names.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_scene::Name;

/// Every entity of a spawned screen by name; the first of a name wins.
pub(crate) struct Parts {
    named: HashMap<String, Entity>,
    /// What the screen is called in a warning: "the options prefab".
    screen: &'static str,
}

impl Parts {
    /// The named entities among `entities`, of the screen called `screen`.
    pub(crate) fn of(world: &World, entities: &[Entity], screen: &'static str) -> Self {
        let mut named = HashMap::new();
        for &entity in entities {
            if let Some(name) = world.get::<Name>(entity) {
                named.entry(name.as_str().to_owned()).or_insert(entity);
            }
        }
        Self { named, screen }
    }

    /// The part called `name`, or `None` with a warning: a prefab restyled
    /// without one loses that row, not the screen.
    pub(crate) fn get(&self, name: &str) -> Option<Entity> {
        let found = self.named.get(name).copied();
        if found.is_none() {
            log::warn!(
                "the {} prefab has no part named {name:?}; that row is left out",
                self.screen
            );
        }
        found
    }
}
