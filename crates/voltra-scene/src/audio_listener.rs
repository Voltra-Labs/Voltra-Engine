//! The listener component: where the scene is heard from.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::hierarchy::world_matrix;

/// The ears. One per scene.
///
/// Unity's `AudioListener`, Godot's `AudioListener2D`, Unreal's listener on
/// the player controller. Every one of them is a component in the world rather
/// than a setting, for the same reason a camera is: it is usually hung off the
/// thing it follows, and following is what a parent already does.
///
/// **Unlike [`Camera`], this has no priority.** A scene holds several framings
/// and renders through one, which is a real authoring need; a scene does not
/// hold several sets of ears. Two active listeners is a mistake to make
/// deterministic, not a choice to give a knob to — so ties break on the
/// entity index and the field that exists is the one that turns a listener
/// off.
///
/// [`Camera`]: crate::camera::Camera
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioListener {
    /// Whether this listener is a candidate at all.
    pub active: bool,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self { active: true }
    }
}

/// The listener a scene is heard through: the first active one.
///
/// `None` when the scene holds no active listener. That is not an error and
/// not silence: the engine falls back to hearing every source unpositioned —
/// full volume, centred — and says so once. A scene where every sound
/// disappeared because nobody added a component would be a worse answer than
/// one that plays them all flat, and it is the same call
/// [`camera::active`](crate::camera::active) makes when a scene has no camera.
///
/// Ties break on [`Entity::index`], ascending, so the answer does not depend
/// on query order.
pub fn active(world: &World) -> Option<Entity> {
    world
        .query::<AudioListener>()
        .filter(|(_, listener)| listener.active)
        .min_by_key(|(entity, _)| entity.index())
        .map(|(entity, _)| entity)
}

/// Where the scene is heard from: [`active`], then that entity's world
/// position.
///
/// Composed through any parents, so a listener can be hung off the character
/// it follows without a follow system existing — the same way
/// [`camera::view`](crate::camera::view) reads a camera's position. A listener
/// with no [`Transform`](crate::transform::Transform) is at the origin.
///
/// `None` means no active listener, which the caller answers for: the engine
/// plays every source unpositioned and says so once.
pub fn position(world: &World) -> Option<Vec2> {
    let entity = active(world)?;
    Some(world_matrix(world, entity).transform_point2(Vec2::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Transform;

    fn spawn(world: &mut World, listener: AudioListener) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(Vec2::ZERO));
        world.insert(entity, listener);
        entity
    }

    #[test]
    fn a_scene_without_a_listener_has_no_active_one() {
        let mut world = World::new();
        world.spawn();
        assert_eq!(active(&world), None);
    }

    #[test]
    fn the_only_listener_is_the_active_one() {
        let mut world = World::new();
        let ears = spawn(&mut world, AudioListener::default());
        assert_eq!(active(&world), Some(ears));
    }

    #[test]
    fn an_inactive_listener_is_never_chosen() {
        let mut world = World::new();
        spawn(&mut world, AudioListener { active: false });
        let ears = spawn(&mut world, AudioListener::default());

        assert_eq!(active(&world), Some(ears));
    }

    #[test]
    fn two_listeners_resolve_to_the_lower_entity_index() {
        // A mistake, made deterministic: the same scene must be heard the same
        // way twice, whatever order the query happens to walk.
        let mut world = World::new();
        let first = spawn(&mut world, AudioListener::default());
        let second = spawn(&mut world, AudioListener::default());

        assert!(first.index() < second.index());
        assert_eq!(active(&world), Some(first));
    }

    #[test]
    fn a_scene_without_a_listener_is_heard_from_nowhere() {
        let world = World::new();
        assert_eq!(position(&world), None);
    }

    #[test]
    fn the_listener_is_heard_from_where_its_transform_says() {
        let mut world = World::new();
        let ears = spawn(&mut world, AudioListener::default());
        world.insert(ears, Transform::from_translation(Vec2::new(3.0, -4.0)));

        assert_eq!(position(&world), Some(Vec2::new(3.0, -4.0)));
    }

    #[test]
    fn a_parented_listener_is_where_its_parent_put_it() {
        // The whole reason it is a component: hung off the character, it
        // follows without a follow system.
        let mut world = World::new();
        let rig = world.spawn();
        world.insert(rig, crate::scene_id::SceneId::new());
        world.insert(rig, Transform::from_translation(Vec2::new(10.0, 0.0)));

        let ears = spawn(&mut world, AudioListener::default());
        world.insert(ears, crate::scene_id::SceneId::new());
        world.insert(ears, Transform::from_translation(Vec2::new(1.0, 2.0)));
        crate::hierarchy::set_parent(&mut world, ears, rig).expect("a plain reparent");

        let heard = position(&world).expect("it is active");
        assert!((heard - Vec2::new(11.0, 2.0)).length() < 1e-5);
    }

    #[test]
    fn a_listener_needs_no_transform_to_be_chosen() {
        // It is then at the origin, which is what the identity matrix already
        // means everywhere else in the scene.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, AudioListener::default());

        assert_eq!(active(&world), Some(entity));
    }
}
