//! The hierarchy's Create menu: entities that are nothing but a place in
//! the tree, by dimension.
//!
//! Godot's "Add Node" offers `Node3D` and Unity's hierarchy "Create Empty";
//! this is that, filed under the dimension it is placed in, since a 3D
//! entity is made here and nowhere else until the 3D view (stage 145).

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{Name, SceneId, Transform3D};

use crate::editor::Editor;
use crate::spawn;

/// The menu, and the spawn it was asked for as one undo entry.
pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    ui.menu_button("Create", |ui| {
        ui.menu_button("3D", |ui| {
            let empty = ui
                .button("Empty")
                .on_hover_text("an entity with a Transform3D at the origin, to parent 3D under");
            if empty.clicked() {
                spawn::record(editor, frame, "Create 3D empty", |frame| {
                    empty_3d(frame.world)
                });
                ui.close();
            }
        });
    });
}

/// A named entity with an identity and a [`Transform3D`] at the origin.
fn empty_3d(world: &mut voltra_ecs::World) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new("Empty3D"));
    world.insert(entity, Transform3D::default());
    entity
}

#[cfg(test)]
mod tests {
    use voltra_ecs::World;
    use voltra_scene::hierarchy;

    use super::*;

    #[test]
    fn an_empty_is_3d_at_the_origin_and_takes_3d_children_only() {
        let mut world = World::new();
        let empty = empty_3d(&mut world);
        assert_eq!(
            world.get::<Transform3D>(empty),
            Some(&Transform3D::default())
        );
        assert!(world.get::<SceneId>(empty).is_some());

        let child = empty_3d(&mut world);
        hierarchy::set_parent(&mut world, child, empty).expect("3D under 3D");
        let flat = world.spawn();
        world.insert(flat, SceneId::new());
        world.insert(flat, voltra_scene::Transform2D::default());
        assert_eq!(
            hierarchy::set_parent(&mut world, flat, empty),
            Err(hierarchy::HierarchyError::MixedDimensions)
        );
    }
}
