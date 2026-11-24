//! The `.atlas.ron` the Atlas Editor has open: its regions, the sheet they
//! are cut from, and the edits that can be undone.
//!
//! **A document of its own, with a history of its own.** The scene's history
//! watches the selected entities; an atlas is a file no entity owns, and
//! routing its edits through the scene's entries would make Ctrl+Z in the
//! viewport take back a rename in a window. Unity's Sprite Editor keeps its
//! own revert/apply the same way, and Godot's TileSet editor its own undo
//! context. Snapshots, not commands: an atlas is a few hundred rectangles at
//! most, and a copy per edit is simpler than an inverse per operation.
//!
//! **Settled edits.** A drag or a name being typed changes the regions every
//! frame; [`AtlasDoc::settle`] records one entry once the pointer is let go
//! and the keyboard is free — one undo entry per gesture, as the scene's
//! history does it.

mod edit;
mod file;
pub mod slice;

use voltra_assets::{AssetPath, Frame, Trim};
use voltra_render::glam::{UVec2, Vec2};

/// One named rectangle of the sheet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Region {
    /// What it is called; empty for none, written without a name.
    pub name: String,
    /// Its rectangle on the sheet, in texels.
    pub frame: Frame,
    /// The point on the origin, `0..1` with `y` down; `None` is the centre.
    pub pivot: Option<Vec2>,
    /// Kept as read: the packer's trim, which the editor does not change.
    pub trim: Option<Trim>,
}

/// An open atlas.
#[derive(Debug, Clone)]
pub struct AtlasDoc {
    path: AssetPath,
    texture: AssetPath,
    sheet: UVec2,
    regions: Vec<Region>,
    /// What the history last recorded: an edit is what differs from it.
    committed: Vec<Region>,
    undo: Vec<Vec<Region>>,
    redo: Vec<Vec<Region>>,
    /// As last saved or opened, for the unsaved mark.
    saved: Vec<Region>,
    /// The region the window shows the fields of.
    pub selected: Option<usize>,
}

/// How many edits back an atlas can be undone.
const HISTORY: usize = 200;

impl AtlasDoc {
    /// Where it saves.
    pub fn path(&self) -> &AssetPath {
        &self.path
    }

    /// The sheet it cuts.
    pub fn texture(&self) -> &AssetPath {
        &self.texture
    }

    /// The sheet's size, in texels.
    pub fn sheet(&self) -> UVec2 {
        self.sheet
    }

    /// Every region, in index order.
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// Region `index`, to be changed in place — by a field being dragged,
    /// say; [`settle`](Self::settle) records the change.
    pub fn region_mut(&mut self, index: usize) -> Option<&mut Region> {
        self.regions.get_mut(index)
    }

    /// Whether it differs from the file.
    pub fn is_dirty(&self) -> bool {
        self.regions != self.saved
    }

    /// Records whatever changed since the last entry as one entry, if
    /// `settled` — nothing held, nothing being typed. Every edit method
    /// below settles itself; this is for changes made in place.
    pub fn settle(&mut self, settled: bool) {
        if !settled || self.regions == self.committed {
            return;
        }
        let before = std::mem::replace(&mut self.committed, self.regions.clone());
        self.undo.push(before);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.clamp_selection();
    }

    /// Takes back the last edit. Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        self.settle(true);
        let Some(before) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.regions, before.clone()));
        self.committed = before;
        self.clamp_selection();
        true
    }

    /// Puts back the last edit undone. Returns whether there was one.
    pub fn redo(&mut self) -> bool {
        self.settle(true);
        let Some(after) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push(std::mem::replace(&mut self.regions, after.clone()));
        self.committed = after;
        self.clamp_selection();
        true
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.regions != self.committed
    }

    /// Whether there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn clamp_selection(&mut self) {
        if self.selected.is_some_and(|i| i >= self.regions.len()) {
            self.selected = None;
        }
    }
}

#[cfg(test)]
mod tests;
