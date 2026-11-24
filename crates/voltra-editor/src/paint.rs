//! Painting a tilemap layer's cells from the viewport.
//!
//! **A mode of the viewport while a layer is selected**, Godot's shape: with a
//! paint tool armed and a [`Tilemap`] selected, a drag paints instead of
//! grabbing the gizmo, and a click paints instead of selecting. Unity keeps a
//! separate Tile Palette window whose tools own the scene view the same way;
//! the Tile Palette window here picks what is painted ([`stamp`]).
//! `W`/`E`/`R` — or selecting something that is not a layer — disarms it.
//!
//! **What is painted is a stamp**: one tile, or a block picked off the
//! palette or — Alt held, the eyedropper — off the map, turned with `Z`
//! (Shift for the other way) and mirrored with `X` and `Y`, Tiled's keys.
//!
//! **One stroke, one undo entry.** The selection is what the history watches,
//! and a stroke edits only the selected layer, so the stroke claims the open
//! entry every frame it is live, exactly as a gizmo drag does.

mod cell;
pub mod fill;
mod gesture;
mod line;
pub mod palette;
pub mod stamp;
mod stroke;
mod tool;

use voltra_core::egui::{Key, Response, Ui};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::IVec2;
use voltra_render::LineBatch;
use voltra_scene::Tilemap;

use gesture::Stroke;
pub use stamp::Stamp;
pub use tool::PaintTool;

/// Outline of a cell about to be painted.
const PAINT_COLOR: [f32; 4] = [0.35, 0.85, 1.0, 0.9];
/// Outline of a cell about to be erased.
const ERASE_COLOR: [f32; 4] = [1.0, 0.4, 0.35, 0.9];
/// Outline of the cells the eyedropper is about to pick.
const PICK_COLOR: [f32; 4] = [1.0, 0.85, 0.3, 0.9];
/// Outline thickness, in pixels; the other overlays' width.
const WIDTH: f32 = 1.5;

/// The armed paint tool, the stamp it paints, and the stroke under way.
#[derive(Debug, Default)]
pub struct Paint {
    tool: Option<PaintTool>,
    /// What the brush, the rectangle and the bucket write. Picked in the
    /// palette or by the eyedropper; frame 0, unmirrored, until then.
    pub stamp: Stamp,
    /// The terrain the brush, the rectangle, the bucket and the eraser
    /// paint with instead of the stamp, if one is chosen in the inspector:
    /// cells join up with their neighbours as they are drawn.
    pub terrain: Option<usize>,
    stroke: Option<Stroke>,
    /// The cell under the pointer this frame, for the outline.
    hovered: Option<IVec2>,
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
        if scene.hovered() && !ui.ctx().egui_wants_keyboard_input() {
            self.turn_keys(ui);
        }
        let (primary, alt) = ui.input(|i| (i.pointer.primary_down(), i.modifiers.alt));
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

        let Some(stroke) = self.stroke else {
            if primary && scene.is_pointer_button_down_on() {
                if let Some(cell) = under {
                    return self.press(frame.world, layer, armed, alt, cell);
                }
            }
            return None;
        };

        if let Some(cell) = under {
            if cell != stroke.last {
                self.drag(frame.world, cell);
            }
        }
        if primary {
            stroke.label()
        } else {
            // Released, wherever the pointer is: the rectangle lands on the
            // last cell the pointer was over.
            self.release(frame.world)
        }
    }

    /// `Z` turns the stamp clockwise (Shift+`Z` the other way), `X` and `Y`
    /// mirror it — Tiled's keys. Never with Ctrl or Cmd, which are undo's.
    fn turn_keys(&mut self, ui: &Ui) {
        ui.input(|i| {
            if i.modifiers.command {
                return;
            }
            if i.key_pressed(Key::Z) {
                self.stamp = if i.modifiers.shift {
                    self.stamp.rotated_ccw()
                } else {
                    self.stamp.rotated_cw()
                };
            }
            if i.key_pressed(Key::X) {
                self.stamp = self.stamp.mirrored_x();
            }
            if i.key_pressed(Key::Y) {
                self.stamp = self.stamp.mirrored_y();
            }
        });
    }

    /// The chosen terrain, if `map` has one at that index — a choice made on
    /// one layer means nothing on a layer with fewer terrains.
    fn terrain_on(&self, map: &Tilemap) -> Option<usize> {
        self.terrain.filter(|&index| index < map.terrains.len())
    }

    /// Outlines what is about to change: the stamp under the pointer, the
    /// rectangle being dragged, or what the eyedropper is about to pick.
    pub fn draw(&self, world: &World, layer: Entity, lines: &mut LineBatch) {
        let (color, a, b) = match self.stroke {
            Some(stroke) if stroke.picks => (PICK_COLOR, stroke.anchor, stroke.last),
            Some(stroke) if stroke.tool == PaintTool::Rectangle => {
                (PAINT_COLOR, stroke.anchor, stroke.last)
            }
            _ => match (self.tool, self.hovered) {
                (Some(tool), Some(cell)) if tool.erases() => (ERASE_COLOR, cell, cell),
                (Some(PaintTool::Brush), Some(cell)) if self.terrain.is_none() => {
                    let size = self.stamp.size().as_ivec2();
                    (PAINT_COLOR, cell, cell + size - IVec2::ONE)
                }
                (Some(_), Some(cell)) => (PAINT_COLOR, cell, cell),
                _ => return,
            },
        };
        let Some(corners) = cell::corners(world, layer, a, b) else {
            return;
        };
        for i in 0..corners.len() {
            lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
        }
    }
}

#[cfg(test)]
mod tests;
