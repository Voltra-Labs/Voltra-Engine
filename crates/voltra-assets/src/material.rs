//! A sprite material: its WGSL fragment function and its uniforms, as one
//! `.material.ron` file names them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::material::{compose, validate, Layout, Uniform};

use crate::path::AssetPath;

/// The suffix that marks a RON file as a material: `flash.material.ron`.
pub const MATERIAL_SUFFIX: &str = ".material.ron";

/// The format a material file is in.
pub const MATERIAL_VERSION: u32 = 1;

/// Whether a file name is a material file, as [`is_atlas`](crate::atlas::is_atlas)
/// answers for slicings.
pub fn is_material(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(MATERIAL_SUFFIX)
}

/// What a material file holds:
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialFile {
    /// The file format version.
    pub version: u32,
    /// The fragment shader's path from the asset root. A plain string, as
    /// a person writes it; checked as a path when the file is read.
    pub shader: String,
    /// Default values by uniform name; a sprite's own values override them.
    #[serde(default)]
    pub uniforms: BTreeMap<String, Uniform>,
}

/// A material read and checked: its whole shader composed, its uniforms
/// laid out — or why it cannot be drawn with, in which case what names it
/// draws as a plain sprite.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// The fragment shader file, which a change to also reloads this.
    pub shader: Option<AssetPath>,
    /// The composed WGSL, empty when there is an error.
    pub source: String,
    /// How the uniforms are laid out in the block, or `None` when the material cannot be drawn with.
    pub layout: Option<Layout>,
    /// The values the uniforms take where a sprite sets none.
    pub defaults: BTreeMap<String, Uniform>,
    /// Bumped by the store whenever this material changes: what a pipeline
    /// built from an earlier one compares to know it is stale.
    pub revision: u64,
    /// Why the material cannot be drawn with, or `None` when it can.
    pub error: Option<String>,
}

impl Default for Material {
    /// A material that draws as a plain sprite: nothing to compile.
    fn default() -> Self {
        Self {
            shader: None,
            source: String::new(),
            layout: None,
            defaults: BTreeMap::new(),
            revision: 0,
            error: Some("no material".to_owned()),
        }
    }
}

impl Material {
    /// The material `file` describes, with `fragment` its shader's text:
    /// composed and validated, or carrying what is wrong.
    pub fn build(file: &MaterialFile, shader: AssetPath, fragment: &str) -> Self {
        let mut material = Self {
            shader: Some(shader),
            defaults: file.uniforms.clone(),
            ..Self::default()
        };
        if file.version > MATERIAL_VERSION {
            material.error = Some(format!(
                "version {} is newer than {MATERIAL_VERSION}",
                file.version
            ));
            return material;
        }
        let layout = match Layout::new(&file.uniforms) {
            Ok(layout) => layout,
            Err(e) => {
                material.error = Some(e);
                return material;
            }
        };
        let source = compose(fragment, &layout);
        match validate(&source) {
            Ok(()) => {
                material.source = source;
                material.layout = Some(layout);
                material.error = None;
            }
            Err(e) => material.error = Some(e),
        }
        material
    }

    /// Whether it can be drawn with.
    pub fn is_usable(&self) -> bool {
        self.error.is_none() && self.layout.is_some()
    }
}
