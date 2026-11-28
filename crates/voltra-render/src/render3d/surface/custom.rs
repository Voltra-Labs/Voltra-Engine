//! What a custom surface adds to a standard one on the GPU: its shader,
//! its uniform block and its own textures.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::shader::SurfaceShader;
use crate::material::BLOCK_SIZE;
use crate::texture::Texture;

/// How a custom texture's bytes are read, as the standard slots are
/// ("Data textures" in ARCHITECTURE's wgpu notes): a colour through the
/// sRGB view, linear when sampled; data through the raw view, its bytes as
/// written. Godot's `source_color` hint, the other way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextureSpace {
    /// A picture: decoded from sRGB.
    #[default]
    Color,
    /// Numbers — noise, a mask, a flow map: read as stored.
    Data,
}

/// One of a custom surface's own textures, and how it is read.
#[derive(Debug, Clone, Copy)]
pub struct CustomImage<'a> {
    /// The image.
    pub texture: &'a Texture,
    /// Colour or data.
    pub space: TextureSpace,
}

impl CustomImage<'_> {
    /// The view it is sampled through.
    pub fn view(&self) -> &wgpu::TextureView {
        match self.space {
            TextureSpace::Color => self.texture.view(),
            TextureSpace::Data => self.texture.raw_view(),
        }
    }
}

/// A custom surface's shading: its checked shader and its uniform block,
/// laid out by its [`Layout`](crate::material::Layout).
#[derive(Debug, Clone, PartialEq)]
pub struct CustomShading {
    /// The composed, checked shader.
    pub shader: Arc<SurfaceShader>,
    /// The material's uniforms as the shader's `material` reads them.
    pub block: [u8; BLOCK_SIZE],
}
