//! Fonts, keyed by the path a scene file names them with; text shaped,
//! broken into lines and rasterized from them.

pub mod atlas;
mod breaks;
mod cache;
pub mod embolden;
pub mod font;
mod itemize;
pub mod layout;
mod place;
mod shape;
pub mod shelf;
pub mod span;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use voltra_render::wgpu::{Device, Queue};
use voltra_render::Texture;

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;
use crate::textures::Textures;

pub use atlas::{Glyph, GlyphAtlas, GlyphKey, GlyphMetrics, GlyphRect};
pub use font::{Font, LineMetrics};
pub use layout::{CaretStop, PlacedGlyph, PlacedObject, TextLayout, Wrap};
pub use span::{InlineBox, RunStyle, Span};

/// The font a `Text` naming none draws with, relative to the asset root.
///
/// Ubuntu Light, the face egui ships, under the Ubuntu Font Licence beside
/// it. A file in the project rather than bytes in the binary, so it hot
/// reloads and can be swapped like any other asset.
pub const DEFAULT_FONT: &str = "fonts/Ubuntu-Light.ttf";

/// Loads fonts from an asset root and hands out shared handles to them;
/// lays text out in them and keeps the glyphs it draws.
///
/// The same shape as [`Textures`] and [`Atlases`](crate::Atlases) — load
/// once per path, a stable handle, reload in place. Needs no GPU except in
/// [`upload`](Self::upload): faces, layout and the atlas image are CPU work,
/// which keeps every test of them headless.
///
/// A glyph a text's font lacks is taken from the next font of its chain:
/// the text's own font, then the [fallbacks](Self::set_fallbacks) the
/// current language names, then the default font.
#[derive(Debug)]
pub struct Fonts {
    root: PathBuf,
    store: Assets<Font>,
    by_path: HashMap<AssetPath, Handle<Font>>,
    /// Every handle issued, by slot index: a glyph key names its font by
    /// index.
    handles: Vec<Handle<Font>>,
    default: Handle<Font>,
    /// The default font, cloned into the slot of a path that will not
    /// load, so a broken font still draws legibly — and the slot is its own,
    /// so repairing the file repairs only the texts naming it.
    fallback: Font,
    /// Tried after a text's own font, before the default.
    fallbacks: Vec<Handle<Font>>,
    fallback_paths: Vec<AssetPath>,
    /// Every font's glyphs, in one image.
    atlas: GlyphAtlas,
    /// The GPU copy of `atlas`, once [`upload`](Self::upload) has made one.
    /// Stable across growths and reloads, for the same reason every other
    /// handle in this crate is.
    texture: Option<Handle<Texture>>,
    layouts: cache::LayoutCache,
}

impl Fonts {
    /// A store rooted at `root`, with [`DEFAULT_FONT`] already loaded.
    ///
    /// A missing default is logged, not fatal: texts naming no font then
    /// draw nothing until the file appears and is reloaded.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let path = AssetPath::new(DEFAULT_FONT).expect("the default font path is a valid path");
        let fallback = match read(&root, &path) {
            Ok(font) => font,
            Err(e) => {
                log::warn!("{e}; texts with no font will draw nothing");
                Font::empty()
            }
        };

        let mut store = Assets::new();
        let default = store.insert(fallback.clone());
        let mut by_path = HashMap::new();
        by_path.insert(path, default);
        Self {
            root,
            store,
            by_path,
            handles: vec![default],
            default,
            fallback,
            fallbacks: Vec::new(),
            fallback_paths: Vec::new(),
            atlas: GlyphAtlas::default(),
            texture: None,
            layouts: cache::LayoutCache::default(),
        }
    }

    /// The handle for `path`, reading it if this is the first time.
    ///
    /// Infallible, as the other stores are: a file that will not parse logs
    /// once and gets a slot drawing with the default face. The answer is
    /// cached, or a broken file would be re-read every frame.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Font> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }
        let font = match read(&self.root, path) {
            Ok(font) => font,
            Err(e) => {
                log::warn!("{e}; drawing with the default font instead");
                self.fallback.clone()
            }
        };
        let handle = self.store.insert(font);
        self.by_path.insert(path.clone(), handle);
        self.handles.push(handle);
        handle
    }

    /// Re-reads `path` under the handle it already has. Returns whether
    /// anything changed.
    ///
    /// A path nothing has loaded is ignored, and a file that will not parse
    /// keeps the face already loaded — an editor's save leaves it briefly
    /// truncated. A successful reload starts the glyph atlas and the
    /// layouts over: every glyph id may now mean another shape.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };
        let font = match read(&self.root, path) {
            Ok(font) => font,
            Err(e) => {
                log::warn!("{e}; keeping the previously loaded font");
                return false;
            }
        };
        if handle == self.default {
            self.fallback = font.clone();
        }
        *self
            .store
            .get_mut(handle)
            .expect("Fonts never removes, so every handle it issued resolves") = font;
        self.atlas.clear();
        self.layouts.clear();
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// Makes `paths`, in order, the fonts tried after a text's own and
    /// before the default — the current language's, from its string table.
    /// Loads each the first time; a call naming the same paths again does
    /// nothing, so it can run every frame.
    pub fn set_fallbacks(&mut self, paths: &[AssetPath]) {
        if self.fallback_paths == paths {
            return;
        }
        self.fallbacks = paths.iter().map(|path| self.load(path)).collect();
        self.fallback_paths = paths.to_vec();
        self.layouts.clear();
    }

    /// The fonts tried after a text's own, before the default.
    pub fn fallbacks(&self) -> &[Handle<Font>] {
        &self.fallbacks
    }

    /// The fonts a text in `font` takes glyphs from, in order: `font`, the
    /// fallbacks, the default; each once.
    pub fn chain(&self, font: Handle<Font>) -> Vec<Handle<Font>> {
        let mut chain = Vec::with_capacity(self.fallbacks.len() + 2);
        for handle in std::iter::once(font)
            .chain(self.fallbacks.iter().copied())
            .chain(std::iter::once(self.default))
        {
            if !chain.contains(&handle) {
                chain.push(handle);
            }
        }
        chain
    }

    /// The first font of `font`'s chain that has a face, if any does.
    pub fn first_loaded(&self, font: Handle<Font>) -> Option<Handle<Font>> {
        self.chain(font)
            .into_iter()
            .find(|handle| self.try_get(*handle).is_some_and(Font::is_loaded))
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Font>> {
        self.by_path.get(path).copied()
    }

    /// The font a text naming none draws with.
    pub fn default_font(&self) -> Handle<Font> {
        self.default
    }

    /// The font `handle` names, or `None` for a handle this store never
    /// issued.
    pub fn try_get(&self, handle: Handle<Font>) -> Option<&Font> {
        self.store.get(handle)
    }

    /// `text` shaped and broken into lines, set as `spans` say; `None` when
    /// the first span's chain has no face at all.
    ///
    /// Cached by its inputs, so asking again in the same frame — the
    /// rasterize, the draw, the pick — or the next one costs a hash.
    pub fn layout(&self, text: &str, spans: &[Span], wrap: Wrap) -> Option<Arc<TextLayout>> {
        self.layouts.get_or(text, spans, wrap, || {
            layout::layout(self, text, spans, wrap)
        })
    }

    /// [`layout`](Self::layout) without the cache: the full cost of
    /// shaping and breaking `text`, every call — what a benchmark measures
    /// and what the cache saves.
    pub fn layout_uncached(&self, text: &str, spans: &[Span], wrap: Wrap) -> Option<TextLayout> {
        layout::layout(self, text, spans, wrap)
    }

    /// [`layout`](Self::layout) of a plain string, all in `font` at `px`.
    pub fn layout_plain(
        &self,
        font: Handle<Font>,
        content: &str,
        px: u32,
        wrap: Wrap,
    ) -> Option<Arc<TextLayout>> {
        self.layout(
            content,
            &[Span::whole(content, RunStyle::plain(font, px))],
            wrap,
        )
    }

    /// Rasterizes the glyphs of `content` in `font` at `px` that the atlas
    /// lacks; see [`rasterize`](Self::rasterize).
    pub fn prepare(&mut self, font: Handle<Font>, content: &str, px: u32) {
        if let Some(layout) = self.layout_plain(font, content, px, Wrap::default()) {
            self.rasterize(&layout);
        }
    }

    /// Rasterizes every glyph `layout` places that the atlas does not hold.
    ///
    /// Separate from drawing because the atlas may grow and repack while
    /// glyphs are added, which moves every glyph already in it: UVs computed
    /// before the last insertion of the frame would be wrong.
    pub fn rasterize(&mut self, layout: &TextLayout) {
        for placed in &layout.glyphs {
            let key = placed.key;
            if self.atlas.contains(key) {
                continue;
            }
            let Some(&handle) = self.handles.get(key.font as usize) else {
                continue;
            };
            let Some(font) = self.store.get(handle) else {
                continue;
            };
            if let Some((metrics, coverage)) = font.rasterize(key.glyph, key.px, key.embolden) {
                self.atlas.insert(key, metrics, coverage);
            }
        }
    }

    /// Every font's glyphs rasterized so far.
    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    /// The texture the atlas is drawn from, or `None` before its first
    /// upload.
    pub fn texture(&self) -> Option<Handle<Texture>> {
        self.texture
    }

    /// Uploads the atlas if it changed since its last upload, creating its
    /// texture the first time.
    pub fn upload(&mut self, textures: &mut Textures, device: &Device, queue: &Queue) {
        if !self.atlas.is_dirty() {
            return;
        }
        let label = "glyphs";
        let (side, pixels) = (self.atlas.side(), self.atlas.pixels());
        match self.texture {
            Some(texture) => {
                textures.replace(device, queue, texture, label, pixels, side, side);
            }
            None => {
                self.texture =
                    Some(textures.insert_generated(device, queue, label, pixels, side, side));
            }
        }
        self.atlas.mark_clean();
    }

    /// Points the atlas at `texture` without uploading anything, for tests
    /// that assert on a batch headless.
    ///
    /// The escape hatch [`Handle::forge`] is, for the same reason: the real
    /// path, [`upload`](Self::upload), needs a device.
    #[doc(hidden)]
    pub fn forge_texture(&mut self, texture: Handle<Texture>) {
        self.texture = Some(texture);
        self.atlas.mark_clean();
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many fonts are stored.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether no font is stored.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

/// Reads and parses one font file. The only place this module touches the
/// disk.
fn read(root: &Path, path: &AssetPath) -> Result<Font, AssetError> {
    let full = root.join(path.as_str());
    let bytes = crate::fs::read(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    Font::from_bytes(bytes).map_err(|message| AssetError::Font {
        path: full,
        message,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod shaping_tests;

#[cfg(test)]
mod bidi_tests;
