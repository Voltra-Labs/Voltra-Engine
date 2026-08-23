//! What a caller drawing a sprite needs to ask of the texture store.

use voltra_render::glam::UVec2;
use voltra_render::Texture;

use crate::handle::Handle;
use crate::textures::Textures;

/// The size of a loaded texture, and the handle that stands in for a missing
/// one.
///
/// A trait rather than `&Textures` because `Textures` cannot exist without a
/// GPU device, and the geometry it feeds — the quad a sprite covers and the
/// UVs it samples — is exactly what has to stay testable headless. The editor
/// and the draw path pass `Textures`; a test passes a map.
pub trait TextureSizes {
    /// The size of `texture` in texels, or `None` for a handle this store
    /// never issued.
    fn size(&self, texture: Handle<Texture>) -> Option<UVec2>;

    /// The handle to draw with when a sprite asks for a frame that is not
    /// there.
    ///
    /// Beside `size` rather than in a trait of its own: both are questions
    /// about the same store, asked at the same moment by the same caller, and
    /// splitting them would thread two parameters through the batch to draw
    /// one quad.
    fn placeholder(&self) -> Handle<Texture>;
}

impl TextureSizes for Textures {
    fn size(&self, texture: Handle<Texture>) -> Option<UVec2> {
        let texture = self.try_get(texture)?;
        Some(UVec2::new(texture.width(), texture.height()))
    }

    fn placeholder(&self) -> Handle<Texture> {
        Textures::placeholder(self)
    }
}
