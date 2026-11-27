//! Spawning a skeleton, and a bone hung at another's tip.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::set_parent;
use voltra_scene::{Bone2D, Name, SceneId, Skeleton2D, Transform2D};

/// A skeleton root at `at` with one bone pointing up from it: everything
/// made, and the bone to select.
pub(crate) fn skeleton(
    frame: &mut UiFrame<'_>,
    name: &str,
    at: Vec2,
) -> (Vec<Entity>, Option<Entity>) {
    let root = frame.world.spawn();
    frame.world.insert(root, SceneId::new());
    frame.world.insert(root, Name::new(name));
    frame.world.insert(root, Transform2D::from_translation(at));
    frame.world.insert(root, Skeleton2D::default());
    let pose = Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_2);
    let bone = bone(frame, "bone", root, pose);
    (vec![root, bone], Some(bone))
}

/// An empty skinned sprite at `at`: give it a texture, hang it under a
/// skeleton, then *Generate mesh* in the inspector.
pub(crate) fn skinned_sprite(frame: &mut UiFrame<'_>, name: &str, at: Vec2) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame
        .world
        .insert(entity, Transform2D::from_translation(at));
    frame
        .world
        .insert(entity, voltra_scene::SkinnedSprite::default());
    entity
}

/// A bone hung at `parent`'s tip, carrying on in its direction.
pub(crate) fn child_bone(frame: &mut UiFrame<'_>, parent: Entity) -> Entity {
    let length = frame.world.get::<Bone2D>(parent).map_or(0.5, |b| b.length);
    let pose = Transform2D::from_translation(Vec2::new(length, 0.0));
    bone(frame, "bone", parent, pose)
}

fn bone(frame: &mut UiFrame<'_>, name: &str, parent: Entity, pose: Transform2D) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame.world.insert(entity, Name::new(name));
    frame.world.insert(entity, pose);
    frame.world.insert(entity, Bone2D::new(0.5, pose));
    if let Err(e) = set_parent(frame.world, entity, parent) {
        log::warn!("a new bone could not be hung on its parent: {e:?}");
    }
    entity
}
