//! What the open draft draws: its standard block and, when it names a
//! shader, its own `surface()` — composed every frame, which is cheap, and
//! checked with naga only when the composed text changes, which is not.
//! The live preview recompiles on an edit and not on a frame.

use std::path::Path;
use std::sync::Arc;

use voltra_assets::{
    check_custom, compose_custom, custom_texture_paths, AssetPath, CustomSurfaceSource,
    MaterialFile, SurfaceSource,
};
use voltra_render::render3d::SurfaceShader;

/// The draft's shader text as last read, and its check.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DraftShader {
    /// The shader path read, the store revision it was read at, and its
    /// text or why it would not read.
    text: Option<(String, Option<u64>, Result<String, String>)>,
    /// The last composed source and what checking it gave.
    checked: Option<(String, Result<Arc<SurfaceShader>, String>)>,
}

/// What a draft draws, and what is wrong with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// What the preview draws: the draft as written, or — its shader
    /// broken — its standard block. `Err` when not even that resolves.
    pub source: Result<SurfaceSource, String>,
    /// Why its own shader does not draw, when it names one.
    pub error: Option<String>,
}

impl DraftShader {
    /// What `file` draws, its shader read from under `root` again when
    /// its path changes or the store's `revision` of the material does
    /// (the `.wgsl` edited on disk reloads the material).
    pub fn draw(&mut self, root: &Path, file: &MaterialFile, revision: Option<u64>) -> Drawn {
        let (standard, _) = file.standard.finite();
        let mut source = match standard.resolve() {
            Ok(source) => source,
            Err(e) => {
                return Drawn {
                    source: Err(e),
                    error: None,
                }
            }
        };
        let shader = file.shader.trim();
        if shader.is_empty() {
            return Drawn {
                source: Ok(source),
                error: None,
            };
        }
        let stale = self
            .text
            .as_ref()
            .is_none_or(|(path, seen, _)| path != shader || *seen != revision);
        if stale {
            let text = AssetPath::new(shader)
                .map_err(|e| format!("the shader {shader:?}: {e}"))
                .and_then(|path| {
                    let full = root.join(path.as_str());
                    std::fs::read_to_string(&full).map_err(|e| format!("{}: {e}", full.display()))
                });
            self.text = Some((shader.to_owned(), revision, text));
        }
        let built = self
            .text
            .as_ref()
            .map_or_else(|| Err("not read".to_owned()), |(_, _, t)| t.clone())
            .and_then(|fragment| {
                let (composed, layout) =
                    compose_custom(&fragment, &file.uniforms, &file.textures, file.lighting)?;
                let shader = self.check(composed, file.textures.len())?;
                let textures = custom_texture_paths(&file.textures)?;
                Ok((
                    CustomSurfaceSource {
                        shader,
                        layout,
                        uniforms: file.uniforms.clone(),
                        lighting: file.lighting,
                    },
                    textures,
                ))
            });
        match built {
            Ok((custom, textures)) => {
                source.textures.custom = textures;
                source.custom = Some(custom);
                Drawn {
                    source: Ok(source),
                    error: None,
                }
            }
            Err(e) => Drawn {
                source: Ok(source),
                error: Some(e),
            },
        }
    }

    /// `composed` checked, or the last check's answer when it is the same
    /// text.
    fn check(&mut self, composed: String, textures: usize) -> Result<Arc<SurfaceShader>, String> {
        if let Some((last, result)) = &self.checked {
            if *last == composed {
                return result.clone();
            }
        }
        let result = check_custom(composed.clone(), textures);
        self.checked = Some((composed, result.clone()));
        result
    }
}

#[cfg(test)]
mod tests {
    use voltra_assets::{CustomTextureFile, StandardFile};
    use voltra_render::material::Uniform;
    use voltra_testkit::scratch_root;

    use super::*;

    const SHADER: &str = "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = material.tint.rgb;\n    return s;\n}\n";

    fn file() -> MaterialFile {
        let mut file = MaterialFile::surface(StandardFile::default());
        file.shader = "materials/x.wgsl".to_owned();
        file.uniforms
            .insert("tint".to_owned(), Uniform::Color((1.0, 0.0, 0.0, 1.0)));
        file
    }

    #[test]
    fn the_draft_recompiles_when_its_composed_text_changes_only() {
        let root = scratch_root();
        std::fs::create_dir_all(root.join("materials")).expect("a folder");
        std::fs::write(root.join("materials/x.wgsl"), SHADER).expect("written");
        let mut draft = DraftShader::default();
        let mut file = file();
        let first = draft.draw(&root, &file, Some(1));
        assert_eq!(first.error, None);
        let custom = |d: &Drawn| d.source.clone().expect("drawn").custom.expect("its own");
        let revision = custom(&first).shader.revision();
        // A value is the block's, not the shader's: no recompile.
        file.uniforms
            .insert("tint".to_owned(), Uniform::Color((0.0, 1.0, 0.0, 1.0)));
        let recoloured = draft.draw(&root, &file, Some(1));
        assert!(Arc::ptr_eq(
            &custom(&recoloured).shader,
            &custom(&first).shader
        ));
        assert_ne!(custom(&recoloured).block(), custom(&first).block());
        // A texture changes the composed text: recompiled.
        file.textures.insert(
            "noise".to_owned(),
            CustomTextureFile::Data("textures/noise.png".to_owned()),
        );
        let textured = draft.draw(&root, &file, Some(1));
        assert_ne!(custom(&textured).shader.revision(), revision);
        assert_eq!(textured.source.expect("drawn").textures.custom.len(), 1);
        // The file broken on disk: read again at the store's next revision,
        // drawn as the standard block with the error.
        std::fs::write(root.join("materials/x.wgsl"), "fn surface( {").expect("written");
        let same = draft.draw(&root, &file, Some(1));
        assert_eq!(same.error, None, "not read again at the same revision");
        let broken = draft.draw(&root, &file, Some(2));
        assert!(broken.error.is_some());
        assert!(broken.source.expect("its standard block").custom.is_none());
    }
}
