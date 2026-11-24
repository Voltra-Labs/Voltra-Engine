//! Fonts, keyed by the path a scene file names them with.

pub mod atlas;
pub mod font;
pub mod layout;
pub mod shelf;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_render::wgpu::{Device, Queue};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;
use crate::textures::Textures;

pub use atlas::{Glyph, GlyphAtlas, GlyphKey, GlyphMetrics, GlyphRect};
pub use font::Font;
pub use layout::{PlacedGlyph, TextLayout, Wrap};

/// The font a `Text` naming none draws with, relative to the asset root.
///
/// Ubuntu Light, the face egui ships, under the Ubuntu Font Licence beside
/// it. A file in the project rather than bytes in the binary, so it hot
/// reloads and can be swapped like any other asset.
pub const DEFAULT_FONT: &str = "fonts/Ubuntu-Light.ttf";

/// Loads fonts from an asset root and hands out shared handles to them.
///
/// The same shape as [`Textures`] and [`Atlases`](crate::Atlases) — load
/// once per path, a stable handle, reload in place. Needs no GPU except in
/// [`upload`](Self::upload): faces, layout and the atlas image are CPU work,
/// which keeps every test of them headless.
#[derive(Debug)]
pub struct Fonts {
    root: PathBuf,
    store: Assets<Font>,
    by_path: HashMap<AssetPath, Handle<Font>>,
    default: Handle<Font>,
    /// The default font's face, cloned into the slot of a path that will not
    /// load, so a broken font still draws legibly — and the slot is its own,
    /// so repairing the file repairs only the texts naming it.
    fallback: Option<fontdue::Font>,
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
            Ok(face) => Some(face),
            Err(e) => {
                log::warn!("{e}; texts with no font will draw nothing");
                None
            }
        };

        let mut store = Assets::new();
        let default = store.insert(Font::new(fallback.clone()));
        let mut by_path = HashMap::new();
        by_path.insert(path, default);
        Self {
            root,
            store,
            by_path,
            default,
            fallback,
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
        let face = match read(&self.root, path) {
            Ok(face) => Some(face),
            Err(e) => {
                log::warn!("{e}; drawing with the default font instead");
                self.fallback.clone()
            }
        };
        let handle = self.store.insert(Font::new(face));
        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Re-reads `path` under the handle it already has. Returns whether
    /// anything changed.
    ///
    /// A path nothing has loaded is ignored, and a file that will not parse
    /// keeps the face already loaded — an editor's save leaves it briefly
    /// truncated.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };
        let face = match read(&self.root, path) {
            Ok(face) => face,
            Err(e) => {
                log::warn!("{e}; keeping the previously loaded font");
                return false;
            }
        };
        if handle == self.default {
            self.fallback = Some(face.clone());
        }
        self.store
            .get_mut(handle)
            .expect("Fonts never removes, so every handle it issued resolves")
            .replace_face(face);
        log::info!("reloaded {}", path.as_str());
        true
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

    /// Rasterizes the glyphs of `content` at `px` that `handle`'s atlas
    /// lacks. Run for every text before the frame's batch is built.
    pub fn prepare(&mut self, handle: Handle<Font>, content: &str, px: u32) {
        if let Some(font) = self.store.get_mut(handle) {
            font.prepare(content, px);
        }
    }

    /// Uploads every atlas that changed since its last upload, creating its
    /// texture the first time.
    pub fn upload(&mut self, textures: &mut Textures, device: &Device, queue: &Queue) {
        for (path, &handle) in &self.by_path {
            let Some(font) = self.store.get_mut(handle) else {
                continue;
            };
            if !font.atlas().is_dirty() {
                continue;
            }
            let label = format!("glyphs:{}", path.as_str());
            let atlas = font.atlas();
            let (side, pixels) = (atlas.side(), atlas.pixels());
            match font.texture() {
                Some(texture) => {
                    textures.replace(device, queue, texture, &label, pixels, side, side);
                }
                None => {
                    let texture =
                        textures.insert_generated(device, queue, &label, pixels, side, side);
                    font.set_texture(texture);
                }
            }
            font.atlas_mut().mark_clean();
        }
    }

    /// Points `handle`'s atlas at `texture` without uploading anything, for
    /// tests that assert on a batch headless.
    ///
    /// The escape hatch [`Handle::forge`] is, for the same reason: the real
    /// path, [`upload`](Self::upload), needs a device.
    #[doc(hidden)]
    pub fn forge_texture(&mut self, handle: Handle<Font>, texture: Handle<voltra_render::Texture>) {
        if let Some(font) = self.store.get_mut(handle) {
            font.set_texture(texture);
            font.atlas_mut().mark_clean();
        }
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

/// Reads and parses one font file. The only place this module touches the
/// disk.
fn read(root: &Path, path: &AssetPath) -> Result<fontdue::Font, AssetError> {
    let full = root.join(path.as_str());
    let bytes = crate::fs::read(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).map_err(|message| {
        AssetError::Font {
            path: full,
            message,
        }
    })
}

#[cfg(test)]
mod tests;
