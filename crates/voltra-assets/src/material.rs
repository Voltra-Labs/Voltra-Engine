//! A material, as one `.material.ron` file names it: a sprite's WGSL
//! fragment function and its uniforms, or a 3D surface's standard block
//! and, optionally, its own WGSL `surface()` with uniforms and textures.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::material::Uniform;
use voltra_render::render3d::SurfaceLighting;

mod checked;
mod custom;
mod surface;

pub use checked::Material;
pub use custom::{
    build_custom, check_custom, compose_custom, custom_texture_paths, CustomSurfaceSource,
    CustomTextureFile, CustomTexturePath,
};
pub use surface::{StandardFile, SurfaceSource, SurfaceTexturePaths};

/// The suffix that marks a RON file as a material: `flash.material.ron`.
pub const MATERIAL_SUFFIX: &str = ".material.ron";

/// The format a material file is in.
pub const MATERIAL_VERSION: u32 = 1;

/// Whether a file name is a material file, as [`is_atlas`](crate::atlas::is_atlas)
/// answers for slicings.
pub fn is_material(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(MATERIAL_SUFFIX)
}

/// What a material draws on: one `.material.ron` format for both, as
/// Godot has one shader language with `shader_type canvas_item` and
/// `shader_type spatial`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaterialDomain {
    /// A sprite or a tile layer: a WGSL fragment function and its uniforms
    /// (stage 83). The default, so every file written before 3D loads.
    #[default]
    Sprite,
    /// A 3D mesh's surface: the `standard` block, lit.
    Surface,
}

/// What a material file holds. A sprite material:
///
/// ```ron
/// (
///     version: 1,
///     shader: "materials/hit_flash.wgsl",
///     uniforms: {
///         "flash": Float(0.0),
///         "flash_color": Color((1.0, 1.0, 1.0, 1.0)),
///     },
/// )
/// ```
///
/// The shader is a WGSL file defining `fn fragment(in: Fragment) ->
/// vec4<f32>`; see [`voltra_render::material::compose`] for what it sees.
/// The uniforms' values are the defaults a sprite's own override.
///
/// A surface material:
///
/// ```ron
/// (
///     version: 1,
///     domain: Surface,
///     standard: (base_color: (0.9, 0.7, 0.4, 1.0), metallic: 1.0, roughness: 0.3),
/// )
/// ```
///
/// See [`StandardFile`] for the block. A custom surface adds a shader
/// defining `fn surface(in: SurfaceInput) -> Surface`, its uniforms as a
/// sprite material's, up to four named textures, and whether it is lit:
///
/// ```ron
/// (
///     version: 1,
///     domain: Surface,
///     shader: "materials/dissolve.wgsl",
///     uniforms: {"amount": Float(0.45)},
///     textures: {"noise": Data("textures/noise.png")},
///     standard: (alpha: Mask(0.5)),
/// )
/// ```
///
/// See [`voltra_render::render3d::compose_surface`] for what it sees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialFile {
    /// The file format version.
    pub version: u32,
    /// What it draws on; `Sprite` when left out.
    #[serde(default, skip_serializing_if = "is_sprite")]
    pub domain: MaterialDomain,
    /// The WGSL file, from the asset root: a sprite material's fragment
    /// function, or a surface material's `surface()` (empty: the standard
    /// block alone). A plain string, as a person writes it; checked as a
    /// path when the file is read.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shader: String,
    /// Default values by uniform name; a sprite's own values override them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uniforms: BTreeMap<String, Uniform>,
    /// A custom surface's own textures by WGSL name, at most four.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub textures: BTreeMap<String, CustomTextureFile>,
    /// Whether a custom surface is lit; `Lit` when left out.
    #[serde(default, skip_serializing_if = "is_lit")]
    pub lighting: SurfaceLighting,
    /// A surface material's inputs.
    #[serde(default, skip_serializing_if = "StandardFile::is_default")]
    pub standard: StandardFile,
}

fn is_sprite(domain: &MaterialDomain) -> bool {
    *domain == MaterialDomain::Sprite
}

fn is_lit(lighting: &SurfaceLighting) -> bool {
    *lighting == SurfaceLighting::Lit
}

impl MaterialFile {
    /// A surface material holding `standard`.
    pub fn surface(standard: StandardFile) -> Self {
        Self {
            version: MATERIAL_VERSION,
            domain: MaterialDomain::Surface,
            shader: String::new(),
            uniforms: BTreeMap::new(),
            textures: BTreeMap::new(),
            lighting: SurfaceLighting::Lit,
            standard,
        }
    }

    /// Every image a surface material names — its standard block's and its
    /// own textures' — checked as asset paths; the first that is not one
    /// says why.
    pub fn surface_texture_paths(&self) -> Result<SurfaceTexturePaths, String> {
        let mut paths = self.standard.textures()?;
        paths.custom = custom_texture_paths(&self.textures)?;
        Ok(paths)
    }

    /// The file's text, as the editor writes it: pretty, one field a line.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        let config = ron::ser::PrettyConfig::new().struct_names(false);
        ron::ser::to_string_pretty(self, config).map(|mut text| {
            text.push('\n');
            text
        })
    }
}
