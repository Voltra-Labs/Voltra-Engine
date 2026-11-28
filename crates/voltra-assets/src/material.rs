//! A material, as one `.material.ron` file names it: a sprite's WGSL
//! fragment function and its uniforms, or a 3D surface's standard block.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::material::{compose, validate, Layout, Uniform};

use crate::path::AssetPath;

mod surface;

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

/// A material read and checked: a sprite material's whole shader composed
/// and its uniforms laid out, or a surface material's inputs — or why it
/// cannot be drawn with, in which case a sprite naming it draws plainly
/// and a mesh as the default white standard.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// What it draws on.
    pub domain: MaterialDomain,
    /// A surface material's numbers and images; `None` for a sprite
    /// material, or one with an error.
    pub surface: Option<SurfaceSource>,
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
    /// What the file says that is ignored or replaced — a number that is
    /// not finite, a block the domain does not read — said once each time
    /// the file is read.
    pub warnings: Vec<String>,
}

impl Default for Material {
    /// A material that draws as a plain sprite: nothing to compile.
    fn default() -> Self {
        Self {
            domain: MaterialDomain::Sprite,
            surface: None,
            shader: None,
            source: String::new(),
            layout: None,
            defaults: BTreeMap::new(),
            revision: 0,
            error: Some("no material".to_owned()),
            warnings: Vec::new(),
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
        for (name, value) in &mut material.defaults {
            if !value.is_finite() {
                *value = value.finite();
                material.warnings.push(format!(
                    "uniform {name:?} is not a finite number; 0 instead"
                ));
            }
        }
        if !file.standard.is_default() {
            material.warnings.push(
                "a sprite material's `standard` block is ignored (it is a surface's)".to_owned(),
            );
        }
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

    /// The surface material `file` describes, checked: its texture paths
    /// are asset paths, and it names no shader (a custom surface shader is
    /// stage 138's).
    pub fn build_surface(file: &MaterialFile) -> Self {
        let mut material = Self {
            domain: MaterialDomain::Surface,
            ..Self::default()
        };
        if file.version > MATERIAL_VERSION {
            material.error = Some(format!(
                "version {} is newer than {MATERIAL_VERSION}",
                file.version
            ));
            return material;
        }
        if !file.shader.trim().is_empty() {
            material.error = Some(format!(
                "a surface material's shader ({:?}) is not read yet: surface materials are \
                 the standard block alone",
                file.shader
            ));
            return material;
        }
        if !file.uniforms.is_empty() {
            material.warnings.push(
                "a surface material's `uniforms` are ignored (they are a sprite shader's)"
                    .to_owned(),
            );
        }
        let (standard, replaced) = file.standard.finite();
        for field in replaced {
            material.warnings.push(format!(
                "standard.{field} is not a finite number; its default instead"
            ));
        }
        match standard.resolve() {
            Ok(surface) => {
                material.surface = Some(surface);
                material.error = None;
            }
            Err(e) => material.error = Some(e),
        }
        material
    }

    /// Whether a sprite can be drawn with it: a sprite material that
    /// composed and validated.
    pub fn is_usable(&self) -> bool {
        self.error.is_none() && self.layout.is_some()
    }

    /// Its surface, when it is a surface material that can be drawn with.
    pub fn surface(&self) -> Option<&SurfaceSource> {
        self.surface.as_ref().filter(|_| self.error.is_none())
    }
}
