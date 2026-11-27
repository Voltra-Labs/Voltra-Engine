//! Elements taken off the screen without being despawned.

use voltra_ecs::{Entity, World};

use crate::hierarchy;

/// Takes this element and everything under it off the screen: not laid
/// out, so not drawn, picked or focused, and taking no room in a
/// [`UiStack`](super::UiStack).
///
/// Godot's `visible = false` on a `Control`, Unity's `SetActive(false)` on
/// a UI object: the entities stay, with every value the player set, and
/// come back as they were. [`UiTabs`](super::UiTabs) shows one page this
/// way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiHidden;

impl UiHidden {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiHidden";
}

/// Whether `entity` or any element above it is [`UiHidden`].
pub fn is_hidden(world: &World, entity: Entity) -> bool {
    world.get::<UiHidden>(entity).is_some()
        || hierarchy::ancestors(world, entity)
            .into_iter()
            .any(|ancestor| world.get::<UiHidden>(ancestor).is_some())
}

/// Hides or shows `entity`; whether that changed anything.
pub fn set_hidden(world: &mut World, entity: Entity, hidden: bool) -> bool {
    let now = world.get::<UiHidden>(entity).is_some();
    if now == hidden {
        return false;
    }
    if hidden {
        world.insert(entity, UiHidden);
    } else {
        world.remove::<UiHidden>(entity);
    }
    true
}
