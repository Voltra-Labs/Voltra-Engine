//! Surface materials drawn on a lit sphere for a panel to show.

use voltra_assets::SurfaceSource;

use super::UiFrame;
use crate::ui::TextureId;

impl UiFrame<'_> {
    /// `source` drawn on a lit sphere, `size` pixels a side, for a panel
    /// to show — the inspector's live preview of a material being edited,
    /// the asset browser's thumbnail of a material file.
    ///
    /// `key` names the preview: one asked for under the same key and size
    /// reuses its target, and is drawn again only when `source` or one of
    /// its images changed. The images are loaded as it is drawn. Drawn after
    /// the layout; `None` the first frame a key is asked for — draw a blank
    /// and ask again, as for [`thumbnail`](Self::thumbnail).
    pub fn surface_preview(
        &mut self,
        key: &str,
        size: u32,
        source: &SurfaceSource,
    ) -> Option<TextureId> {
        self.previews.ask(key, size, source)
    }
}
