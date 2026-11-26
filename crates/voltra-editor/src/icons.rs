//! The glyphs the editor draws as icons on buttons.
//!
//! One list, so one test can prove egui's built-in fonts (Ubuntu Light, Noto
//! Emoji, emoji-icon-font) draw every one: a glyph none of them holds shows as
//! an empty box, which is what `✥`, `⤢`, `🖌`, `⌫`, `⇔`, `⇕` and `✕` did until
//! the editor was first looked at in a real window.

/// The translate tool (`W`): a cross, as Unity's move handle.
pub const MOVE: &str = "✚";
/// The rotate tool (`E`), and turning a tile stamp clockwise.
pub const ROTATE_CW: &str = "⟳";
/// Turning a tile stamp anticlockwise.
pub const ROTATE_CCW: &str = "⟲";
/// The scale tool (`R`): a frame's four corners.
pub const SCALE: &str = "⛶";

/// The tile brush (`B`).
pub const BRUSH: &str = "✏";
/// The box fill (`U`).
pub const RECTANGLE: &str = "⬛";
/// The tile eraser (`D`): an empty cell.
pub const ERASER: &str = "⬜";
/// The flood fill (`G`).
pub const FILL: &str = "💧";
/// Mirroring a tile stamp left to right.
pub const MIRROR_X: &str = "⬌";
/// Mirroring a tile stamp top to bottom.
pub const MIRROR_Y: &str = "⬍";

/// Removing a row: a collider, a component, a tile's custom field.
pub const REMOVE: &str = "🗙";

/// Play.
pub const PLAY: &str = "▶";
/// Pause.
pub const PAUSE: &str = "⏸";
/// One frame forward while paused.
pub const STEP: &str = "⏭";
/// Stop: the authored scene back.
pub const STOP: &str = "⏹";

/// Every icon, for the test that checks the fonts draw them.
#[cfg(test)]
const ALL: [&str; 15] = [
    MOVE, ROTATE_CW, ROTATE_CCW, SCALE, BRUSH, RECTANGLE, ERASER, FILL, MIRROR_X, MIRROR_Y, REMOVE,
    PLAY, PAUSE, STEP, STOP,
];

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_assets::fontdue;
    use voltra_core::egui::{FontDefinitions, FontFamily};

    /// Read from each font's own character map rather than egui's
    /// `has_glyphs`, which answers "no" for every glyph that lives in the
    /// same face as its replacement character (epaint 0.35 `Font::has_glyph`
    /// compares faces, not glyphs) — `▶`, `⬛` and `💧` among them.
    #[test]
    fn egui_s_built_in_fonts_draw_every_icon() {
        let definitions = FontDefinitions::default();
        let faces: Vec<fontdue::Font> = definitions.families[&FontFamily::Proportional]
            .iter()
            .map(|name| {
                let data = &definitions.font_data[name];
                let settings = fontdue::FontSettings {
                    collection_index: data.index,
                    ..Default::default()
                };
                fontdue::Font::from_bytes(&data.font[..], settings).expect("egui's own fonts read")
            })
            .collect();
        let drawn = |c: char| faces.iter().any(|face| face.lookup_glyph_index(c) != 0);
        let missing: Vec<&str> = ALL
            .into_iter()
            .filter(|icon| !icon.chars().all(drawn))
            .collect();
        assert!(missing.is_empty(), "drawn as empty boxes: {missing:?}");
    }
}
