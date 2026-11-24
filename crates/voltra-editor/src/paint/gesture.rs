//! One stroke, from the press to the release, apart from where the cells
//! come from: the viewport's pointer, or the bridge's list of cells.
//!
//! The same press, drag and release either way, so a test driven through
//! the bridge paints exactly what a hand on the mouse would.

use voltra_ecs::{Entity, World};
use voltra_render::glam::IVec2;
use voltra_scene::Tilemap;

use super::stamp::Stamp;
use super::{fill, stroke, Paint, PaintTool};

/// A drag that started on a layer, and holds it until the button comes up.
#[derive(Debug, Clone, Copy)]
pub(super) struct Stroke {
    pub(super) layer: Entity,
    /// Carried rather than read from [`Paint::tool`] each frame, so a key
    /// pressed mid-stroke arms the next stroke instead of changing this one —
    /// the gizmo's rule, for the same reason.
    pub(super) tool: PaintTool,
    /// The eyedropper: Alt held when the stroke began. It picks a stamp
    /// off the map instead of painting.
    pub(super) picks: bool,
    pub(super) anchor: IVec2,
    pub(super) last: IVec2,
}

impl Stroke {
    /// What the history calls it, or `None` for a stroke that edits no
    /// cell — the eyedropper.
    pub(super) fn label(&self) -> Option<&'static str> {
        (!self.picks).then(|| self.tool.label())
    }
}

impl Paint {
    /// Starts a stroke of `tool` on `layer` at `cell`; `picks` makes it the
    /// eyedropper. Returns what the history calls it.
    pub(super) fn press(
        &mut self,
        world: &mut World,
        layer: Entity,
        tool: PaintTool,
        picks: bool,
        cell: IVec2,
    ) -> Option<&'static str> {
        let stroke = Stroke {
            layer,
            tool,
            picks,
            anchor: cell,
            last: cell,
        };
        if !picks {
            match tool {
                PaintTool::Brush | PaintTool::Eraser => self.apply_line(world, stroke, cell),
                PaintTool::Fill => self.fill(world, layer, cell),
                PaintTool::Rectangle => {}
            }
        }
        self.stroke = Some(stroke);
        stroke.label()
    }

    /// Carries the stroke under way to `cell`.
    pub(super) fn drag(&mut self, world: &mut World, cell: IVec2) {
        let Some(mut stroke) = self.stroke else {
            return;
        };
        if !stroke.picks {
            self.apply_line(world, stroke, cell);
        }
        stroke.last = cell;
        self.stroke = Some(stroke);
    }

    /// Ends the stroke under way where it last was: the rectangle lands, the
    /// eyedropper picks. Returns what the history calls it.
    pub(super) fn release(&mut self, world: &mut World) -> Option<&'static str> {
        let stroke = self.stroke.take()?;
        if stroke.picks {
            let map = world.get::<Tilemap>(stroke.layer)?;
            match Stamp::from_map(map, stroke.anchor, stroke.last) {
                Some(stamp) => self.stamp = stamp,
                None => log::info!("nothing there to pick"),
            }
            return None;
        }
        if stroke.tool == PaintTool::Rectangle {
            let terrain = world
                .get::<Tilemap>(stroke.layer)
                .and_then(|map| self.terrain_on(map));
            if let Some(map) = world.get_mut::<Tilemap>(stroke.layer) {
                match terrain {
                    Some(terrain) => {
                        stroke::fill_terrain(map, stroke.anchor, stroke.last, Some(terrain));
                    }
                    None => {
                        stroke::fill_stamp(map, stroke.anchor, stroke.last, &self.stamp);
                    }
                }
            }
        }
        stroke.label()
    }

    /// A whole stroke through `cells` at once — the first pressed, the last
    /// released — as the bridge drives it. Returns what the history calls it.
    pub(crate) fn stroke_through(
        &mut self,
        world: &mut World,
        layer: Entity,
        tool: PaintTool,
        picks: bool,
        cells: &[IVec2],
    ) -> Option<&'static str> {
        let (&first, rest) = cells.split_first()?;
        self.stroke = None;
        self.press(world, layer, tool, picks, first);
        for &cell in rest {
            self.drag(world, cell);
        }
        self.release(world)
    }

    /// Paints or erases from the stroke's last cell to `to`, for the tools
    /// that write as they go.
    fn apply_line(&self, world: &mut World, stroke: Stroke, to: IVec2) {
        let erase = match stroke.tool {
            PaintTool::Brush => false,
            PaintTool::Eraser => true,
            PaintTool::Rectangle | PaintTool::Fill => return,
        };
        if let Some(map) = world.get_mut::<Tilemap>(stroke.layer) {
            match self.terrain_on(map) {
                Some(terrain) => {
                    let terrain = (!erase).then_some(terrain);
                    stroke::line_terrain(map, stroke.last, to, terrain);
                }
                None if erase => stroke::line(map, stroke.last, to, None),
                None => stroke::line_stamp(map, stroke.last, to, &self.stamp),
            }
        }
    }

    /// The bucket at `cell`: the stamp laid as a pattern from the clicked
    /// cell, or the chosen terrain joined up, over the region it reaches.
    fn fill(&self, world: &mut World, layer: Entity, cell: IVec2) {
        let Some(map) = world.get_mut::<Tilemap>(layer) else {
            return;
        };
        let cells = fill::region(map, cell);
        match self.terrain_on(map) {
            Some(terrain) => {
                for at in cells {
                    map.paint_terrain(at, Some(terrain));
                }
            }
            None => {
                for at in cells {
                    if let Some(tile) = self.stamp.pattern(cell, at) {
                        map.set(at, Some(tile));
                    }
                }
            }
        }
    }
}
