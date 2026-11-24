//! Which paint tool a viewport drag on a tilemap layer uses.
//!
//! The keys are Unity's Tile Palette ones — `B` brush, `U` box fill, `D`
//! erase, `G` flood fill — for the reason `W`/`E`/`R` are the transform keys: a binding
//! someone already knows beats a locally nicer one. Godot's `D`/`R`/`L`
//! would put the rectangle on `R`, which is already Scale here.

use voltra_core::egui::Key;

/// A tool that edits the cells of the selected layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTool {
    /// Drag to paint the brush tile into every cell the pointer crosses.
    Brush,
    /// Drag out a rectangle; release fills it with the brush tile.
    Rectangle,
    /// Drag to empty every cell the pointer crosses.
    Eraser,
    /// Click to fill the area joined to the clicked cell that holds what it
    /// holds.
    Fill,
}

impl PaintTool {
    /// Every paint tool, in toolbar order.
    pub const ALL: [Self; 4] = [Self::Brush, Self::Rectangle, Self::Eraser, Self::Fill];

    /// The key that selects this tool.
    pub fn key(self) -> Key {
        match self {
            Self::Brush => Key::B,
            Self::Rectangle => Key::U,
            Self::Eraser => Key::D,
            Self::Fill => Key::G,
        }
    }

    /// What the history calls one stroke of this tool.
    pub fn label(self) -> &'static str {
        match self {
            Self::Brush => "Paint tiles",
            Self::Rectangle => "Fill rectangle",
            Self::Eraser => "Erase tiles",
            Self::Fill => "Fill area",
        }
    }

    /// The glyph the toolbar draws for this tool.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Brush => "🖌",
            Self::Rectangle => "⬛",
            Self::Eraser => "⌫",
            Self::Fill => "💧",
        }
    }

    /// The tooltip the toolbar shows, key included.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Brush => "Brush (B) — drag to paint the picked tile on the selected layer",
            Self::Rectangle => "Rectangle (U) — drag out a box; release fills it",
            Self::Eraser => "Eraser (D) — drag to empty cells of the selected layer",
            Self::Fill => "Fill (G) — click to fill the area joined to a cell",
        }
    }

    /// The tool a bridge request names: `brush`, `rectangle`, `eraser` or
    /// `fill`.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "brush" => Some(Self::Brush),
            "rectangle" => Some(Self::Rectangle),
            "eraser" => Some(Self::Eraser),
            "fill" => Some(Self::Fill),
            _ => None,
        }
    }

    /// Whether a stroke of this tool empties cells rather than filling them.
    pub fn erases(self) -> bool {
        self == Self::Eraser
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;

    #[test]
    fn no_paint_key_is_a_transform_key() {
        // The viewport reads both sets of keys in the same frame; a shared key
        // would switch to two tools at once.
        for paint in PaintTool::ALL {
            for tool in Tool::ALL {
                assert_ne!(paint.key(), tool.key(), "{paint:?} and {tool:?}");
            }
        }
    }

    #[test]
    fn every_paint_tool_has_its_own_key_and_label() {
        let mut keys: Vec<Key> = PaintTool::ALL.iter().map(|t| t.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), PaintTool::ALL.len());

        let mut labels: Vec<&str> = PaintTool::ALL
            .iter()
            .map(|t| t.label())
            .chain(Tool::ALL.iter().map(|t| t.label()))
            .collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), PaintTool::ALL.len() + Tool::ALL.len());
    }
}
