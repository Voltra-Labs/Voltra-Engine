//! Editing a shadow caster's polygon from the viewport.
//!
//! **A mode of the viewport while the caster is selected**, the tilemap
//! paint mode's shape and Unity's "Edit Shape" button on `ShadowCaster2D`:
//! the inspector turns it on, and while it is on a drag on a point moves
//! it, a press on an edge's midpoint adds a point there (and a drag goes on
//! to move it), and a right click on a point removes it. `Esc`, the button
//! again, a tool, or selecting anything else turns it off. The gizmo and
//! click-to-select wait meanwhile. The handles are drawn in `draw`.
//!
//! **Points stay in the entity's space.** The pointer is taken into it
//! through the inverse of the entity's composed matrix, so a scaled, turned
//! or parented caster is edited where it is drawn and stored as authored.
//!
//! **One drag, one undo entry**: the selection is what the history watches,
//! and an edit touches only the selected caster, so a drag claims the open
//! entry every frame it is live, as a gizmo drag and a paint stroke do.

mod draw;
mod handle;

use voltra_core::egui::{Key, PointerButton, Response, Ui};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::world_matrix;
use voltra_scene::{CasterShape, ShadowCaster2D};

use handle::Handle;

/// What the history entry of an edit is called.
const LABEL: &str = "Edit shadow shape";

/// Which caster is being edited, and the point being dragged.
#[derive(Debug, Default)]
pub struct ShapeEdit {
    editing: Option<Entity>,
    /// The index of the point under a drag. Carried across frames; checked
    /// against the polygon each frame, which an undo may have replaced.
    drag: Option<usize>,
    /// The handle under the pointer this frame, for the highlight.
    hovered: Option<Handle>,
}

/// `entity`'s polygon and whether it is closed, if its caster has one.
fn polygon(world: &World, entity: Entity) -> Option<(&[Vec2], bool)> {
    match &world.get::<ShadowCaster2D>(entity)?.shape {
        CasterShape::Polygon { points, closed } => Some((points.as_slice(), *closed)),
        CasterShape::Collider => None,
    }
}

fn points_mut(world: &mut World, entity: Entity) -> Option<&mut Vec<Vec2>> {
    match &mut world.get_mut::<ShadowCaster2D>(entity)?.shape {
        CasterShape::Polygon { points, .. } => Some(points),
        CasterShape::Collider => None,
    }
}

fn viewport_of(scene: &Response) -> Vec2 {
    Vec2::new(scene.rect.width(), scene.rect.height())
}

impl ShapeEdit {
    pub fn is_editing(&self, entity: Entity) -> bool {
        self.editing == Some(entity)
    }

    /// Edits `entity`'s polygon, or stops editing for `None`.
    pub fn set_editing(&mut self, entity: Option<Entity>) {
        self.editing = entity;
        self.drag = None;
        self.hovered = None;
    }

    /// Ends a drag without ending the editing: an undo or redo has just
    /// replaced what it was moving.
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    /// The caster being edited, if it is still the selection and still has
    /// a polygon; editing stops otherwise.
    pub fn target(&mut self, world: &World, selected: Option<Entity>) -> Option<Entity> {
        let entity = self.editing?;
        let still =
            selected == Some(entity) && world.is_alive(entity) && polygon(world, entity).is_some();
        if !still {
            self.set_editing(None);
        }
        still.then_some(entity)
    }

    /// Runs the pointer against `entity`'s polygon for one frame. Returns
    /// what to call the history entry while an edit is live, including the
    /// frame it ends.
    pub fn update(
        &mut self,
        ui: &Ui,
        scene: &Response,
        frame: &mut UiFrame<'_>,
        entity: Entity,
    ) -> Option<&'static str> {
        let viewport = viewport_of(scene);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            // A minimised window: every conversion below divides by it.
            self.drag = None;
            return None;
        }
        // Wherever the pointer is — it is often still over the inspector's
        // button — unless a text field has the keyboard.
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.set_editing(None);
            return None;
        }
        let matrix = world_matrix(frame.world, entity);
        let inverse = matrix.inverse();
        if matrix.determinant() == 0.0 || !inverse.is_finite() {
            // Scaled to nothing: no pointer position maps back into its
            // space, and nothing drawn is there to grab. A small caster is
            // fine; only one with no inverse is refused.
            self.drag = None;
            return None;
        }
        let cursor = scene
            .interact_pointer_pos()
            .or_else(|| scene.hover_pos())
            .map(|p| Vec2::new(p.x - scene.rect.min.x, p.y - scene.rect.min.y));
        let to_local =
            |pixel: Vec2| inverse.transform_point2(frame.camera.viewport_to_world(pixel, viewport));

        if let Some(index) = self.drag {
            // The primary button, not "any button down on the viewport": a
            // right button held for the fly chord must not keep a drag alive.
            if !ui.input(|i| i.pointer.primary_down()) {
                // Released, wherever the pointer is. This frame still claims
                // the entry so the edit's last position is in it.
                self.drag = None;
                return Some(LABEL);
            }
            let local = cursor.map(to_local);
            match points_mut(frame.world, entity) {
                Some(points) if index < points.len() => {
                    if let Some(local) = local {
                        points[index] = local;
                    }
                    return Some(LABEL);
                }
                // Undone mid-drag into fewer points, or no polygon at all.
                _ => {
                    self.drag = None;
                    return None;
                }
            }
        }

        let (points, closed) = polygon(frame.world, entity)?;
        let on_screen: Vec<Vec2> = points
            .iter()
            .map(|p| {
                frame
                    .camera
                    .world_to_viewport(matrix.transform_point2(*p), viewport)
            })
            .collect();
        self.hovered = scene
            .hover_pos()
            .and(cursor)
            .and_then(|at| handle::handle_at(at, &on_screen, closed));

        let pressed = scene.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_pressed(PointerButton::Primary));
        if pressed {
            let points = points_mut(frame.world, entity)?;
            self.drag = match self.hovered? {
                Handle::Point(index) => Some(index),
                Handle::Edge(index) => Some(handle::insert(points, index)),
            };
            return Some(LABEL);
        }
        if scene.secondary_clicked() {
            if let Some(Handle::Point(index)) = self.hovered {
                let points = points_mut(frame.world, entity)?;
                if handle::remove(points, index, closed) {
                    self.hovered = None;
                    return Some(LABEL);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caster(world: &mut World, shape: CasterShape) -> Entity {
        let entity = world.spawn();
        world.insert(
            entity,
            ShadowCaster2D {
                shape,
                self_shadow: false,
            },
        );
        entity
    }

    fn line() -> CasterShape {
        CasterShape::Polygon {
            points: vec![Vec2::ZERO, Vec2::X],
            closed: false,
        }
    }

    #[test]
    fn the_edited_caster_is_the_target_while_it_is_selected() {
        let mut world = World::new();
        let wall = caster(&mut world, line());
        let mut edit = ShapeEdit::default();
        assert_eq!(edit.target(&world, Some(wall)), None, "not editing yet");
        edit.set_editing(Some(wall));
        assert_eq!(edit.target(&world, Some(wall)), Some(wall));
        assert!(edit.is_editing(wall));
    }

    #[test]
    fn editing_stops_when_the_selection_moves_on() {
        let mut world = World::new();
        let (wall, other) = (caster(&mut world, line()), caster(&mut world, line()));
        let mut edit = ShapeEdit::default();
        edit.set_editing(Some(wall));
        assert_eq!(edit.target(&world, Some(other)), None);
        assert!(!edit.is_editing(wall), "and stays stopped");
        assert_eq!(edit.target(&world, Some(wall)), None);
    }

    #[test]
    fn editing_stops_when_the_polygon_or_the_entity_goes() {
        let mut world = World::new();
        let wall = caster(&mut world, line());
        let mut edit = ShapeEdit::default();
        edit.set_editing(Some(wall));
        if let Some(c) = world.get_mut::<ShadowCaster2D>(wall) {
            c.shape = CasterShape::Collider;
        }
        assert_eq!(
            edit.target(&world, Some(wall)),
            None,
            "back to its collider"
        );

        let gone = caster(&mut world, line());
        edit.set_editing(Some(gone));
        world.despawn(gone);
        assert_eq!(edit.target(&world, Some(gone)), None, "despawned");
    }
}
