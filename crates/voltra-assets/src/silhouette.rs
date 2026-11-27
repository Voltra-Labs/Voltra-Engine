//! The outlines of textures' opaque texels, traced once per texture and
//! rectangle, for shadows cast by a sprite's own shape.
//!
//! Kept beside [`Textures`](crate::Textures) rather than in it: the GPU
//! store keeps no pixels on the CPU, and only the few textures some caster
//! traces need decoding twice. A rectangle — the frame a sprite draws — is
//! traced the first time it is asked for, so an animated sprite's frames
//! are traced as they come up and then shared.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use voltra_render::glam::Vec2;
use voltra_render::texture::DecodedImage;

use crate::path::AssetPath;
use crate::textures::decode_file;

mod trace;

pub use trace::{trace, ALPHA_THRESHOLD, TOLERANCE};

/// A rectangle of texels: x, y, width, height.
pub type Rect = [u32; 4];

/// Closed outlines, each a loop of points in `0..=1` across a rectangle.
pub type Outlines = Arc<Vec<Vec<Vec2>>>;

/// Traced outlines, keyed by texture path and rectangle.
pub struct Silhouettes {
    root: PathBuf,
    /// Each path's pixels, or `None` for one that would not load — kept so
    /// a broken path is reported once, not every frame it is asked for.
    images: HashMap<AssetPath, Option<Arc<DecodedImage>>>,
    /// Per path, so a lookup borrows the path instead of cloning it; per
    /// rectangle and alpha threshold under it.
    traced: HashMap<AssetPath, HashMap<(Rect, u8), Outlines>>,
}

impl Silhouettes {
    /// A store reading textures from `root`, the asset root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            images: HashMap::new(),
            traced: HashMap::new(),
        }
    }

    /// Traces `rect` of `path` at `threshold` alpha if it has not been. A
    /// file that will not load is logged once and traces nothing.
    pub fn ensure(&mut self, path: &AssetPath, rect: Rect, threshold: u8) {
        if self.get(path, rect, threshold).is_some() {
            return;
        }
        let root = &self.root;
        let image = self
            .images
            .entry(path.clone())
            .or_insert_with(|| match decode_file(root, path) {
                Ok(image) => Some(Arc::new(image)),
                Err(e) => {
                    log::warn!("{e}; its sprites cast no silhouette shadow");
                    None
                }
            })
            .clone();
        let outlines = image.map_or_else(Vec::new, |image| {
            trace(&image.rgba, image.width, image.height, rect, threshold)
        });
        self.traced
            .entry(path.clone())
            .or_default()
            .insert((rect, threshold), Arc::new(outlines));
    }

    /// The outlines of `rect` of `path` at `threshold`, in `0..=1` across
    /// it, if [`ensure`](Self::ensure) has traced them.
    pub fn get(&self, path: &AssetPath, rect: Rect, threshold: u8) -> Option<&[Vec<Vec2>]> {
        self.traced
            .get(path)?
            .get(&(rect, threshold))
            .map(|outlines| outlines.as_slice())
    }

    /// Forgets `path`'s pixels and outlines, so the next
    /// [`ensure`](Self::ensure) reads the file again. Returns whether there
    /// was anything to forget. For hot reload.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let had = self.images.remove(path).is_some();
        self.traced.remove(path);
        had
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &std::path::Path, name: &str, size: u32, opaque_left: u32) {
        use image::ImageEncoder;
        let pixels: Vec<u8> = (0..size * size)
            .flat_map(|i| [9, 9, 9, if i % size < opaque_left { 255 } else { 0 }])
            .collect();
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("dir");
        let file = std::fs::File::create(path).expect("file");
        image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
            .write_image(&pixels, size, size, image::ExtendedColorType::Rgba8)
            .expect("png");
    }

    fn root() -> voltra_testkit::Scratch {
        voltra_testkit::Scratch::new("voltra-silhouettes")
    }

    #[test]
    fn a_rectangle_is_traced_once_asked_for_and_forgotten_on_reload() {
        let root = root();
        write(&root, "sprites/half.png", 8, 4);
        let path = AssetPath::new("sprites/half.png").expect("valid");
        let mut store = Silhouettes::new(&root);
        assert!(
            store.get(&path, [0, 0, 8, 8], ALPHA_THRESHOLD).is_none(),
            "not traced yet"
        );
        store.ensure(&path, [0, 0, 8, 8], ALPHA_THRESHOLD);
        let outlines = store
            .get(&path, [0, 0, 8, 8], ALPHA_THRESHOLD)
            .expect("traced");
        assert_eq!(outlines.len(), 1);
        assert!(outlines[0].iter().all(|p| p.x <= 0.5 + 1e-6));

        write(&root, "sprites/half.png", 8, 8);
        assert!(store.reload(&path));
        assert!(
            store.get(&path, [0, 0, 8, 8], ALPHA_THRESHOLD).is_none(),
            "forgotten"
        );
        store.ensure(&path, [0, 0, 8, 8], ALPHA_THRESHOLD);
        let outlines = store
            .get(&path, [0, 0, 8, 8], ALPHA_THRESHOLD)
            .expect("traced again");
        assert!(outlines[0].iter().any(|p| p.x > 0.9), "the new pixels");
    }

    #[test]
    fn a_missing_file_traces_nothing_rather_than_failing() {
        let root = root();
        let mut store = Silhouettes::new(&root);
        let path = AssetPath::new("sprites/absent.png").expect("valid");
        store.ensure(&path, [0, 0, 4, 4], ALPHA_THRESHOLD);
        assert_eq!(
            store.get(&path, [0, 0, 4, 4], ALPHA_THRESHOLD),
            Some(&[][..])
        );
    }
}
