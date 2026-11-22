//! What a quad is drawn with besides its corners: the run it joins.

use voltra_assets::Handle;
use voltra_render::{Texture, DEFAULT_LIGHT_LAYERS};

/// The texture, normal map and light layers a quad draws with. Quads in a
/// row with the same surface share a [`SpriteRange`](super::SpriteRange).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    /// `None` draws against the renderer's white bind group.
    pub texture: Option<Handle<Texture>>,
    /// The normal map over `texture`'s texels; `None` is flat.
    pub normal: Option<Handle<Texture>>,
    /// The light layers the quad is on.
    pub light_layers: u32,
}

impl Surface {
    /// `texture`, flat, on the default light layer.
    pub fn textured(texture: Option<Handle<Texture>>) -> Self {
        Self {
            texture,
            normal: None,
            light_layers: DEFAULT_LIGHT_LAYERS,
        }
    }

    /// The same surface on `light_layers`.
    pub fn on(self, light_layers: u32) -> Self {
        Self {
            light_layers,
            ..self
        }
    }
}
