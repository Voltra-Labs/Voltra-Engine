//! A surface material's own shading: its WGSL `surface()`, its uniforms
//! and its named textures, as the file writes them and as they resolve.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use voltra_render::material::{Layout, Uniform, BLOCK_SIZE};
use voltra_render::render3d::{
    compose_surface, CustomShading, SurfaceLighting, SurfaceShader, TextureSpace,
};

use crate::path::AssetPath;

/// One of a custom surface's textures in the file, by how its bytes are
/// read: `"noise": Data("textures/noise.png")`, `"decal":
/// Color("textures/decal.png")`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomTextureFile {
    /// A picture, decoded from sRGB.
    Color(String),
    /// Numbers, read as stored.
    Data(String),
}

impl CustomTextureFile {
    /// The path as written.
    pub fn path(&self) -> &str {
        match self {
            Self::Color(path) | Self::Data(path) => path,
        }
    }

    /// The path, to edit.
    pub fn path_mut(&mut self) -> &mut String {
        match self {
            Self::Color(path) | Self::Data(path) => path,
        }
    }

    /// How it is read.
    pub fn space(&self) -> TextureSpace {
        match self {
            Self::Color(_) => TextureSpace::Color,
            Self::Data(_) => TextureSpace::Data,
        }
    }

    /// The same path read as `space`.
    pub fn with_space(&self, space: TextureSpace) -> Self {
        let path = self.path().to_owned();
        match space {
            TextureSpace::Color => Self::Color(path),
            TextureSpace::Data => Self::Data(path),
        }
    }
}

/// A custom texture resolved: its WGSL name, its image and how it is read.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomTexturePath {
    /// What the shader calls it.
    pub name: String,
    /// The image, from the asset root; `None` for a slot named with no
    /// image yet (white, as a standard slot left empty).
    pub path: Option<AssetPath>,
    /// Colour or data.
    pub space: TextureSpace,
}

/// A surface material's own shading, composed and checked: what the frame
/// turns into a [`CustomShading`].
#[derive(Debug, Clone, PartialEq)]
pub struct CustomSurfaceSource {
    /// The composed, checked shader.
    pub shader: Arc<SurfaceShader>,
    /// Where each uniform sits in the block.
    pub layout: Layout,
    /// The uniforms' values, by name.
    pub uniforms: BTreeMap<String, Uniform>,
    /// Lit or not.
    pub lighting: SurfaceLighting,
}

impl CustomSurfaceSource {
    /// The uniform block for its values.
    pub fn block(&self) -> [u8; BLOCK_SIZE] {
        self.layout.block(&self.uniforms)
    }

    /// The shading the GPU material is made with.
    pub fn shading(&self) -> CustomShading {
        CustomShading {
            shader: Arc::clone(&self.shader),
            block: self.block(),
        }
    }
}

/// The textures `textures` names, checked as asset paths, in name order —
/// the order the shader binds them. One with no path yet is a slot with no
/// image: it reads white, and the shading still compiles.
pub fn custom_texture_paths(
    textures: &BTreeMap<String, CustomTextureFile>,
) -> Result<Vec<CustomTexturePath>, String> {
    textures
        .iter()
        .map(|(name, file)| {
            let raw = file.path().trim();
            let path = if raw.is_empty() {
                None
            } else {
                Some(
                    AssetPath::new(raw)
                        .map_err(|e| format!("texture {name:?} {:?}: {e}", file.path()))?,
                )
            };
            Ok(CustomTexturePath {
                name: name.clone(),
                path,
                space: file.space(),
            })
        })
        .collect()
}

/// `fragment` — a surface material's WGSL — composed with `uniforms`, the
/// textures named in `textures` and `lighting`, and checked: the shading,
/// or what is wrong and where. Composing is cheap, checking is not;
/// [`compose_custom`] and [`check_custom`] split the two for a caller that
/// composes every frame and checks on change.
pub fn build_custom(
    fragment: &str,
    uniforms: &BTreeMap<String, Uniform>,
    textures: &BTreeMap<String, CustomTextureFile>,
    lighting: SurfaceLighting,
) -> Result<CustomSurfaceSource, String> {
    let (source, layout) = compose_custom(fragment, uniforms, textures, lighting)?;
    let shader = check_custom(source, textures.len())?;
    Ok(CustomSurfaceSource {
        shader,
        layout,
        uniforms: uniforms.clone(),
        lighting,
    })
}

/// The composed source and the uniforms' layout, not yet checked.
pub fn compose_custom(
    fragment: &str,
    uniforms: &BTreeMap<String, Uniform>,
    textures: &BTreeMap<String, CustomTextureFile>,
    lighting: SurfaceLighting,
) -> Result<(String, Layout), String> {
    let layout = Layout::new(uniforms)?;
    let names: Vec<&str> = textures.keys().map(String::as_str).collect();
    let source = compose_surface(fragment, &layout, &names, lighting)?;
    Ok((source, layout))
}

/// A [`compose_custom`]d source checked with naga.
pub fn check_custom(source: String, textures: usize) -> Result<Arc<SurfaceShader>, String> {
    SurfaceShader::new(source, textures).map(Arc::new)
}
