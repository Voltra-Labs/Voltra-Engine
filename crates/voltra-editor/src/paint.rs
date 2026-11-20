//! Painting a tilemap layer's cells from the viewport.
//!
//! **A mode of the viewport while a layer is selected**, Godot's shape: with a
//! paint tool armed and a [`Tilemap`] selected, a drag paints instead of
//! grabbing the gizmo, and a click paints instead of selecting. Unity keeps a
//! separate Tile Palette window whose tools own the scene view the same way.
//! `W`/`E`/`R` — or selecting something that is not a layer — disarms it.
//!
//! **One stroke, one undo entry.** The selection is what the history watches,
//! and a stroke edits only the selected layer, so the stroke claims the open
//! entry every frame it is live, exactly as a gizmo drag does.

mod cell;
mod line;
mod stroke;
mod tool;

use voltra_core::egui::{Response, Ui};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::IVec2;
use voltra_render::LineBatch;
use voltra_scene::{Tile, Tilemap};

pub use tool::PaintTool;

/// Outline of a cell about to be painted.
const PAINT_COLOR: [f32; 4] = [0.35, 0.85, 1.0, 0.9];
/// Outline of a cell about to be erased.
const ERASE_COLOR: [f32; 4] = [1.0, 0.4, 0.35, 0.9];
/// Outline thickness, in pixels; the other overlays' width.
const WIDTH: f32 = 1.5;

/// The armed paint tool, the tile it paints, and the stroke under way.
#[derive(Debug, Default)]
pub struct Paint {
    tool: Option<PaintTool>,
    /// What the brush and the rectangle write. Picked in the inspector's
    /// palette; frame 0, unmirrored, until then.
    pub brush: Tile,
    stroke: Option<Stroke>,
    /// The cell under the pointer this frame, for the outline.
    hovered: Option<IVec2>,
}

/// A drag that started on a layer, and holds it until the button comes up.
#[derive(Debug, Clone, Copy)]
struct Stroke {
    layer: Entity,
    /// Carried rather than read from [`Paint::tool`] each frame, so a key
    /// pressed mid-stroke arms the next stroke instead of changing this one —
    /// the gizmo's rule, for the same reason.
    tool: PaintTool,
    anchor: IVec2,
    last: IVec2,
}

impl Paint {
    pub fn tool(&self) -> Option<PaintTool> {
        self.tool
    }

    /// Arms `tool`, or disarms painting for `None`.
    pub fn set_tool(&mut self, tool: Option<PaintTool>) {
        self.tool = tool;
    }

    /// The selection, if it is a layer and a paint tool is armed: the layer a
    /// viewport drag paints.
    pub fn target(&self, world: &World, selected: Option<Entity>) -> Option<Entity> {
        self.tool?;
        selected.filter(|&entity| world.is_alive(entity) && world.get::<Tilemap>(entity).is_some())
    }

    /// Runs the pointer against `layer` for one frame. Returns what to call
    /// the history entry while a stroke is live, including the frame it ends.
    pub fn update(
        &mut self,
        ui: &Ui,
        scene: &Response,
        frame: &mut UiFrame<'_>,
        layer: Entity,
    ) -> Option<&'static str> {
        let Some(armed) = self.tool else {
            self.stroke = None;
            return None;
        };
        let primary = ui.input(|i| i.pointer.primary_down());
        // The interaction position while dragging, so a stroke keeps
        // following the pointer out past the edge of the panel.
        let pointer = scene.interact_pointer_pos().or_else(|| scene.hover_pos());
        let under = pointer.and_then(|pointer| {
            let at = crate::picking::world_at(scene, pointer, frame);
            cell::cell_under(frame.world, layer, at)
        });
        self.hovered = scene.hover_pos().and(under);

        // A stroke whose layer was deselected, despawned or respawned by an
        // undo is over; its entity no longer names what it was painting.
        if self.stroke.is_some_and(|stroke| stroke.layer != layer) {
            self.stroke = None;
        }

        let Some(mut stroke) = self.stroke else {
            if primary && scene.is_pointer_button_down_on() {
                if let Some(cell) = under {
                    let stroke = Stroke {
                        layer,
                        tool: armed,
                        anchor: cell,
                        last: cell,
                    };
                    self.apply_line(frame.world, stroke, cell);
                    self.stroke = Some(stroke);
                    return Some(armed.label());
                }
            }
            return None;
        };

        if let Some(cell) = under {
            self.apply_line(frame.world, stroke, cell);
            stroke.last = cell;
        }
        if primary {
            self.stroke = Some(stroke);
        } else {
            // Released, wherever the pointer is: the rectangle lands on the
            // last cell the pointer was over.
            if stroke.tool == PaintTool::Rectangle {
                if let Some(map) = frame.world.get_mut::<Tilemap>(layer) {
                    stroke::fill(map, stroke.anchor, stroke.last, Some(self.brush));
                }
            }
            self.stroke = None;
        }
        Some(stroke.tool.label())
    }

    /// Paints or erases from the stroke's last cell to `to`, for the tools
    /// that write as they go.
    fn apply_line(&self, world: &mut World, stroke: Stroke, to: IVec2) {
        let tile = match stroke.tool {
            PaintTool::Brush => Some(self.brush),
            PaintTool::Eraser => None,
            PaintTool::Rectangle => return,
        };
        if let Some(map) = world.get_mut::<Tilemap>(stroke.layer) {
            stroke::line(map, stroke.last, to, tile);
        }
    }

    /// Outlines the cell under the pointer, or the rectangle being dragged.
    pub fn draw(&self, world: &World, layer: Entity, lines: &mut LineBatch) {
        let (tool, a, b) = match self.stroke {
            Some(stroke) if stroke.tool == PaintTool::Rectangle => {
                (stroke.tool, stroke.anchor, stroke.last)
            }
            _ => match (self.tool, self.hovered) {
                (Some(tool), Some(cell)) => (tool, cell, cell),
                _ => return,
            },
        };
        let Some(corners) = cell::corners(world, layer, a, b) else {
            return;
        };
        let color = if tool.erases() {
            ERASE_COLOR
        } else {
            PAINT_COLOR
        };
        for i in 0..corners.len() {
            lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;
    use voltra_scene::Transform;

    use super::*;

    fn world_with_layer() -> (World, Entity, Entity) {
        let mut world = World::new();
        let layer = world.spawn();
        world.insert(layer, Transform::default());
        world.insert(layer, Tilemap::new(Vec2::ONE));
        let sprite = world.spawn();
        world.insert(sprite, Transform::default());
        (world, layer, sprite)
    }

    #[test]
    fn an_unarmed_brush_targets_nothing() {
        let (world, layer, _) = world_with_layer();
        assert_eq!(Paint::default().target(&world, Some(layer)), None);
    }

    #[test]
    fn an_armed_brush_targets_the_selected_layer_only() {
        let (mut world, layer, sprite) = world_with_layer();
        let mut paint = Paint::default();
        paint.set_tool(Some(PaintTool::Brush));

        assert_eq!(paint.target(&world, Some(layer)), Some(layer));
        assert_eq!(paint.target(&world, Some(sprite)), None);
        assert_eq!(paint.target(&world, None), None);

        world.despawn(layer);
        assert_eq!(paint.target(&world, Some(layer)), None, "a despawned layer");
    }
}
