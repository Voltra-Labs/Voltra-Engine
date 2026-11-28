//! Hot reload in a headless run: a file's change applied as the editor's
//! watch applies it, without the watch's timing.

use voltra_assets::AssetPath;

use super::Headless;

impl Headless {
    /// Reloads `path` as the editor's file watch would when the file
    /// changes — a texture, a material or the shader one names, a prefab, a
    /// clip — in whichever store holds it, under its handle.
    pub fn reload(&mut self, path: &AssetPath) {
        self.app.reload_assets(vec![path.clone()]);
    }
}
