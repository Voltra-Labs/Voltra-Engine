//! A surface material's `standard` block: glTF 2.0's metallic-roughness
//! inputs as a `.material.ron` writes them, and what they resolve to.

use serde::{Deserialize, Serialize};
use voltra_render::render3d::{AlphaMode, StandardSurface, UvTransform};

use crate::path::AssetPath;

/// The `standard: (...)` block of a `domain: Surface` material — glTF
/// 2.0's metallic-roughness material, field for field, so an imported
/// model's materials map onto it one to one:
///
/// ```ron
/// standard: (
///     base_color: (0.8, 0.3, 0.2, 1.0),
///     base_color_texture: "textures/brick.png",
///     metallic: 0.0,
///     roughness: 0.7,
///     normal_texture: "textures/brick_normal.png",
///     alpha: Mask(0.5),
/// ),
/// ```
///
/// Every field may be left out; a texture is a path from the asset root,
/// empty for none. Colours are linear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StandardFile {
    /// Linear RGBA, multiplying the texture.
    pub base_color: [f32; 4],
    /// Colour (sRGB) and alpha.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base_color_texture: String,
    /// 0 a dielectric, 1 a metal; multiplies the texture's blue channel.
    pub metallic: f32,
    /// Perceptual roughness; multiplies the texture's green channel.
    pub roughness: f32,
    /// Data: G roughness, B metallic.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub metallic_roughness_texture: String,
    /// Data: a tangent-space normal map.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub normal_texture: String,
    /// How far the normal map bends the normal.
    pub normal_scale: f32,
    /// Data: R how much ambient reaches each point.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub occlusion_texture: String,
    /// How much the occlusion texture darkens.
    pub occlusion_strength: f32,
    /// Linear RGB light given off, multiplying the texture.
    pub emissive: [f32; 3],
    /// Colour (sRGB).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub emissive_texture: String,
    /// Multiplies `emissive`, in the light units' energy.
    pub emissive_strength: f32,
    /// `Opaque`, `Mask(cutoff)` or `Blend`.
    pub alpha: AlphaMode,
    /// Both faces drawn.
    pub double_sided: bool,
    /// Its base colour drawn as it is, no light.
    pub unlit: bool,
    /// Moves every texture's coordinates.
    #[serde(skip_serializing_if = "UvTransform::is_identity")]
    pub uv_transform: UvTransform,
}

impl Default for StandardFile {
    fn default() -> Self {
        let d = StandardSurface::DEFAULT;
        Self {
            base_color: d.base_color,
            base_color_texture: String::new(),
            metallic: d.metallic,
            roughness: d.roughness,
            metallic_roughness_texture: String::new(),
            normal_texture: String::new(),
            normal_scale: d.normal_scale,
            occlusion_texture: String::new(),
            occlusion_strength: d.occlusion_strength,
            emissive: d.emissive,
            emissive_texture: String::new(),
            emissive_strength: d.emissive_strength,
            alpha: d.alpha,
            double_sided: d.double_sided,
            unlit: d.unlit,
            uv_transform: d.uv_transform,
        }
    }
}

impl StandardFile {
    /// Whether every field is its default: a sprite material's file
    /// leaves the block out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The numbers, without the images.
    pub fn surface(&self) -> StandardSurface {
        StandardSurface {
            base_color: self.base_color,
            metallic: self.metallic,
            roughness: self.roughness,
            normal_scale: self.normal_scale,
            occlusion_strength: self.occlusion_strength,
            emissive: self.emissive,
            emissive_strength: self.emissive_strength,
            alpha: self.alpha,
            double_sided: self.double_sided,
            unlit: self.unlit,
            uv_transform: self.uv_transform,
        }
    }

    /// The texture paths, checked as asset paths; the first that is not
    /// one says why.
    pub fn textures(&self) -> Result<SurfaceTexturePaths, String> {
        let path = |name: &str, raw: &str| -> Result<Option<AssetPath>, String> {
            if raw.trim().is_empty() {
                return Ok(None);
            }
            AssetPath::new(raw)
                .map(Some)
                .map_err(|e| format!("{name} {raw:?}: {e}"))
        };
        Ok(SurfaceTexturePaths {
            base_color: path("base_color_texture", &self.base_color_texture)?,
            metallic_roughness: path(
                "metallic_roughness_texture",
                &self.metallic_roughness_texture,
            )?,
            normal: path("normal_texture", &self.normal_texture)?,
            occlusion: path("occlusion_texture", &self.occlusion_texture)?,
            emissive: path("emissive_texture", &self.emissive_texture)?,
        })
    }

    /// What it resolves to, or why it cannot.
    pub fn resolve(&self) -> Result<SurfaceSource, String> {
        Ok(SurfaceSource {
            standard: self.surface(),
            textures: self.textures()?,
        })
    }
}

/// The images a surface names, by slot, each from the asset root.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceTexturePaths {
    /// Colour.
    pub base_color: Option<AssetPath>,
    /// Data: G roughness, B metallic.
    pub metallic_roughness: Option<AssetPath>,
    /// Data: a tangent-space normal.
    pub normal: Option<AssetPath>,
    /// Data: R occlusion.
    pub occlusion: Option<AssetPath>,
    /// Colour.
    pub emissive: Option<AssetPath>,
}

impl SurfaceTexturePaths {
    /// Every path named, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &AssetPath> {
        [
            &self.base_color,
            &self.metallic_roughness,
            &self.normal,
            &self.occlusion,
            &self.emissive,
        ]
        .into_iter()
        .flatten()
    }
}

/// A surface material read and checked: its numbers and the images it
/// names — what the draw turns into a GPU material once the images are
/// loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceSource {
    /// The numbers.
    pub standard: StandardSurface,
    /// The images, by slot.
    pub textures: SurfaceTexturePaths,
}

impl Default for SurfaceSource {
    /// The default white standard: what a mesh with no material draws.
    fn default() -> Self {
        Self {
            standard: StandardSurface::DEFAULT,
            textures: SurfaceTexturePaths::default(),
        }
    }
}
