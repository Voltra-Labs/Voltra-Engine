//! Spawning lights.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Name, SceneId, Transform};

/// Drops the default point light at `at`; it shows once a camera is lit.
pub(crate) fn light(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, Transform::from_translation(at));
    frame
        .world
        .insert(entity, voltra_scene::PointLight2D::default());
    entity
}

/// Drops a global light: a faint lift of the ambient on every layer, to be
/// narrowed to the layers it is for. Its place does not matter; it sits at
/// the origin so the hierarchy has somewhere to show it.
pub(crate) fn global_light(frame: &mut UiFrame<'_>, name: &str) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, Transform::default());
    frame
        .world
        .insert(entity, voltra_scene::GlobalLight2D::default());
    entity
}
