//! A material, as one `.material.ron` file names it: a sprite's WGSL
//! fragment function and its uniforms, or a 3D surface's standard block.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::material::Uniform;

mod checked;
mod surface;

pub use checked::Material;
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
/// See [`StandardFile`] for the block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialFile {
    /// The file format version.
    pub version: u32,
    /// What it draws on; `Sprite` when left out.
    #[serde(default, skip_serializing_if = "is_sprite")]
    pub domain: MaterialDomain,
    /// A sprite material's fragment shader, from the asset root. A plain
    /// string, as a person writes it; checked as a path when the file is
    /// read. Empty in a surface material.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shader: String,
    /// Default values by uniform name; a sprite's own values override them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uniforms: BTreeMap<String, Uniform>,
    /// A surface material's inputs.
    #[serde(default, skip_serializing_if = "StandardFile::is_default")]
    pub standard: StandardFile,
}

fn is_sprite(domain: &MaterialDomain) -> bool {
    *domain == MaterialDomain::Sprite
}

impl MaterialFile {
    /// A surface material holding `standard`.
    pub fn surface(standard: StandardFile) -> Self {
        Self {
            version: MATERIAL_VERSION,
            domain: MaterialDomain::Surface,
            shader: String::new(),
            uniforms: BTreeMap::new(),
            standard,
        }
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
