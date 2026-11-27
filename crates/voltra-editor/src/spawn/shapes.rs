//! Spawning lines, filled polygons and trails.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Line2D, Name, Polygon2D, SceneId, Trail2D, Transform2D};

/// Drops the default line — two segments with a corner — at `at`.
pub(crate) fn line(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    spawn(frame, name, at, Line2D::default())
}

/// Drops the default polygon, a unit square, at `at`.
pub(crate) fn polygon(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    spawn(frame, name, at, Polygon2D::default())
}

/// Drops a trail at `at`: it draws once the entity moves while playing.
pub(crate) fn trail(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    spawn(frame, name, at, Trail2D::default())
}

fn spawn<T: 'static>(frame: &mut UiFrame<'_>, name: &str, at: Vec2, component: T) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame
        .world
        .insert(entity, Transform2D::from_translation(at));
    frame.world.insert(entity, component);
    entity
}
