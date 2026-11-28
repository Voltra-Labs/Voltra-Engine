//! A material read and checked: what a `.material.ron` file resolves to,
//! or why it cannot be drawn with.

use std::collections::BTreeMap;

use voltra_render::material::{compose, validate, Layout, Uniform};
use voltra_render::render3d::SurfaceLighting;

use super::custom::{build_custom, custom_texture_paths};
use super::{MaterialDomain, MaterialFile, SurfaceSource, MATERIAL_VERSION};
use crate::path::AssetPath;

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
        material.defaults = finite_uniforms(&file.uniforms, &mut material.warnings);
        for (set, what) in [
            (!file.standard.is_default(), "`standard` block is"),
            (!file.textures.is_empty(), "`textures` are"),
            (file.lighting != SurfaceLighting::Lit, "`lighting` is"),
        ] {
            if set {
                material
                    .warnings
                    .push(format!("a sprite material's {what} ignored (a surface's)"));
            }
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
    /// are asset paths, and — when it names a shader, `shader` being that
    /// file's path and its text or why it would not read — its `surface()`
    /// composes and validates with its uniforms and textures.
    ///
    /// **A custom surface that will not compile draws its standard block**
    /// — what its `surface()` would have started from — with the error
    /// kept, as a broken sprite material's sprites draw plainly. A standard
    /// texture path that is no asset path leaves no surface at all (the
    /// default white standard).
    pub fn build_surface(
        file: &MaterialFile,
        shader: Option<(AssetPath, Result<String, String>)>,
    ) -> Self {
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
        let custom = !file.shader.trim().is_empty();
        if !custom {
            for (set, what) in [
                (!file.uniforms.is_empty(), "`uniforms` are"),
                (!file.textures.is_empty(), "`textures` are"),
                (
                    file.lighting != SurfaceLighting::Lit,
                    "`lighting` is (`standard.unlit` turns the light off)",
                ),
            ] {
                if set {
                    material.warnings.push(format!(
                        "a surface material's {what} ignored without a `shader`"
                    ));
                }
            }
        }
        let (standard, replaced) = file.standard.finite();
        for field in replaced {
            material.warnings.push(format!(
                "standard.{field} is not a finite number; its default instead"
            ));
        }
        let mut surface = match standard.resolve() {
            Ok(surface) => surface,
            Err(e) => {
                material.error = Some(e);
                return material;
            }
        };
        material.error = None;
        if custom {
            let uniforms = finite_uniforms(&file.uniforms, &mut material.warnings);
            let built = match shader {
                Some((path, text)) => {
                    material.shader = Some(path);
                    text.and_then(|fragment| {
                        build_custom(&fragment, &uniforms, &file.textures, file.lighting)
                    })
                }
                None => Err(format!("the shader {:?} is no asset path", file.shader)),
            }
            .and_then(|built| Ok((built, custom_texture_paths(&file.textures)?)));
            match built {
                Ok((built, textures)) => {
                    surface.textures.custom = textures;
                    surface.custom = Some(built);
                }
                Err(e) => material.error = Some(e),
            }
        }
        material.surface = Some(surface);
        material
    }

    /// Whether a sprite can be drawn with it: a sprite material that
    /// composed and validated.
    pub fn is_usable(&self) -> bool {
        self.error.is_none() && self.layout.is_some()
    }

    /// Its surface, when it is a surface material that can be drawn with:
    /// as written, or — a custom surface that would not compile, its
    /// [`error`](Self::error) set — as its standard block.
    pub fn surface(&self) -> Option<&SurfaceSource> {
        self.surface.as_ref()
    }
}

/// `uniforms` with every number that is not finite made 0, each said in
/// `warnings`.
fn finite_uniforms(
    uniforms: &BTreeMap<String, Uniform>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, Uniform> {
    let mut out = uniforms.clone();
    for (name, value) in &mut out {
        if !value.is_finite() {
            *value = value.finite();
            warnings.push(format!(
                "uniform {name:?} is not a finite number; 0 instead"
            ));
        }
    }
    out
}
